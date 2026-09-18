//! 游戏配置页（展示层）：只做控件组装与取值。
//!
//! 页面不含任何业务逻辑：任何「做事」的请求都通过 [`ConfigHandlers`] 交给上层处理。

use std::rc::Rc;

/// 环境变量行项：(key, 行控件, 是否启用判定器)。
type BuiltEnvItem<'a> = (&'a str, EnvRow, Rc<dyn Fn() -> bool>);

use adw::prelude::*;

use crate::model::GameConfig;
use crate::utils::catalog::{self, ENV_CATEGORIES};
use crate::widgets;

/// 渲染器下拉选项：(启动参数, 显示文本)。
const RENDERERS: &[(&str, &str)] = &[
    ("", "不指定"),
    ("-dx11", "DirectX 11（-dx11）"),
    ("-dx12", "DirectX 12（-dx12）"),
    ("-opengl", "OpenGL（-opengl）"),
    ("-vulkan", "Vulkan（-vulkan）"),
];

/// 额外出现在「基本设置」里的环境变量。
const PROTONPATH: &str = "PROTONPATH";
const DEBUG_CMD: &str = "PROTON_REMOTE_DEBUG_CMD";

/// 页面的动作入口，由外层（`window` / `app`）接线，页面本身不依赖 `App`。
#[derive(Clone)]
pub struct ConfigHandlers {
    /// 任一控件发生变化（同步到内存）
    pub changed: Rc<dyn Fn()>,
    /// 保存到磁盘
    pub save: Rc<dyn Fn()>,
    /// 启动游戏
    pub run: Rc<dyn Fn()>,
    /// 终止游戏
    pub stop: Rc<dyn Fn()>,
    /// 追加一条自定义环境变量
    pub add_var: Rc<dyn Fn()>,
    /// 显示当前游戏的日志
    pub show_logs: Rc<dyn Fn()>,
}

/// 环境变量行的两种形态。
enum EnvRow {
    Bool(adw::SwitchRow),
    Text {
        expander: adw::ExpanderRow,
        entry: adw::EntryRow,
    },
}

/// 自定义环境变量行。
pub struct CustomRow {
    expander: adw::ExpanderRow,
    key_entry: adw::EntryRow,
    value_entry: adw::EntryRow,
}

pub struct ConfigPage {
    pub root: gtk::Widget,
    handlers: ConfigHandlers,
    name_entry: adw::EntryRow,
    exe_entry: adw::EntryRow,
    args_entry: adw::EntryRow,
    workdir_entry: adw::EntryRow,
    proton_entry: adw::EntryRow,
    debug_entry: adw::EntryRow,
    renderer_row: adw::ComboRow,
    env_rows: Vec<(&'static str, EnvRow)>,
    custom_rows: Vec<CustomRow>,
    /// P0-1: 建页时的自定义 key 快照，用于 apply_to 中"先清后写"
    custom_initial: Vec<String>,
    custom_group: adw::PreferencesGroup,
    running_badge: gtk::Label,
    pid_label: gtk::Label,
    stop_button: gtk::Button,
}

/// 「控件变化 → 同步」回调，各控件 connect_* 通用。
fn changed_cb<T>(handlers: &ConfigHandlers) -> impl Fn(&T) + 'static {
    let cb = handlers.changed.clone();
    move |_: &T| cb()
}

