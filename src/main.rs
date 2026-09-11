//! 入口：只负责创建 AdwApplication 并绑定 activate。

mod app;
mod model;
mod navigation;
mod page;
mod utils;
mod widgets;
mod window;

use gtk::glib;
use gtk::prelude::*;

fn main() -> glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(window::APP_ID)
        .build();
    application.connect_activate(window::build);
    application.run()
}
