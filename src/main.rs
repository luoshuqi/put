#![windows_subsystem = "windows"]

use gio::prelude::*;
use gtk::prelude::*;
use gtk::WindowPosition;
use put::Window;
use std::env::args;
use std::rc::Rc;

fn main() {
    let app = gtk::Application::new(None, Default::default()).expect("init failed");

    app.connect_activate(|app| {
        let window = Rc::new(Window::new());
        window.set_application(Some(app));
        window.set_position(WindowPosition::Center);
        window.setup();
        window.show();
    });

    app.run(&args().collect::<Vec<_>>());
}