/// 顶层构建：按 `game` 的当前值填充全部控件。
pub fn build(game: &GameConfig, handlers: &ConfigHandlers, running_pid: Option<i32>) -> ConfigPage {
    let handlers = handlers.clone();

    let prefs = adw::PreferencesPage::new();
    prefs.set_title("游戏配置");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let mut page = ConfigPage {
        root: root.clone().upcast(),
        handlers: handlers.clone(),
        name_entry: adw::EntryRow::new(),
        exe_entry: adw::EntryRow::new(),
        args_entry: adw::EntryRow::new(),
        workdir_entry: adw::EntryRow::new(),
        proton_entry: adw::EntryRow::new(),
        debug_entry: adw::EntryRow::new(),
        renderer_row: adw::ComboRow::new(),
        env_rows: Vec::new(),
        custom_rows: Vec::new(),
        custom_initial: Vec::new(),
        custom_group: adw::PreferencesGroup::new(),
        running_badge: gtk::Label::new(Some("运行中")),
        pid_label: gtk::Label::new(None),
        stop_button: gtk::Button::with_label("停止"),
    };

    prefs.add(&basic_group(game, &handlers, &page));
    prefs.add(&env_group(game, &handlers, &mut page));
    prefs.add(&custom_group(game, &handlers, &mut page));

    // 顶部操作栏 + 可滚动的偏好设置页
    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&prefs)
        .build();
    root.append(&action_bar(&page, &handlers));
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    root.append(&scrolled);

    page.update_running(running_pid);
    page
}

// ─── 顶部操作栏 ──────────────────────────────────────────────────────────

fn action_bar(page: &ConfigPage, handlers: &ConfigHandlers) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.set_margin_top(12);
    bar.set_margin_bottom(12);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    bar.set_valign(gtk::Align::Center);

    let save = gtk::Button::with_label("保存配置");
    save.set_tooltip_text(Some("把当前配置写入 games.json（Ctrl+S）"));
    {
        let cb = handlers.save.clone();
        save.connect_clicked(move |_| cb());
    }

    let run = gtk::Button::with_label("运行");
    run.add_css_class("suggested-action");
    run.set_tooltip_text(Some("用 umu-run 启动该游戏"));
    {
        let cb = handlers.run.clone();
        run.connect_clicked(move |_| cb());
    }

    let logs = gtk::Button::with_label("日志");
    logs.set_tooltip_text(Some("查看该游戏的运行日志"));
    {
        let cb = handlers.show_logs.clone();
        logs.connect_clicked(move |_| cb());
    }

    page.running_badge.add_css_class("success");
    page.running_badge.add_css_class("caption");
    page.pid_label.add_css_class("dim-label");
    page.pid_label.add_css_class("caption");

    page.stop_button.add_css_class("destructive-action");
    page.stop_button.set_tooltip_text(Some("终止该游戏的进程"));
    {
        let cb = handlers.stop.clone();
        page.stop_button.connect_clicked(move |_| cb());
    }

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    bar.append(&save);
    bar.append(&logs);
    bar.append(&run);
    bar.append(&spacer);
    bar.append(&page.running_badge);
    bar.append(&page.pid_label);
    bar.append(&page.stop_button);
    bar
}

// ─── 基本设置 ────────────────────────────────────────────────────────────

