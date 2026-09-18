//! 导航层：左侧游戏列表（侧边栏）+ 右侧内容区路由。
//!
//! 这里只做「接线 + 展示」：行的增删一律通过 `gtk::ListBox` 的 append / remove_all，
//! 行控件自身即 `adw::ActionRow`（`GtkListBoxRow` 子类），不做手工挂载。
//!
//! P0-6: ListBoxRow 的 widget_name 存储游戏 ID，不再依赖数组下标。

use adw::prelude::*;

use crate::utils::config::ConfigStore;
use crate::widgets;

/// 侧边栏控件集合。
pub struct Sidebar {
    pub page: adw::NavigationPage,
    pub list_box: gtk::ListBox,
    pub empty_label: gtk::Label,
    pub add_button: gtk::Button,
    pub delete_button: gtk::Button,
}

/// 构建左侧游戏列表。
pub fn build_sidebar() -> Sidebar {
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .build();
    list_box.add_css_class("navigation-sidebar");

    let empty_label = gtk::Label::new(Some("暂无游戏配置\n点击右上角「＋」添加"));
    empty_label.add_css_class("dim-label");
    empty_label.set_justify(gtk::Justification::Center);
    empty_label.set_margin_top(24);
    empty_label.set_margin_bottom(24);
    empty_label.set_margin_start(12);
    empty_label.set_margin_end(12);
    empty_label.set_visible(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&list_box);
    content.append(&empty_label);

    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();

    let title = gtk::Label::new(Some("游戏列表"));
    title.add_css_class("heading");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    title.set_hexpand(true);

    let add_button = widgets::icon_button("list-add-symbolic", "添加游戏");
    let delete_button = widgets::icon_button("user-trash-symbolic", "删除选中的游戏");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_margin_top(12);
    header.set_margin_bottom(6);
    header.set_margin_start(12);
    header.set_margin_end(6);
    header.append(&title);
    header.append(&add_button);
    header.append(&delete_button);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    root.append(&scrolled);

    Sidebar {
        page: adw::NavigationPage::new(&root, "游戏列表"),
        list_box,
        empty_label,
        add_button,
        delete_button,
    }
}

/// P0-6: 按当前数据重建列表行，返回 ID → 行 的映射。
///
/// `selected_id` 是当前选中的游戏 ID（而非数组下标）。
/// `is_running` 判断游戏 ID 是否正在运行。
/// 每个 `ListBoxRow` 的 `widget_name` 存储对应游戏 ID，供 `row_selected` 回调读取。
pub fn refresh_list(
    list_box: &gtk::ListBox,
    empty_label: &gtk::Label,
    store: &ConfigStore,
    selected_id: Option<&str>,
    is_running: &dyn Fn(&str) -> bool,
) -> std::collections::HashMap<String, adw::ActionRow> {
    list_box.remove_all();

    let mut rows = std::collections::HashMap::new();
    for game in store.games() {
        let row = adw::ActionRow::builder()
            .title(game.display_name())
            .subtitle(game.subtitle())
            .subtitle_lines(1)
            .build();
        row.set_tooltip_text(Some(&game.subtitle()));

        // P0-6: 把游戏 ID 存入 widget_name，供 row_selected 回调使用
        row.set_widget_name(&game.id);

        let icon = gtk::Image::from_icon_name("applications-games-symbolic");
        icon.add_css_class("dim-label");
        row.add_prefix(&icon);

        if is_running(&game.id) {
            let badge = gtk::Label::new(Some("运行中"));
            badge.add_css_class("success");
            badge.add_css_class("caption");
            row.add_suffix(&badge);
        }

        list_box.append(&row);
        rows.insert(game.id.clone(), row);
    }

    empty_label.set_visible(rows.is_empty());
    list_box.set_visible(!rows.is_empty());

    // 恢复选中
    if let Some(id) = selected_id
        && let Some(row) = rows.get(id)
    {
        list_box.select_row(Some(row));
    }

    rows
}

/// P0-6: 按 ID 更新单行的标题 / 副标题。
pub fn update_row(row: &adw::ActionRow, store: &ConfigStore, id: &str) {
    if let Some(game) = store.game_by_id(id) {
        row.set_title(game.display_name());
        row.set_subtitle(&game.subtitle());
        row.set_tooltip_text(Some(&game.subtitle()));
    }
}
