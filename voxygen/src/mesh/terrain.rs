use crate::{
    mesh::{vol, Meshable},
    render::{self, FluidPipeline, Mesh, TerrainPipeline},
};
use common::{
    terrain::Block,
    vol::{ReadVol, RectRasterableVol, Vox},
    volumes::vol_grid_2d::VolGrid2d,
};
use std::fmt::Debug;
use vek::*;

type TerrainVertex = <TerrainPipeline as render::Pipeline>::Vertex;
type FluidVertex = <FluidPipeline as render::Pipeline>::Vertex;

const DIRS: [Vec2<i32>; 4] = [
    Vec2 { x: 1, y: 0 },
    Vec2 { x: 0, y: 1 },
    Vec2 { x: -1, y: 0 },
    Vec2 { x: 0, y: -1 },
];

const DIRS_3D: [Vec3<i32>; 6] = [
    Vec3 { x: 1, y: 0, z: 0 },
    Vec3 { x: 0, y: 1, z: 0 },
    Vec3 { x: 0, y: 0, z: 1 },
    Vec3 { x: -1, y: 0, z: 0 },
    Vec3 { x: 0, y: -1, z: 0 },
    Vec3 { x: 0, y: 0, z: -1 },
];

fn calc_light<V: RectRasterableVol<Vox = Block> + ReadVol + Debug>(
    bounds: Aabb<i32>,
    vol: &VolGrid2d<V>,
) -> impl Fn(Vec3<i32>) -> f32 {
    const NOT_VOID: u8 = 255;
    const SUNLIGHT: u8 = 24;

    let outer = Aabb {
        min: bounds.min - SUNLIGHT as i32,
        max: bounds.max + SUNLIGHT as i32,
    };

    let mut vol_cached = vol.cached();

    // Voids are voxels that that contain air or liquid that are protected from direct rays by blocks
    // above them
    //
    let mut voids = vec![NOT_VOID; outer.size().product() as usize];
    let void_idx = {
        let (_, h, d) = outer.clone().size().into_tuple();
        move |x, y, z| (x * h * d + y * d + z) as usize
    };
    // List of voids for efficient iteration
    let mut voids_list = vec![];
    // Rays are cast down
    // Vec<(highest non air block, lowest non air block)>
    let mut rays = vec![(outer.size().d, 0); (outer.size().w * outer.size().h) as usize];
    for x in 0..outer.size().w {
        for y in 0..outer.size().h {
            let mut outside = true;
            for z in (0..outer.size().d).rev() {
                let block = vol_cached
                    .get(outer.min + Vec3::new(x, y, z))
                    .ok()
                    .copied()
                    .unwrap_or(Block::empty());

                if !block.is_air() {
                    if outside {
                        rays[(outer.size().w * y + x) as usize].0 = z;
                        outside = false;
                    }
                    rays[(outer.size().w * y + x) as usize].1 = z;
                }

                if (block.is_air() || block.is_fluid()) && !outside {
                    voids_list.push(Vec3::new(x, y, z));
                    voids[void_idx(x, y, z)] = 0;
                }
            }
        }
    }

    // Propagate light into voids adjacent to rays
    let mut opens = Vec::new();
    'voids: for pos in &mut voids_list {
        let void_idx = void_idx(pos.x, pos.y, pos.z);
        for dir in &DIRS {
            let col = Vec2::<i32>::from(*pos) + dir;
            // If above highest non air block (ray passes by)
            if pos.z
                > *rays
                    .get(((outer.size().w * col.y) + col.x) as usize)
                    .map(|(ray, _)| ray)
                    .unwrap_or(&0)
            {
                voids[void_idx] = SUNLIGHT - 1;
                opens.push(*pos);
                continue 'voids;
            }
        }

        // Ray hits directly (occurs for liquids)
        if pos.z
            >= *rays
                .get(((outer.size().w * pos.y) + pos.x) as usize)
                .map(|(ray, _)| ray)
                .unwrap_or(&0)
        {
            voids[void_idx] = SUNLIGHT - 1;
            opens.push(*pos);
        }
    }

    while opens.len() > 0 {
        let mut new_opens = Vec::new();
        for open in &opens {
            let parent_l = voids[void_idx(open.x, open.y, open.z)];
            for dir in &DIRS_3D {
                let other = *open + *dir;
                if let Some(l) = voids.get_mut(void_idx(other.x, other.y, other.z)) {
                    if *l < parent_l - 1 {
                        new_opens.push(other);
                        *l = parent_l - 1;
                    }
                }
            }
        }
        opens = new_opens;
    }

    move |wpos| {
        let pos = wpos - outer.min;
        rays.get(((outer.size().w * pos.y) + pos.x) as usize)
            .and_then(|(ray, deep)| {
                if pos.z > *ray {
                    Some(1.0)
                } else if pos.z < *deep {
                    Some(0.0)
                } else {
                    None
                }
            })
            .or_else(|| {
                voids
                    .get(void_idx(pos.x, pos.y, pos.z))
                    .filter(|l| **l != NOT_VOID)
                    .map(|l| *l as f32 / SUNLIGHT as f32)
            })
            .unwrap_or(0.0)
    }
}