fn basic_group(
    game: &GameConfig,
    handlers: &ConfigHandlers,
    page: &ConfigPage,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("基本设置").build();

    // 名称
    page.name_entry.set_title("名称");
    page.name_entry.set_text(&game.name);
    page.name_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.name_entry);

    // 可执行文件
    page.exe_entry.set_title("可执行文件");
    page.exe_entry.set_text(&game.executable);
    page.exe_entry
        .set_tooltip_text(Some("游戏可执行文件的完整路径"));
    page.exe_entry.add_suffix(&browse_button(
        &page.exe_entry,
        "选择可执行文件",
        false,
        handlers,
    ));
    page.exe_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.exe_entry);

    // 命令行参数
    page.args_entry.set_title("命令行参数");
    page.args_entry.set_text(&game.args);
    page.args_entry
        .set_tooltip_text(Some("按空格拆分为多个参数，追加在可执行文件之后"));
    page.args_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.args_entry);

    // 工作目录
    page.workdir_entry.set_title("工作目录");
    page.workdir_entry.set_text(&game.work_dir);
    page.workdir_entry
        .set_tooltip_text(Some("留空则使用可执行文件所在目录"));
    page.workdir_entry.add_suffix(&browse_button(
        &page.workdir_entry,
        "选择工作目录",
        true,
        handlers,
    ));
    page.workdir_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.workdir_entry);

    // Proton 版本（PROTONPATH）
    page.proton_entry.set_title("Proton 版本");
    page.proton_entry.set_text(
        game.env_vars
            .get(PROTONPATH)
            .map(String::as_str)
            .unwrap_or(""),
    );
    page.proton_entry
        .set_tooltip_text(Some("Proton 路径或版本号，如 GE-Proton9-5"));
    page.proton_entry.add_suffix(&browse_button(
        &page.proton_entry,
        "选择 Proton 目录",
        true,
        handlers,
    ));
    page.proton_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.proton_entry);

    // 修改器 / 注入器（PROTON_REMOTE_DEBUG_CMD）
    page.debug_entry.set_title("修改器 / 注入器");
    page.debug_entry.set_text(
        game.env_vars
            .get(DEBUG_CMD)
            .map(String::as_str)
            .unwrap_or(""),
    );
    page.debug_entry
        .set_tooltip_text(Some("启动前注入的可执行文件路径（留空不注入）"));
    page.debug_entry.add_suffix(&browse_button(
        &page.debug_entry,
        "选择修改器 / 注入器",
        false,
        handlers,
    ));
    page.debug_entry
        .connect_changed(changed_cb::<adw::EntryRow>(handlers));
    group.add(&page.debug_entry);

    // 渲染器
    let labels: Vec<&str> = RENDERERS.iter().map(|(_, label)| *label).collect();
    let model = gtk::StringList::new(&labels);
    page.renderer_row.set_title("渲染器");
    page.renderer_row
        .set_subtitle("启动参数，并非所有游戏都支持");
    page.renderer_row.set_model(Some(&model));
    // P1-18: 找不到匹配时选第一个（"不指定"）而非静默变成 -vulkan
    let selected = RENDERERS
        .iter()
        .position(|(arg, _)| *arg == game.renderer)
        .unwrap_or(0) as u32;
    page.renderer_row.set_selected(selected);
    page.renderer_row
        .connect_selected_notify(changed_cb::<adw::ComboRow>(handlers));
    group.add(&page.renderer_row);

    group
}

fn browse_button(
    row: &adw::EntryRow,
    tooltip: &'static str,
    folder: bool,
    handlers: &ConfigHandlers,
) -> gtk::Button {
    let icon = if folder {
        "folder-open-symbolic"
    } else {
        "document-open-symbolic"
    };
    let button = widgets::icon_button(icon, tooltip);
    let target = row.clone();
    let cb = handlers.changed.clone();
    button.connect_clicked(move |_| widgets::pick_path(&target, tooltip, folder, cb.clone()));
    button
}

// ─── 环境变量 ────────────────────────────────────────────────────────────

