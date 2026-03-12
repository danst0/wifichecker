mod heatmap;
mod models;
mod persistence;
mod services;
mod widgets;
mod window;

use libadwaita::prelude::*;
use libadwaita::Application;

const APP_ID: &str = "io.github.wifichecker";

fn main() {
    env_logger::init();

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(|app| {
        let win = window::Window::new(app);
        win.window.present();
    });

    app.run();
}
