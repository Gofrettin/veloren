// TODO: figure out where exactly this code should be located

// Library
use conrod_core::{
    Positionable,
    Sizeable,
    Widget,
    Labelable,
    widget_ids,
    event::Input,
    image::Id as ImgId,
    text::font::Id as FontId,
    widget::{
        Image,
        Button,
        Canvas,
    }
};

// Crate
use crate::{
    window::Window,
    render::Renderer,
    ui::Ui,
};

widget_ids!{
    struct Ids {
        bag,
        bag_contents,
        bag_close,
        menu_top,
        menu_mid,
        menu_bot,
        menu_canvas,
        menu_buttons[],
    }
}

// TODO: make macro to mimic widget_ids! for images ids or find another solution to simplify addition of new images.
struct Imgs {
    bag: ImgId,
    bag_hover: ImgId,
    bag_press: ImgId,
    bag_open: ImgId,
    bag_open_hover: ImgId,
    bag_open_press: ImgId,
    bag_contents: ImgId,
    close_button: ImgId,
    close_button_hover: ImgId,
    close_button_press: ImgId,
    menu_top: ImgId,
    menu_mid: ImgId,
    menu_bot: ImgId,
    menu_button: ImgId,
}
impl Imgs {
    fn new(ui: &mut Ui, renderer: &mut Renderer) -> Imgs {
        let mut load = |filename| {
            let image = image::open(&[env!("CARGO_MANIFEST_DIR"), "/test_assets/hud/", filename].concat()).unwrap();
            ui.new_image(renderer, &image).unwrap()
        };
        Imgs {
            bag: load("bag/icon/0_bag.png"),
            bag_hover: load("bag/icon/1_bag_hover.png"),
            bag_press: load("bag/icon/2_bag_press.png"),
            bag_open: load("bag/icon/3_bag_open.png"),
            bag_open_hover: load("bag/icon/4_bag_open_hover.png"),
            bag_open_press: load("bag/icon/5_bag_open_press.png"),
            bag_contents: load("bag/bg.png"),
            close_button: load("x/0_x.png"),
            close_button_hover: load("x/1_x_hover.png"),
            close_button_press: load("x/2_x_press.png"),
            menu_button: load("menu/main/menu_button.png"),
            menu_top: load("menu/main/menu_top.png"),
            menu_mid: load("menu/main/menu_mid.png"),
            menu_bot: load("menu/main/menu_bottom.png"),
        }
    }
}

pub struct TestHud {
    ui: Ui,
    ids: Ids,
    imgs: Imgs,
    bag_open: bool,
    menu_open: bool,
    font_id: FontId,
}

impl TestHud {
    pub fn new(window: &mut Window) -> Self {
        let mut ui = Ui::new(window).unwrap();
        // Generate ids
        let mut ids = Ids::new(ui.id_generator());
        ids.menu_buttons.resize(5, &mut ui.id_generator());
        // Load images
        let imgs = Imgs::new(&mut ui, window.renderer_mut());
        // Load font
        let font_id = ui.new_font(conrod_core::text::font::from_file(
            concat!(env!("CARGO_MANIFEST_DIR"), "/test_assets/font/Metamorphous-Regular.ttf")
        ).unwrap());
        Self {
            ui,
            imgs,
            ids,
            bag_open: false,
            menu_open: false,
            font_id,
        }
    }

    fn update_layout(&mut self) {

        let ref mut ui_widgets = self.ui.set_widgets();

        // Check if the bag was clicked
        // (can't use .was_clicked() because we are changing the image and this is after setting the widget which causes flickering as it takes a frame to change after the mouse button is lifted)
        if ui_widgets.widget_input(self.ids.bag).clicks().left().count() % 2 == 1 {
            self.bag_open = !self.bag_open;
        }
        // Bag contents
        // Note that display_contents is set before checking if the bag was clicked
        // this ensures that the contents and open bag img are displayed on the same frame
        if self.bag_open {
            // Contents
            Image::new(self.imgs.bag_contents)
                .w_h(1504.0/4.0, 1760.0/4.0)
                .bottom_right_with_margins(88.0, 68.0)
                .set(self.ids.bag_contents, ui_widgets);

            // X-Button
            if Button::image(self.imgs.close_button)
                    .w_h(144.0/4.0, 144.0/4.0)
                    .hover_image(self.imgs.close_button_hover)
                    .press_image(self.imgs.close_button_press)
                    .top_right_with_margins_on(self.ids.bag_contents, 0.0, 12.0)
                    .set(self.ids.bag_close, ui_widgets)
                    .was_clicked() {
                            self.bag_open = false;
                    }

        }
        //Bag
        Button::image(if self.bag_open {self.imgs.bag_open} else {self.imgs.bag})
            .bottom_right_with_margin_on(ui_widgets.window, 20.0)
            .hover_image(if self.bag_open {self.imgs.bag_open_hover} else {self.imgs.bag_hover})
            .press_image(if self.bag_open {self.imgs.bag_open_press} else {self.imgs.bag_press})
            .w_h(420.0/4.0, 480.0/4.0)
            .set(self.ids.bag, ui_widgets);


        // Attempt to make resizable image based container for buttons
        // Maybe this could be made into a Widget type if it is useful
        if self.menu_open {
            let num = self.ids.menu_buttons.len();
            // Canvas to hold everything together
            Canvas::new()
                .w_h(106.0, 54.0 + num as f64 * 30.0)
                .middle_of(ui_widgets.window)
                .set(self.ids.menu_canvas, ui_widgets);
            // Top of menu
            Image::new(self.imgs.menu_top)
                .w_h(106.0, 28.0)
                .mid_top_of(self.ids.menu_canvas)
                // Does not work because of bug in conrod, but above line is equivalent
                //.parent(ids.menu_canvas)
                //.mid_top()
                .set(self.ids.menu_top, ui_widgets);
            // Bottom of Menu
            // Note: conrod defaults to the last used parent
            Image::new(self.imgs.menu_bot)
                .w_h(106.0, 26.0)
                .mid_bottom()
                .set(self.ids.menu_bot, ui_widgets);
            // Midsection background
            Image::new(self.imgs.menu_mid)
                .w_h(106.0, num as f64 * 30.0)
                .mid_bottom_with_margin(26.0)
                .set(self.ids.menu_mid, ui_widgets);
            // Menu buttons
            if num > 0 {
                Button::image(self.imgs.menu_button)
                    .mid_top_with_margin_on(self.ids.menu_mid, 8.0)
                    .w_h(48.0, 20.0)
                    .label(&format!("Button {}", 1))
                    .label_rgb(1.0, 0.4, 1.0)
                    .label_font_size(7)
                    .set(self.ids.menu_buttons[0], ui_widgets);
            }
            for i in 1..num {
                Button::image(self.imgs.menu_button)
                    .down(10.0)
                    .label(&format!("Button {}", i + 1))
                    .label_rgb(1.0, 0.4, 1.0)
                    .label_font_size(7)
                    .set(self.ids.menu_buttons[i], ui_widgets);
            }
        }
    }

    pub fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
    }

    pub fn handle_event(&mut self, input: Input) {
        self.ui.handle_event(input);
    }

    pub fn maintain(&mut self, renderer: &mut Renderer) {
        self.update_layout();
        self.ui.maintain(renderer);
    }

    pub fn render(&self, renderer: &mut Renderer) {
        self.ui.render(renderer);
    }
}