fn env_group(
    game: &GameConfig,
    handlers: &ConfigHandlers,
    page: &mut ConfigPage,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("环境变量")
        .description("勾选并填写需要的环境变量，留空则不设置")
        .build();

    for category in ENV_CATEGORIES {
        let total = category.vars.len();

        let expander = adw::ExpanderRow::builder().title(category.name).build();

        // 本分类构建出的行 + 每行「当前是否启用」的判定器（用于刷新计数副标题）。
        let mut built: Vec<BuiltEnvItem> = Vec::new();

        for def in category.vars {
            let key = def.key;
            let (row, checker): (EnvRow, Rc<dyn Fn() -> bool>) = match game.env_vars.get(key) {
                Some(value) if catalog::is_enabled(key, value) => {
                    if catalog::is_bool_toggle(key) {
                        let r = widgets::bool_env_row(def.label, key, true);
                        let c = r.clone();
                        (EnvRow::Bool(r), Rc::new(move || c.is_active()))
                    } else {
                        let browse = browse_hint(key);
                        let (r, entry) = widgets::text_env_row(
                            def.label,
                            key,
                            value,
                            browse,
                            handlers.changed.clone(),
                        );
                        let c = r.clone();
                        (
                            EnvRow::Text { expander: r, entry },
                            Rc::new(move || c.enables_expansion()),
                        )
                    }
                }
                _ => {
                    if catalog::is_bool_toggle(key) {
                        let r = widgets::bool_env_row(def.label, key, false);
                        let c = r.clone();
                        (EnvRow::Bool(r), Rc::new(move || c.is_active()))
                    } else {
                        let browse = browse_hint(key);
                        let (r, entry) = widgets::text_env_row(
                            def.label,
                            key,
                            "",
                            browse,
                            handlers.changed.clone(),
                        );
                        let c = r.clone();
                        (
                            EnvRow::Text { expander: r, entry },
                            Rc::new(move || c.enables_expansion()),
                        )
                    }
                }
            };
            // 行挂进分类折叠行
            match &row {
                EnvRow::Bool(s) => expander.add_row(s),
                EnvRow::Text { expander: ex, .. } => expander.add_row(ex),
            }
            built.push((key, row, checker));
        }

        // 本分类各行的「是否启用」判定器集合，供刷新计数。
        let checkers: Vec<Rc<dyn Fn() -> bool>> = built.iter().map(|(_, _, c)| c.clone()).collect();
        let set_count = checkers.iter().filter(|f| f()).count();
        expander.set_subtitle(&format!("已设置 {set_count}/{total}"));
        expander.set_expanded(set_count > 0);

        // 切换任意开关时刷新「已设置 X/N」副标题，同时同步内存。
        let refresh: Rc<dyn Fn()> = Rc::new({
            let expander = expander.clone();
            move || {
                let n = checkers.iter().filter(|f| f()).count();
                expander.set_subtitle(&format!("已设置 {n}/{total}"));
            }
        });

        for (key, row, _) in built {
            match &row {
                EnvRow::Bool(switch) => {
                    let refresh = refresh.clone();
                    let cb = handlers.changed.clone();
                    switch.connect_active_notify(move |_| {
                        refresh();
                        cb();
                    });
                }
                EnvRow::Text {
                    expander: ex,
                    entry,
                } => {
                    let refresh = refresh.clone();
                    let cb = handlers.changed.clone();
                    ex.connect_enable_expansion_notify(move |_| {
                        refresh();
                        cb();
                    });
                    let cb = handlers.changed.clone();
                    entry.connect_changed(move |_| cb());
                }
            }
            page.env_rows.push((key, row));
        }

        group.add(&expander);
    }

    group
}

fn browse_hint(key: &str) -> Option<(&'static str, bool)> {
    if catalog::is_file_key(key) {
        Some(("选择文件", false))
    } else if catalog::is_dir_key(key) {
        Some(("选择目录", true))
    } else {
        None
    }
}

// ─── 自定义环境变量 ──────────────────────────────────────────────────────

fn custom_group(
    game: &GameConfig,
    handlers: &ConfigHandlers,
    page: &mut ConfigPage,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("自定义环境变量")
        .description("内置清单之外的键值对，会原样传给 umu-run")
        .build();

    let add_button = widgets::icon_button("list-add-symbolic", "添加自定义环境变量");
    {
        let cb = handlers.add_var.clone();
        add_button.connect_clicked(move |_| cb());
    }
    group.set_header_suffix(Some(&add_button));

    for (key, value) in &game.env_vars {
        if catalog::is_known_key(key) {
            continue;
        }
        // P0-1: 记录建页时已存在的自定义 key
        page.custom_initial.push(key.clone());
        let row = CustomRow::build(key, value, &group, handlers);
        group.add(&row.expander);
        page.custom_rows.push(row);
    }

    page.custom_group = group.clone();
    group
}