impl<V: RectRasterableVol<Vox = Block> + ReadVol + Debug> Meshable<TerrainPipeline, FluidPipeline>
    for VolGrid2d<V>
{
    type Pipeline = TerrainPipeline;
    type TranslucentPipeline = FluidPipeline;
    type Supplement = Aabb<i32>;

    fn generate_mesh(
        &self,
        range: Self::Supplement,
    ) -> (Mesh<Self::Pipeline>, Mesh<Self::TranslucentPipeline>) {
        let mut opaque_mesh = Mesh::new();
        let mut fluid_mesh = Mesh::new();

        let light = calc_light(range, self);

        let mut vol_cached = self.cached();

        for x in range.min.x + 1..range.max.x - 1 {
            for y in range.min.y + 1..range.max.y - 1 {
                let mut lights = [[[0.0; 3]; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        for k in 0..3 {
                            lights[k][j][i] = light(
                                Vec3::new(x, y, range.min.z)
                                    + Vec3::new(i as i32, j as i32, k as i32)
                                    - 1,
                            );
                        }
                    }
                }

                let get_color = |maybe_block: Option<&Block>| {
                    maybe_block
                        .filter(|vox| vox.is_opaque())
                        .and_then(|vox| vox.get_color())
                        .map(|col| Rgba::from_opaque(col))
                        .unwrap_or(Rgba::zero())
                };

                let mut blocks = [[[None; 3]; 3]; 3];
                let mut colors = [[[Rgba::zero(); 3]; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        for k in 0..3 {
                            let block = vol_cached
                                .get(
                                    Vec3::new(x, y, range.min.z)
                                        + Vec3::new(i as i32, j as i32, k as i32)
                                        - 1,
                                )
                                .ok()
                                .copied();
                            colors[k][j][i] = get_color(block.as_ref());
                            blocks[k][j][i] = block;
                        }
                    }
                }

                for z in range.min.z..range.max.z {
                    let pos = Vec3::new(x, y, z);
                    let offs = (pos - (range.min + 1) * Vec3::new(1, 1, 0)).map(|e| e as f32);

                    lights[0] = lights[1];
                    lights[1] = lights[2];
                    blocks[0] = blocks[1];
                    blocks[1] = blocks[2];
                    colors[0] = colors[1];
                    colors[1] = colors[2];

                    for i in 0..3 {
                        for j in 0..3 {
                            lights[2][j][i] = light(pos + Vec3::new(i as i32, j as i32, 2) - 1);
                        }
                    }
                    for i in 0..3 {
                        for j in 0..3 {
                            let block = vol_cached
                                .get(pos + Vec3::new(i as i32, j as i32, 2) - 1)
                                .ok()
                                .copied();
                            colors[2][j][i] = get_color(block.as_ref());
                            blocks[2][j][i] = block;
                        }
                    }

                    let block = blocks[1][1][1];

                    // Create mesh polygons
                    if block.map(|vox| vox.is_opaque()).unwrap_or(false) {
                        vol::push_vox_verts(
                            &mut opaque_mesh,
                            faces_to_make(&blocks, false, |vox| !vox.is_opaque()),
                            offs,
                            &colors, //&[[[colors[1][1][1]; 3]; 3]; 3],
                            |pos, norm, col, ao, light| {
                                let light = (light.min(ao) * 255.0) as u32;
                                let norm = if norm.x != 0.0 {
                                    if norm.x < 0.0 {
                                        0
                                    } else {
                                        1
                                    }
                                } else if norm.y != 0.0 {
                                    if norm.y < 0.0 {
                                        2
                                    } else {
                                        3
                                    }
                                } else {
                                    if norm.z < 0.0 {
                                        4
                                    } else {
                                        5
                                    }
                                };
                                TerrainVertex::new(norm, light, pos, col)
                            },
                            &lights,
                        );
                    } else if block.map(|vox| vox.is_fluid()).unwrap_or(false) {
                        vol::push_vox_verts(
                            &mut fluid_mesh,
                            faces_to_make(&blocks, false, |vox| vox.is_air()),
                            offs,
                            &colors,
                            |pos, norm, col, _ao, light| {
                                FluidVertex::new(pos, norm, col, light, 0.3)
                            },
                            &lights,
                        );
                    }
                }
            }
        }

        (opaque_mesh, fluid_mesh)
    }
}

