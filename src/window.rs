//! 主窗口外壳：AdwToolbarView + AdwHeaderBar + 底部状态栏 + NavigationSplitView。

use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::app::{self, Ui};
use crate::navigation;

/// 反向域名形式的应用 ID。
pub const APP_ID: &str = "com.protonlaunch.Manager";

/// 构建主窗口（单窗口应用，重复激活只呈现已有窗口）。
pub fn build(application: &adw::Application) {
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }

    // ── 顶部栏 ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::new();

    let title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    title.append(&gtk::Label::new(Some("Proton 启动管理器")));
    let version = gtk::Label::new(Some(concat!("v", env!("CARGO_PKG_VERSION"))));
    version.add_css_class("dim-label");
    title.append(&version);
    header.set_title_widget(Some(&title));

    // ── 侧边栏（游戏列表）────────────────────────────────────────────────
    let sidebar = navigation::build_sidebar();

    // ── 内容区 ───────────────────────────────────────────────────────────
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(250);

    let config_bin = adw::Bin::new();
    stack.add_named(&config_bin, Some("config"));

    let split_view = adw::NavigationSplitView::new();
    split_view.set_min_sidebar_width(200.0);
    split_view.set_max_sidebar_width(360.0);
    split_view.set_sidebar_width_fraction(0.24);
    split_view.set_sidebar(Some(&sidebar.page));
    split_view.set_content(Some(&adw::NavigationPage::new(&stack, "游戏配置")));

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&split_view));

    // ── 窗口骨架 ─────────────────────────────────────────────────────────
    let (status_bar, status_label, running_label) = build_status_bar();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toast_overlay));
    toolbar.add_bottom_bar(&status_bar);

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Proton 启动管理器")
        .default_width(1080)
        .default_height(720)
        .content(&toolbar)
        .build();
    window.set_size_request(480, 400);
    window.set_icon_name(Some("proton-launch"));

    // ── 顶部栏控件 ───────────────────────────────────────────────────────
    let toggle = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .tooltip_text("显示 / 隐藏游戏列表")
        .build();
    toggle.add_css_class("flat");
    toggle.set_visible(false);
    header.pack_start(&toggle);
    {
        let split_view = split_view.clone();
        toggle.connect_toggled(move |button| {
            if split_view.is_collapsed() {
                split_view.set_show_content(!button.is_active());
            }
        });
    }
    // 折叠态才显示汉堡按钮：折叠时 show_content == false 即「正在显示侧边栏」
    // P1-11: 用 WeakRef 避免 split_view/toggle → sync → clone 循环
    {
        let weak_watched = glib::WeakRef::new();
        weak_watched.set(Some(&split_view));
        let weak_toggle = glib::WeakRef::new();
        weak_toggle.set(Some(&toggle));
        let sync: Rc<dyn Fn()> = Rc::new(move || {
            let Some(watched) = weak_watched.upgrade() else {
                return;
            };
            let Some(toggle) = weak_toggle.upgrade() else {
                return;
            };
            let collapsed = watched.is_collapsed();
            toggle.set_visible(collapsed);
            let sidebar_shown = collapsed && !watched.shows_content();
            if toggle.is_active() != sidebar_shown {
                toggle.set_active(sidebar_shown);
            }
        });
        {
            let sync = sync.clone();
            split_view.connect_collapsed_notify(move |_| sync());
        }
        {
            let sync = sync.clone();
            split_view.connect_show_content_notify(move |_| sync());
        }
        sync();
    }

    let about_button = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text("关于")
        .build();
    about_button.add_css_class("flat");
    header.pack_end(&about_button);
    header.pack_end(&build_appearance_menu(application));

    // ── 交给 App 接线 ────────────────────────────────────────────────────
    app::launch(Ui {
        window,
        toast_overlay,
        split_view,
        list_box: sidebar.list_box,
        empty_label: sidebar.empty_label,
        stack,
        config_bin,
        status_label,
        running_label,
        add_button: sidebar.add_button,
        delete_button: sidebar.delete_button,
        about_button,
    });
}

/// 底部状态栏：左侧状态文字，右侧运行计数。
fn build_status_bar() -> (gtk::Box, gtk::Label, gtk::Label) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.set_margin_start(12);
    bar.set_margin_end(12);

    let status = gtk::Label::new(Some("就绪"));
    status.set_xalign(0.0);
    status.set_halign(gtk::Align::Start);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let running = gtk::Label::new(None);
    running.add_css_class("dim-label");

    bar.append(&status);
    bar.append(&running);
    (bar, status, running)
}

/// 主菜单：外观（跟随系统 / 浅色 / 深色）。
fn build_appearance_menu(application: &adw::Application) -> gtk::MenuButton {
    let appearance = gio::Menu::new();
    appearance.append(Some("跟随系统"), Some("app.color-scheme('default')"));
    appearance.append(Some("浅色"), Some("app.color-scheme('light')"));
    appearance.append(Some("深色"), Some("app.color-scheme('dark')"));

    let menu = gio::Menu::new();
    menu.append_submenu(Some("外观"), &appearance);

    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("主菜单")
        .menu_model(&menu)
        .build();
    button.add_css_class("flat");

    let action = gio::SimpleAction::new_stateful(
        "color-scheme",
        Some(glib::VariantTy::STRING),
        &"default".to_variant(),
    );
    action.connect_activate(|action, parameter| {
        let Some(value) = parameter.and_then(|p| p.get::<String>()) else {
            return;
        };
        let scheme = match value.as_str() {
            "light" => adw::ColorScheme::ForceLight,
            "dark" => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
        action.set_state(&value.to_variant());
    });
    application.add_action(&action);

    button
}
