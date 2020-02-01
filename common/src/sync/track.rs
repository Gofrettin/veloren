use super::{
    packet::{CompPacket, CompUpdateKind},
    uid::Uid,
};
use specs::{BitSet, Component, Entity, Join, ReadStorage, World, WorldExt};
use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

pub struct UpdateTracker<C: Component> {
    reader_id: specs::ReaderId<specs::storage::ComponentEvent>,
    inserted: BitSet,
    modified: BitSet,
    removed: BitSet,
    phantom: PhantomData<C>,
}
impl<C: Component> UpdateTracker<C>
where
    C::Storage: specs::storage::Tracked,
{
    pub fn new(specs_world: &mut World) -> Self {
        Self {
            reader_id: specs_world.write_storage::<C>().register_reader(),
            inserted: BitSet::new(),
            modified: BitSet::new(),
            removed: BitSet::new(),
            phantom: PhantomData,
        }
    }

    pub fn inserted(&self) -> &BitSet { &self.inserted }

    pub fn modified(&self) -> &BitSet { &self.modified }

    pub fn removed(&self) -> &BitSet { &self.removed }

    pub fn record_changes<'a>(&mut self, storage: &specs::ReadStorage<'a, C>) {
        self.inserted.clear();
        self.modified.clear();
        self.removed.clear();

        for event in storage.channel().read(&mut self.reader_id) {
            match event {
                specs::storage::ComponentEvent::Inserted(id) => {
                    // If previously removed/modified we don't need to know that anymore
                    self.removed.remove(*id);
                    self.modified.remove(*id);
                    self.inserted.add(*id);
                },
                specs::storage::ComponentEvent::Modified(id) => {
                    // We don't care about modification if the component was just added
                    if !self.inserted.contains(*id) {
                        debug_assert!(!self.removed.contains(*id)); // Theoretically impossible
                        self.modified.add(*id);
                    }
                },
                specs::storage::ComponentEvent::Removed(id) => {
                    // Don't need to know that it was inserted/modified if it was subsequently
                    // removed
                    self.inserted.remove(*id);
                    self.modified.remove(*id);
                    self.removed.add(*id);
                },
            };
        }
    }
}

impl<C: Component + Clone + Send + Sync> UpdateTracker<C> {
    pub fn add_packet_for<'a, P>(
        &self,
        storage: &ReadStorage<'a, C>,
        entity: Entity,
        packets: &mut Vec<P>,
    ) where
        P: CompPacket,
        P: From<C>,
        C: TryFrom<P>,
        P::Phantom: From<PhantomData<C>>,
        P::Phantom: TryInto<PhantomData<C>>,
        C::Storage: specs::storage::Tracked,
    {
        if let Some(comp) = storage.get(entity) {
            packets.push(P::from(comp.clone()));
        }
    }

    pub fn get_updates_for<'a, P>(
        &self,
        uids: &specs::ReadStorage<'a, Uid>,
        storage: &specs::ReadStorage<'a, C>,
        entity_filter: impl Join + Copy,
        buf: &mut Vec<(u64, CompUpdateKind<P>)>,
    ) where
        P: CompPacket,
        P: From<C>,
        C: TryFrom<P>,
        P::Phantom: From<PhantomData<C>>,
        P::Phantom: TryInto<PhantomData<C>>,
        C::Storage: specs::storage::Tracked,
    {
        // Generate inserted updates
        for (uid, comp, _, _) in (uids, storage, &self.inserted, entity_filter).join() {
            buf.push((
                (*uid).into(),
                CompUpdateKind::Inserted(P::from(comp.clone())),
            ));
        }

        // Generate modified updates
        for (uid, comp, _, _) in (uids, storage, &self.modified, entity_filter).join() {
            buf.push((
                (*uid).into(),
                CompUpdateKind::Modified(P::from(comp.clone())),
            ));
        }

        // Generate removed updates
        for (uid, _, _) in (uids, &self.removed, entity_filter).join() {
            buf.push((
                (*uid).into(),
                CompUpdateKind::Removed(P::Phantom::from(PhantomData::<C>)),
            ));
        }
    }
}
