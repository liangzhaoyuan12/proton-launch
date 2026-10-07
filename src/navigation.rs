//! 导航层：左侧游戏列表（侧边栏）+ 右侧内容区路由。
//!
//! 这里只做「接线 + 展示」：行的增删一律通过 `gtk::ListBox` 的 append / remove_all，
//! 行控件自身即 `adw::ActionRow`（`GtkListBoxRow` 子类），不做手工挂载。
//!
//! P0-6: ListBoxRow 的 widget_name 存储游戏 ID，不再依赖数组下标。

use adw::prelude::*;

use crate::utils::config::ConfigStore;
use crate::widgets;

/// 侧边栏恒在视窗最底部的「手柄状态」行的 `widget_name`
/// （见 A8：入口不进游戏列表，独立固定在侧栏视窗底部，不随列表滚动 / 重建移动）。
pub const GAMEPAD_ROW_ID: &str = "gamepad-state";

/// 侧边栏控件集合。
pub struct Sidebar {
    pub page: adw::NavigationPage,
    pub list_box: gtk::ListBox,
    pub empty_label: gtk::Label,
    pub add_button: gtk::Button,
    pub delete_button: gtk::Button,
    /// 恒在视窗最底部的手柄状态行。
    pub gamepad_row: adw::ActionRow,
    /// 承载手柄状态行的固定区列表（独立于游戏列表，恒贴侧栏视窗底部）。
    pub gamepad_list_box: gtk::ListBox,
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

    let gamepad_row = adw::ActionRow::builder()
        .title("手柄状态")
        .subtitle("实时输入 · 键程当量 · 振动测试")
        .subtitle_lines(1)
        .build();
    gamepad_row.set_widget_name(GAMEPAD_ROW_ID);
    let gamepad_icon = gtk::Image::from_icon_name("input-gamepad-symbolic");
    gamepad_icon.add_css_class("dim-label");
    gamepad_row.add_prefix(&gamepad_icon);

    // A8：手柄状态行放进**独立**的固定区列表，不参与游戏列表的 remove_all() 重建，
    // 因此恒贴侧栏视窗底部（列表滚动 / 增删 / 窗口缩放都不动它）。
    // 单独加 navigation-sidebar 类，保证行样式与上方游戏行一致。
    let gamepad_list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .build();
    gamepad_list_box.add_css_class("navigation-sidebar");
    gamepad_list_box.append(&gamepad_row);

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

    // 侧栏骨架：头部 → 分隔线 → 可滚动游戏列表 → 分隔线 → 底部固定区（手柄状态）。
    // 只有中间的 scrolled 参与伸缩，所以底部固定区恒贴侧栏视窗底部。
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    root.append(&scrolled);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    root.append(&gamepad_list_box);

    Sidebar {
        page: adw::NavigationPage::new(&root, "游戏列表"),
        list_box,
        empty_label,
        add_button,
        delete_button,
        gamepad_row,
        gamepad_list_box,
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
    // 列表本身恒可见：即便一款游戏都没有，侧栏视窗底部还有「手柄状态」固定行（A8），
    // 它在独立的固定区里，不归本函数管，也不会被这里 remove_all() 删掉。
    list_box.set_visible(true);

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
