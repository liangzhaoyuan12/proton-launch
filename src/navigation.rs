//! 导航层：左侧游戏列表（侧边栏）+ 右侧内容区路由。
//!
//! 这里只做「接线 + 展示」：行的增删一律通过 `gtk::ListBox` 的 append / remove_all，
//! 行控件自身即 `adw::ActionRow`（`GtkListBoxRow` 子类），不做手工挂载。

use gtk::prelude::*;
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

    // 标题 + 操作按钮
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

/// 按当前数据重建列表行，并恢复选中；返回创建出的行（与游戏索引一一对应）。
///
/// 调用方需在重建期间屏蔽 `row-selected` 信号（`syncing` 标记）。
pub fn refresh_list(
    list_box: &gtk::ListBox,
    empty_label: &gtk::Label,
    store: &ConfigStore,
    selected: Option<usize>,
    is_running: &dyn Fn(usize) -> bool,
) -> Vec<adw::ActionRow> {
    list_box.remove_all();

    let mut rows = Vec::with_capacity(store.len());
    for (idx, game) in store.games().iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(game.display_name(idx))
            .subtitle(game.subtitle())
            .subtitle_lines(1)
            .build();
        row.set_tooltip_text(Some(&game.subtitle()));

        // 条目 = 图标 + 文字（规范 §3.4）
        let icon = gtk::Image::from_icon_name("applications-games-symbolic");
        icon.add_css_class("dim-label");
        row.add_prefix(&icon);

        if is_running(idx) {
            let badge = gtk::Label::new(Some("运行中"));
            badge.add_css_class("success");
            badge.add_css_class("caption");
            row.add_suffix(&badge);
        }

        list_box.append(&row);
        rows.push(row);
    }

    empty_label.set_visible(rows.is_empty());
    list_box.set_visible(!rows.is_empty());

    if let Some(idx) = selected {
        if let Some(row) = rows.get(idx) {
            list_box.select_row(Some(row));
        }
    }

    rows
}

/// 更新单行的标题 / 副标题（改名等轻量刷新，避免整表重建）。
pub fn update_row(row: &adw::ActionRow, store: &ConfigStore, idx: usize) {
    if let Some(game) = store.game(idx) {
        row.set_title(&game.display_name(idx));
        row.set_subtitle(&game.subtitle());
        row.set_tooltip_text(Some(&game.subtitle()));
    }
}