/// Use the 6 voxels/blocks surrounding the center
/// to detemine which faces should be drawn
/// Unlike the one in segments.rs this uses a provided array of blocks instead
/// of retrieving from a volume
/// blocks[z][y][x]
fn faces_to_make(
    blocks: &[[[Option<Block>; 3]; 3]; 3],
    error_makes_face: bool,
    should_add: impl Fn(Block) -> bool,
) -> [bool; 6] {
    // Faces to draw
    let make_face = |opt_v: Option<Block>| opt_v.map(|v| should_add(v)).unwrap_or(error_makes_face);
    [
        make_face(blocks[1][1][0]),
        make_face(blocks[1][1][2]),
        make_face(blocks[1][0][1]),
        make_face(blocks[1][2][1]),
        make_face(blocks[0][1][1]),
        make_face(blocks[2][1][1]),
    ]
}

/*
impl<V: BaseVol<Vox = Block> + ReadVol + Debug> Meshable for VolGrid3d<V> {
    type Pipeline = TerrainPipeline;
    type Supplement = Aabb<i32>;

    fn generate_mesh(&self, range: Self::Supplement) -> Mesh<Self::Pipeline> {
        let mut mesh = Mesh::new();

        let mut last_chunk_pos = self.pos_key(range.min);
        let mut last_chunk = self.get_key(last_chunk_pos);

        let size = range.max - range.min;
        for x in 1..size.x - 1 {
            for y in 1..size.y - 1 {
                for z in 1..size.z - 1 {
                    let pos = Vec3::new(x, y, z);

                    let new_chunk_pos = self.pos_key(range.min + pos);
                    if last_chunk_pos != new_chunk_pos {
                        last_chunk = self.get_key(new_chunk_pos);
                        last_chunk_pos = new_chunk_pos;
                    }
                    let offs = pos.map(|e| e as f32 - 1.0);
                    if let Some(chunk) = last_chunk {
                        let chunk_pos = Self::chunk_offs(range.min + pos);
                        if let Some(col) = chunk.get(chunk_pos).ok().and_then(|vox| vox.get_color())
                        {
                            let col = col.map(|e| e as f32 / 255.0);

                            vol::push_vox_verts(
                                &mut mesh,
                                self,
                                range.min + pos,
                                offs,
                                col,
                                TerrainVertex::new,
                                false,
                            );
                        }
                    } else {
                        if let Some(col) = self
                            .get(range.min + pos)
                            .ok()
                            .and_then(|vox| vox.get_color())
                        {
                            let col = col.map(|e| e as f32 / 255.0);

                            vol::push_vox_verts(
                                &mut mesh,
                                self,
                                range.min + pos,
                                offs,
                                col,
                                TerrainVertex::new,
                                false,
                            );
                        }
                    }
                }
            }
        }
        mesh
    }
}
*/