impl CustomRow {
    fn build(
        key: &str,
        value: &str,
        group: &adw::PreferencesGroup,
        handlers: &ConfigHandlers,
    ) -> Self {
        let key_entry = adw::EntryRow::builder().title("变量名").build();
        key_entry.set_text(key);
        key_entry.set_tooltip_text(Some("环境变量名，例如 DXVK_ASYNC"));

        let value_entry = adw::EntryRow::builder().title("值").build();
        value_entry.set_text(value);

        let expander = adw::ExpanderRow::builder()
            .title(display_key(key))
            .subtitle("自定义变量")
            .build();
        expander.set_expanded(true);
        expander.add_row(&key_entry);
        expander.add_row(&value_entry);

        let delete = widgets::icon_button("user-trash-symbolic", "删除该变量");
        expander.add_suffix(&delete);

        // 标题跟随变量名
        {
            let expander = expander.clone();
            key_entry.connect_changed(move |entry| {
                expander.set_title(display_key(entry.text().as_str()));
            });
        }
        key_entry.connect_changed(changed_cb::<adw::EntryRow>(handlers));
        value_entry.connect_changed(changed_cb::<adw::EntryRow>(handlers));

        // 删除：从分组移除（parent 置空后不再参与取值），并触发一次同步
        {
            let group = group.clone();
            let expander = expander.clone();
            let cb = handlers.changed.clone();
            delete.connect_clicked(move |_| {
                group.remove(&expander);
                cb();
            });
        }

        CustomRow {
            expander,
            key_entry,
            value_entry,
        }
    }
}

fn display_key(key: &str) -> &str {
    if key.trim().is_empty() {
        "新变量"
    } else {
        key
    }
}

// ─── 取值 / 状态刷新 ─────────────────────────────────────────────────────

impl ConfigPage {
    /// 追加一条空的自定义变量（由分组标题栏的「＋」按钮触发）。
    ///
    /// 空变量名在 [`ConfigPage::apply_to`] 中会被跳过，调用方随后同步一次即可。
    pub fn add_custom_var(&mut self) {
        let row = CustomRow::build("", "", &self.custom_group, &self.handlers);
        self.custom_group.add(&row.expander);
        self.custom_rows.push(row);
    }

    /// 把界面上的值写回 `game`（不落盘）。
    pub fn apply_to(&self, game: &mut GameConfig) {
        game.name = self.name_entry.text().to_string();
        game.executable = self.exe_entry.text().to_string();
        game.args = self.args_entry.text().to_string();
        game.work_dir = self.workdir_entry.text().to_string();

        // P1-18: renderer 无效值时保持空字符串
        let selected = self.renderer_row.selected();
        game.renderer =
            if selected == gtk::INVALID_LIST_POSITION || selected >= RENDERERS.len() as u32 {
                String::new()
            } else {
                RENDERERS[selected as usize].0.to_string()
            };

        set_or_remove(
            &mut game.env_vars,
            PROTONPATH,
            self.proton_entry.text().as_str(),
        );
        set_or_remove(
            &mut game.env_vars,
            DEBUG_CMD,
            self.debug_entry.text().as_str(),
        );

        for (key, row) in &self.env_rows {
            match row {
                EnvRow::Bool(switch) => {
                    if switch.is_active() {
                        game.env_vars.insert((*key).to_string(), "1".to_string());
                    } else {
                        game.env_vars.remove(*key);
                    }
                }
                EnvRow::Text { expander, entry } => {
                    if expander.enables_expansion() {
                        game.env_vars
                            .insert((*key).to_string(), entry.text().to_string());
                    } else {
                        game.env_vars.remove(*key);
                    }
                }
            }
        }

        // P0-1: 先清空所有初始自定义 key，再写入现存行（确保删除生效）
        for k in &self.custom_initial {
            game.env_vars.remove(k);
        }
        for row in &self.custom_rows {
            // 已被删除的行：从分组移除后 parent 为空，不再取值
            if row.expander.parent().is_none() {
                continue;
            }
            let key = row.key_entry.text();
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            game.env_vars
                .insert(key.to_string(), row.value_entry.text().to_string());
        }
    }

    /// 刷新「运行中」相关控件的显示。
    pub fn update_running(&self, pid: Option<i32>) {
        let running = pid.is_some();
        self.running_badge.set_visible(running);
        self.pid_label.set_visible(running);
        self.stop_button.set_visible(running);
        if let Some(pid) = pid {
            self.pid_label.set_text(&format!("PID: {pid}"));
        } else {
            self.pid_label.set_text("");
        }
    }
}

fn set_or_remove(env: &mut std::collections::HashMap<String, String>, key: &str, value: &str) {
    if value.trim().is_empty() {
        env.remove(key);
    } else {
        env.insert(key.to_string(), value.to_string());
    }
}
