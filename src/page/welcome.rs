//! 空状态页：还没有任何游戏时展示。

use std::rc::Rc;

use gtk::prelude::*;

pub fn build(on_add: Rc<dyn Fn()>) -> adw::StatusPage {
    let page = adw::StatusPage::builder()
        .icon_name("applications-games-symbolic")
        .title("Proton 启动管理器")
        .description("点击下方按钮创建新的游戏配置，或从左侧列表选择一个已有游戏进行编辑。")
        .build();

    let button = gtk::Button::with_label("添加游戏");
    button.add_css_class("suggested-action");
    button.add_css_class("pill");
    button.set_halign(gtk::Align::Center);
    button.set_tooltip_text(Some("创建一份新的游戏配置"));
    button.connect_clicked(move |_| on_add());
    page.set_child(Some(&button));

    page
}
