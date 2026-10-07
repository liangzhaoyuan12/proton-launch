//! 应用级状态与接线：数据集散、进程管理、状态栏 / Toast 反馈。
//!
//! 界面控件只通过 [`ConfigHandlers`] 之类的回调与本模块交互，
//! 本模块持有唯一的强引用 [`APP`]，回调里一律只拿 `Weak`，避免引用环。

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use crate::model::GameConfig;
use crate::navigation;
use crate::page::config::{ConfigHandlers, ConfigPage};
use crate::utils::config::ConfigStore;
use crate::utils::gamepad::{self, GamepadInfo};
use crate::utils::gamepad_ff::{FfWriter, Motor, PULSE_MS, magnitudes};
use crate::utils::gamepad_input::{InputMsg, InputReader};
use crate::utils::notify;
use crate::utils::runner::Runner;

/// 运行中的进程。
struct RunningState {
    child: Child,
    done: bool,
}

/// P1-3: 日志环形缓冲，防止长时间运行导致 OOM。
const LOG_MAX_LINES: usize = 5000;
const LOG_DISPLAY_LINES: usize = 2000;

struct LogBuffer {
    lines: Vec<String>,
    truncated: bool,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            truncated: false,
        }
    }

    fn push(&mut self, line: String) {
        if self.lines.len() >= LOG_MAX_LINES {
            // 丢弃最前 10% 并插入截断标记
            let drop = LOG_MAX_LINES / 10;
            self.lines.drain(..drop);
            self.truncated = true;
            self.lines
                .push("…… 日志已截断，更早的行已丢弃 ……".to_string());
        }
        self.lines.push(line);
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.truncated = false;
    }

    /// 返回最后 N 行用于显示。
    fn tail(&self, n: usize) -> String {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].join("\n")
    }
}

/// 窗口内各控件的句柄（由 `window` 构建后交给 `App`）。
pub struct Ui {
    pub window: adw::ApplicationWindow,
    pub toast_overlay: adw::ToastOverlay,
    pub split_view: adw::NavigationSplitView,
    pub list_box: gtk::ListBox,
    pub empty_label: gtk::Label,
    pub stack: gtk::Stack,
    pub config_bin: adw::Bin,
    pub status_label: gtk::Label,
    pub running_label: gtk::Label,
    pub gamepad_indicator: Rc<crate::widgets::gamepad_indicator::GamepadIndicator>,
    pub add_button: gtk::Button,
    pub delete_button: gtk::Button,
    pub about_button: gtk::Button,
    /// 恒在侧栏视窗最底部的「手柄状态」行（A8）。
    pub gamepad_row: adw::ActionRow,
    /// 承载该行的固定区列表（独立于游戏列表 `list_box`，不参与列表重建）。
    pub gamepad_list_box: gtk::ListBox,
}

thread_local! {
    /// 唯一强引用：保证 `App` 与全部回调句柄在整个进程生命周期内存活。
    static APP: RefCell<Option<Rc<RefCell<App>>>> = const { RefCell::new(None) };
}

fn noop() -> Rc<dyn Fn()> {
    Rc::new(|| {})
}

/// 由 `window` 调用：构造状态、保活、接上信号、呈现首帧。
pub fn launch(ui: Ui) {
    let state = Rc::new(RefCell::new(App::new(ui)));
    state.borrow_mut().me = Rc::downgrade(&state);
    APP.with(|slot| *slot.borrow_mut() = Some(state.clone()));
    state.borrow_mut().boot();
    // present() 期间保持 `&App` 借用：窗口 show 时 GTK 会做一次初始焦点遍历
    // （`gtk_window_show` → `gtk_window_move_focus`），焦点落到侧栏底部固定区的
    // 「手柄状态」行时 GTK 会**顺带选中该行**（`gtk_list_box_row_focus` →
    // `gtk_list_box_update_selection`），此时行选中回调被 `with_app` 的重入保护丢掉，
    // 切页逻辑不会跑——选中态留到首帧后由 `settle_first_frame` 收敛。
    // 不要改成「先 clone window 再 present」，那样回调会执行、首帧会切到手柄状态页。
    state.borrow().ui.window.present();
    let state_for_idle = state.clone();
    glib::idle_add_local_once(move || {
        if let Ok(mut app) = state_for_idle.try_borrow_mut() {
            app.settle_first_frame();
        }
    });
}

impl App {
    /// 首帧后的收敛（`launch` 在 `present()` 之后用 idle 调一次）。
    ///
    /// 启动瞬间的初始焦点遍历会把焦点送进底部固定区的「手柄状态」行，GTK 焦点进入
    /// `ListBox` 行即选中 → 「游戏行 + 手柄状态行」同时高亮。这里按当前页把两个
    /// ListBox 的选中态对齐回「同一时刻只有一条高亮」，并把焦点交回当前页对应的行
    /// （否则键盘焦点停在手柄行上，回车/空格会直接切页）。
    fn settle_first_frame(&mut self) {
        self.sync_sidebar_selection();

        let on_gamepad = self.ui.stack.visible_child_name().as_deref() == Some("gamepad");
        let target: Option<gtk::Widget> = if on_gamepad {
            Some(self.ui.gamepad_row.clone().upcast())
        } else if let Some(id) = self.selected.as_deref()
            && let Some(row) = self.sidebar_rows.get(id)
        {
            Some(row.clone().upcast())
        } else {
            // 空列表（欢迎页）：焦点给「添加游戏」按钮
            Some(self.ui.add_button.clone().upcast())
        };
        if let Some(target) = target {
            gtk::prelude::GtkWindowExt::set_focus(&self.ui.window, Some(&target));
        }
        eprintln!(
            "[SEL] settle_first_frame: 游戏行={:?} 手柄行={:?}",
            self.ui
                .list_box
                .selected_row()
                .map(|r| r.widget_name().to_string()),
            self.ui.gamepad_list_box.selected_row().is_some()
        );
    }

    /// 把两个 ListBox（游戏列表 / 底部固定区）的选中态按当前页对齐为
    /// 「同一时刻只有一条高亮」：非手柄页反选手柄行、补选当前游戏行；手柄页反之。
    fn sync_sidebar_selection(&mut self) {
        let on_gamepad = self.ui.stack.visible_child_name().as_deref() == Some("gamepad");
        // syncing 屏蔽游戏列表的 row_selected → select_game 重入
        self.syncing.set(true);
        if on_gamepad {
            self.ui.list_box.select_row(None::<&gtk::ListBoxRow>);
            if self.ui.gamepad_list_box.selected_row().is_none() {
                self.ui
                    .gamepad_list_box
                    .select_row(Some(&self.ui.gamepad_row));
            }
        } else {
            self.ui
                .gamepad_list_box
                .select_row(None::<&gtk::ListBoxRow>);
            let row = self
                .selected
                .as_deref()
                .and_then(|id| self.sidebar_rows.get(id));
            if let Some(row) = row
                && self.ui.list_box.selected_row().is_none()
            {
                self.ui.list_box.select_row(Some(row));
            }
        }
        self.syncing.set(false);
    }
}

/// 在回调里拿到 `App`：借用失败（重入）时安静返回，不 panic。
fn with_app<F: FnOnce(&mut App)>(weak: &Weak<RefCell<App>>, f: F) {
    if let Some(app) = weak.upgrade()
        && let Ok(mut app) = app.try_borrow_mut()
    {
        f(&mut app);
    }
}

/// 在回调里拿到 `App` 并返回一个值。
fn with_app_save<R, F: FnOnce(&mut App) -> R>(weak: &Weak<RefCell<App>>, f: F) -> Option<R> {
    if let Some(app) = weak.upgrade()
        && let Ok(mut app) = app.try_borrow_mut()
    {
        return Some(f(&mut app));
    }
    None
}

pub struct App {
    // 数据
    store: ConfigStore,
    runner: Runner,
    /// P0-6: 用游戏 ID 做主键，不再用数组下标。
    selected: Option<String>,
    running: HashMap<String, RunningState>,
    /// P1-3: 使用环形缓冲替代无限 Vec。
    logs: HashMap<String, Arc<Mutex<LogBuffer>>>,

    /// 已连接手柄，主键为 event 节点（多手柄各自独立）。
    gamepads: HashMap<String, GamepadInfo>,
    /// 热插拔监听线程（软件存活期内存在，退出即停）。
    gamepad_monitor: Option<gamepad::Monitor>,
    /// 取走监听线程快照的主循环定时器。
    gamepad_source: Option<glib::SourceId>,
    /// 「手柄状态」页（GOAL.md P2 起）。
    gamepad_page: Option<crate::page::gamepad::GamepadPage>,
    /// 当前在页面下拉里选中的设备下标。
    gamepad_index: Option<u32>,
    /// P3 手柄状态页的输入读取：通道 + 读取线程 + 主循环定时器。
    gamepad_input_rx: Option<std::sync::mpsc::Receiver<InputMsg>>,
    gamepad_reader: Option<InputReader>,
    gamepad_input_source: Option<glib::SourceId>,
    /// 正在读取的 event 节点（防止重复启动读取线程）。
    gamepad_node: Option<String>,
    /// P5 振动：写入句柄（懒打开，换设备 / 断开即丢）。
    gamepad_ff: Option<FfWriter>,
    /// P5 振动：纪元 —— 每次开始/停止都递增，让过期的定时停失效。
    rumble_epoch: u64,
    /// P5 振动：当前是否正在振动（停止日志去重用）。
    rumble_active: bool,

    // 界面
    ui: Ui,
    sidebar_rows: HashMap<String, adw::ActionRow>,
    config_page: Option<ConfigPage>,
    handlers: ConfigHandlers,
    syncing: Rc<Cell<bool>>,
    dirty: Rc<Cell<bool>>,

    // P1-1: 进程轮询定时器
    poll_source: Option<glib::SourceId>,

    me: Weak<RefCell<App>>,
}

impl App {
    fn new(ui: Ui) -> Self {
        let (store, load_error) = ConfigStore::new();
        let app = App {
            store,
            runner: Runner::new(),
            selected: None,
            running: HashMap::new(),
            logs: HashMap::new(),
            gamepads: HashMap::new(),
            gamepad_monitor: None,
            gamepad_source: None,
            gamepad_page: None,
            gamepad_index: None,
            gamepad_input_rx: None,
            gamepad_reader: None,
            gamepad_input_source: None,
            gamepad_node: None,
            gamepad_ff: None,
            rumble_epoch: 0,
            rumble_active: false,
            ui,
            sidebar_rows: HashMap::new(),
            config_page: None,
            handlers: ConfigHandlers {
                changed: noop(),
                save: noop(),
                run: noop(),
                stop: noop(),
                add_var: noop(),
                show_logs: noop(),
            },
            syncing: Rc::new(Cell::new(false)),
            dirty: Rc::new(Cell::new(false)),
            poll_source: None,
            me: Weak::new(),
        };
        // P0-4: 配置加载失败时弹窗告知用户
        if let Some(msg) = load_error {
            let window = app.ui.window.clone();
            let msg = msg.clone();
            let _ = glib::idle_add_local(move || {
                let dialog =
                    adw::MessageDialog::new(Some(&window), Some("配置文件损坏"), Some(&msg));
                dialog.add_response("ok", "确定");
                dialog.present();
                glib::ControlFlow::Break
            });
        }
        app
    }

    // ─── 启动 ────────────────────────────────────────────────────────────

    fn boot(&mut self) {
        self.handlers = self.build_handlers();

        // GOAL.md P2: 「手柄状态」页挂到内容区 Stack（页面本身只组装控件）
        {
            let me = self.me.clone();
            let me_device = me.clone();
            let me_press = me.clone();
            let handlers = crate::page::gamepad::GamepadHandlers {
                device_selected: Rc::new(move |index| {
                    with_app(&me_device, |app| app.on_device_selected(index));
                }),
                // P5：页面只发意图；上传/播放、1s 与 2s 定时停都在 app 层
                rumble_press: Rc::new(move |motor, pct| {
                    with_app(&me_press, |app| app.on_rumble_press(motor, pct));
                }),
                rumble_release: Rc::new(move || {
                    with_app(&me, |app| app.on_rumble_release());
                }),
            };
            let page = crate::page::gamepad::build(handlers);
            self.ui.stack.add_named(&page.root, Some("gamepad"));
            self.gamepad_page = Some(page);
        }

        // 侧边栏操作
        {
            let me = self.me.clone();
            self.ui
                .add_button
                .connect_clicked(move |_| with_app(&me, |app| app.add_game()));
        }
        {
            let me = self.me.clone();
            self.ui
                .delete_button
                .connect_clicked(move |_| with_app(&me, |app| app.delete_selected()));
        }
        // P0-6: 从 ListBoxRow 的 name 属性读取游戏 ID，而非 index
        {
            let me = self.me.clone();
            let syncing = self.syncing.clone();
            self.ui.list_box.connect_row_selected(move |_, row| {
                if syncing.get() {
                    return;
                }
                let Some(row) = row else {
                    eprintln!("[SEL] main list row_selected(None)");
                    return;
                };
                let widget_name = row.widget_name().to_string();
                eprintln!("[SEL] main list row_selected({widget_name})");
                let Some(game_id) = widget_name.into() else {
                    return;
                };
                with_app(&me, |app| app.select_game(&game_id));
            });
        }

        // A8: 底部固定区的手柄状态行（独立 ListBox）→ 切到手柄状态页
        {
            let me = self.me.clone();
            self.ui
                .gamepad_list_box
                .connect_row_selected(move |_, row| {
                    eprintln!(
                        "[SEL] gamepad list row_selected fired, row={:?}",
                        row.as_ref().map(|r| r.widget_name().to_string())
                    );
                    // 只处理「选中本行」：反选（None）是切走页面时的清理动作
                    let Some(row) = row else { return };
                    if row.widget_name() != navigation::GAMEPAD_ROW_ID {
                        return;
                    }
                    with_app(&me, |app| {
                        // 幂等保护：焦点导航把选中落到本行时同样会触发本回调，
                        // 已在本页就直接跳过，避免重复切 Stack 子页。
                        if app.ui.stack.visible_child_name().as_deref() != Some("gamepad") {
                            app.show_gamepad_page();
                        }
                    });
                });
        }

        // 关于
        {
            let me = self.me.clone();
            self.ui
                .about_button
                .connect_clicked(move |_| with_app(&me, |app| app.show_about()));
        }

        // P1-1: 轮询改为按需启停（不再无条件 200ms 常驻）

        // P2-7: 关闭确认框复用 — 用 RefCell 在闭包内缓存，避免循环引用
        {
            let me = self.me.clone();
            let dirty = self.dirty.clone();
            let window = self.ui.window.clone();
            let dialog_slot: Rc<RefCell<Option<adw::MessageDialog>>> = Rc::new(RefCell::new(None));
            self.ui.window.connect_close_request(move |_| {
                if !dirty.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                // 复用已有的对话框
                let dialog = if let Some(ref d) = *dialog_slot.borrow() {
                    d.clone()
                } else {
                    let d = adw::MessageDialog::new(
                        Some(&window),
                        Some("有未保存的修改"),
                        Some("当前配置尚未保存到文件，是否保存？"),
                    );
                    d.add_response("cancel", "取消");
                    d.add_response("discard", "不保存");
                    d.add_response("save", "保存");
                    d.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                    d.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                    d.set_default_response(Some("save"));
                    d.set_close_response("cancel");
                    let me = me.clone();
                    let window = window.clone();
                    d.connect_response(None, move |_, response| match response {
                        "save" => {
                            let saved = with_app_save(&me, |app| {
                                app.sync_current_game();
                                app.store.save().is_ok()
                            });
                            if saved == Some(true) {
                                with_app(&me, |app| app.dirty.set(false));
                                window.close();
                            }
                        }
                        "discard" => {
                            with_app(&me, |app| {
                                app.dirty.set(false);
                            });
                            window.close();
                        }
                        _ => {}
                    });
                    *dialog_slot.borrow_mut() = Some(d.clone());
                    d
                };
                dialog.present();
                gtk::glib::Propagation::Stop
            });
        }

        self.install_shortcuts();

        // 空状态页
        let welcome = crate::page::welcome::build(self.action(|app| app.add_game()));
        self.ui.stack.add_named(&welcome, Some("welcome"));

        // 释放内嵌的 umu-run
        if let Err(e) = self.runner.extract_umu() {
            eprintln!("警告: umu-run 释放失败: {e}");
        }

        // 首帧
        if self.store.is_empty() {
            self.config_page = None;
            self.ui.config_bin.set_child(None::<&gtk::Widget>);
            self.show_welcome();
        } else {
            let first_id = self.store.games()[0].id.clone();
            self.select_game(&first_id);
        }
        self.refresh_sidebar();
        self.update_running_ui();
        self.start_gamepad_monitor();
    }

    // ─── 手柄连接状态 ────────────────────────────────────────────────────

    /// 启动手柄监测：先把**当前已连接**的手柄渲染进右下角（不弹通知——
    /// 软件打开前就插着的手柄只展示），随后监听热插拔。
    fn start_gamepad_monitor(&mut self) {
        let initial = gamepad::scan();
        self.apply_gamepads(initial.clone(), false);
        self.gamepad_monitor = Some(gamepad::Monitor::start(initial));

        let me = self.me.clone();
        self.gamepad_source = Some(glib::timeout_add_local(
            Duration::from_millis(400),
            move || {
                with_app(&me, |app| app.drain_gamepad_events());
                glib::ControlFlow::Continue
            },
        ));
    }

    /// 主循环取走监听线程攒下的快照（通常为空）。
    fn drain_gamepad_events(&mut self) {
        let snapshots = self
            .gamepad_monitor
            .as_ref()
            .map(|monitor| monitor.drain())
            .unwrap_or_default();
        for snapshot in snapshots {
            self.apply_gamepads(snapshot, true);
        }
    }

    /// 刷新手柄列表并更新右下角计数/卡片。
    ///
    /// `announce` 为真时，对**新插入**的手柄发系统通知 + Toast 并改写状态栏；
    /// 拔出只更新状态栏（不弹窗）。`announce` 为假只用于启动首帧。
    fn apply_gamepads(&mut self, pads: Vec<GamepadInfo>, announce: bool) {
        let before: HashSet<String> = self.gamepads.keys().cloned().collect();
        let now: HashSet<&str> = pads.iter().map(|p| p.id()).collect();
        let added: Vec<&GamepadInfo> = pads.iter().filter(|p| !before.contains(p.id())).collect();
        let removed: Vec<String> = self
            .gamepads
            .values()
            .filter(|g| !now.contains(g.id()))
            .map(|g| g.name.clone())
            .collect();

        self.gamepads = pads
            .iter()
            .map(|p| (p.id().to_string(), p.clone()))
            .collect();
        self.ui.gamepad_indicator.update(&pads);

        // 手柄状态页跟着热插拔走：刷新下拉；仅当正停在该页时切空态/内容态
        if let Some(page) = self.gamepad_page.as_ref() {
            page.set_devices(&self.gamepad_device_names(), self.gamepad_index);
            let on_page = self
                .ui
                .stack
                .visible_child_name()
                .is_some_and(|n| n == "gamepad");
            if on_page {
                page.set_connected(!self.gamepads.is_empty());
            }
        }
        self.ensure_input_reader();

        let total = pads.len();
        if announce && !added.is_empty() {
            let quoted: Vec<String> = added.iter().map(|p| format!("「{}」", p.name)).collect();
            let listed: Vec<String> = added
                .iter()
                .map(|p| format!("{} · {}", p.name, p.protocol.label()))
                .collect();
            let msg = if quoted.len() == 1 {
                format!("{}手柄已连接（当前 {total} 个）", quoted[0])
            } else {
                format!(
                    "已连接 {} 个手柄：{}（当前 {total} 个）",
                    quoted.len(),
                    quoted.join("、")
                )
            };
            self.set_status(&msg, false);
            self.toast(&msg, false);
            notify::desktop(
                "手柄已连接",
                &format!("{}\n当前已连接 {total} 个手柄", listed.join("\n")),
                "input-gamepad-symbolic",
            );
        }
        if announce && !removed.is_empty() {
            // 拔出：状态栏 + Toast + 系统通知，与连接事件对称
            let msg = if removed.len() == 1 {
                format!("「{}」手柄已断开（当前 {total} 个）", removed[0])
            } else {
                format!(
                    "已拔出 {} 个手柄：{}（当前 {total} 个）",
                    removed.len(),
                    removed
                        .iter()
                        .map(|n| format!("「{n}」"))
                        .collect::<Vec<_>>()
                        .join("、")
                )
            };
            self.set_status(&msg, false);
            self.toast(&msg, false);
            notify::desktop(
                "手柄已断开",
                &format!("{}\n当前已连接 {total} 个手柄", removed.join("\n")),
                "input-gamepad-symbolic",
            );
        }
    }

    fn build_handlers(&self) -> ConfigHandlers {
        ConfigHandlers {
            changed: self.action(|app| app.sync_current_game()),
            save: self.action(|app| app.save_current()),
            run: self.action(|app| app.run_current()),
            stop: self.action(|app| app.stop_current()),
            add_var: self.action(|app| app.add_custom_var()),
            show_logs: self.action(|app| app.show_log_dialog()),
        }
    }

    fn action(&self, f: fn(&mut App)) -> Rc<dyn Fn()> {
        let me = self.me.clone();
        Rc::new(move || with_app(&me, f))
    }

    fn install_shortcuts(&self) {
        // P2-8: 使用 Managed 作用域，输入框内打字时不会误触发
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Managed);
        add_shortcut(
            &controller,
            "<Control>s",
            self.action(|app| app.save_current()),
        );
        add_shortcut(&controller, "<Control>n", self.action(|app| app.add_game()));
        add_shortcut(
            &controller,
            "<Control>r",
            self.action(|app| app.run_current()),
        );
        self.ui.window.add_controller(controller);
    }

    // ─── 侧边栏 / 选中（全部用 ID） ───────────────────────────────────

    fn refresh_sidebar(&mut self) {
        self.syncing.set(true);
        // 若当前停在「手柄状态」页，重建后恢复的是手柄状态行的选中状态，
        // 而不是游戏行——否则一次列表刷新就会把选中切回游戏，页面被拽回配置页。
        let on_gamepad = self.ui.stack.visible_child_name().as_deref() == Some("gamepad");
        eprintln!("[SEL] refresh_sidebar on_gamepad={on_gamepad}");
        let rows = navigation::refresh_list(
            &self.ui.list_box,
            &self.ui.empty_label,
            &self.store,
            if on_gamepad {
                None
            } else {
                self.selected.as_deref()
            },
            &|id| self.running.get(id).is_some_and(|s| !s.done),
        );
        // 底部固定区的「手柄状态」行不在游戏列表里，remove_all() 动不到它，
        // 恒贴侧栏视窗底部，这里无需重建（A8）。
        self.syncing.set(false);
        self.sidebar_rows = rows;
    }

    fn select_game(&mut self, id: &str) {
        if self.store.game_by_id(id).is_none() {
            return;
        }
        eprintln!("[SEL] select_game({id})");
        self.sync_current_game();
        self.selected = Some(id.to_string());
        self.rebuild_config_page();
        self.show_content_page();
        let name = self
            .store
            .game_by_id(id)
            .map(|g| g.display_name().to_string())
            .unwrap_or_default();
        self.set_status(&format!("已选择: {name}"), false);
    }

    fn add_game(&mut self) {
        self.sync_current_game();
        let name = format!("新游戏 #{}", self.store.len() + 1);
        let id = self.store.add_game(GameConfig::new(&name));
        self.selected = Some(id.clone());
        // 先切回配置页再重建列表：refresh_sidebar 按「当前页」决定是否恢复游戏行选中，
        // 若仍在手柄页会传 None，导致新行无高亮（且两个 ListBox 各存各的选中态）
        self.rebuild_config_page();
        self.show_content_page();
        self.refresh_sidebar();
        self.set_status(&format!("已添加: {name}"), false);
        match self.store.save() {
            Ok(()) => self.dirty.set(false),
            Err(e) => self.report_error(&format!("保存失败: {e}")),
        }
    }

    fn delete_selected(&mut self) {
        let Some(ref id) = self.selected.clone() else {
            self.report_error("请先选择要删除的游戏");
            return;
        };
        if self.store.game_by_id(id).is_none() {
            return;
        }
        self.sync_current_game();

        // 运行中的进程一并终止
        if let Some(mut state) = self.running.remove(id) {
            let pid = state.child.id() as i32;
            // P2-6: 检查 kill 返回值
            unsafe {
                if libc::kill(-pid, libc::SIGTERM) != 0 {
                    eprintln!("SIGTERM 发送失败: {}", std::io::Error::last_os_error());
                }
            }
            let _ = state.child.wait();
        }
        // P0-6: 用 ID 做主键，无需索引重映射

        let name = self
            .store
            .game_by_id(id)
            .map(|g| g.display_name().to_string())
            .unwrap_or_default();
        self.store.remove_game_by_id(id);

        // 选择相邻游戏
        self.selected = if self.store.is_empty() {
            None
        } else {
            // 选删除位置的下一个（若无则选最后一个）
            let pos = self.store.index_of_id(id).unwrap_or(self.store.len());
            let new_pos = pos.min(self.store.len().saturating_sub(1));
            Some(self.store.games()[new_pos].id.clone())
        };

        // 先切页（配置页 / 空态）再重建列表：refresh_sidebar 按「当前页」决定是否恢复
        // 游戏行选中，顺序反了会让新选中的行没有高亮（两个 ListBox 各存各的选中态）
        if self.selected.is_some() {
            self.rebuild_config_page();
            self.show_content_page();
        } else {
            self.config_page = None;
            self.ui.config_bin.set_child(None::<&gtk::Widget>);
            self.show_welcome();
        }
        self.refresh_sidebar();
        self.update_running_ui();
        self.set_status(&format!("已删除: {name}"), false);

        if let Err(e) = self.store.save() {
            self.report_error(&format!("保存失败: {e}"));
        } else {
            self.dirty.set(false);
        }
    }

    // ─── 配置页 ─────────────────────────────────────────────────────────

    fn rebuild_config_page(&mut self) {
        let Some(ref id) = self.selected else {
            self.config_page = None;
            self.ui.config_bin.set_child(None::<&gtk::Widget>);
            return;
        };
        let Some(game) = self.store.game_by_id(id).cloned() else {
            return;
        };
        let pid = self
            .running
            .get(id)
            .filter(|s| !s.done)
            .map(|s| s.child.id() as i32);
        let page = crate::page::config::build(&game, &self.handlers, pid);
        self.ui.config_bin.set_child(Some(&page.root));
        self.config_page = Some(page);
    }

    /// 把界面上的值同步进内存中的配置（不落盘）。
    fn sync_current_game(&mut self) {
        let Some(ref id) = self.selected.clone() else {
            return;
        };
        let Some(game) = self.store.game_by_id(id).cloned() else {
            return;
        };
        let Some(page) = self.config_page.as_ref() else {
            return;
        };
        let old = game.clone();
        let mut game = game;
        page.apply_to(&mut game);
        if game != old {
            self.store.update_game_by_id(id, game);
            self.dirty.set(true);
        }

        // 更新侧边栏行
        if let Some(row) = self.sidebar_rows.get(id.as_str()) {
            navigation::update_row(row, &self.store, id);
        }
    }

    fn add_custom_var(&mut self) {
        if let Some(page) = self.config_page.as_mut() {
            page.add_custom_var();
        }
        self.sync_current_game();
    }

    fn save_current(&mut self) {
        self.sync_current_game();
        match self.store.save() {
            Ok(()) => {
                self.dirty.set(false);
                self.set_status("配置已保存", false);
                self.toast("配置已保存", false);
            }
            Err(e) => self.report_error(&format!("保存失败: {e}")),
        }
    }

    // ─── 运行 / 终止 ────────────────────────────────────────────────────

    /// P1-1: 确保轮询定时器已启动。
    fn ensure_polling(&mut self) {
        if self.poll_source.is_some() {
            return;
        }
        let me = self.me.clone();
        let source = gtk::glib::timeout_add_local(Duration::from_millis(200), move || {
            with_app(&me, |app| app.poll_running());
            gtk::glib::ControlFlow::Continue
        });
        self.poll_source = Some(source);
    }

    /// P1-1: 无运行中进程时停止轮询。
    fn stop_polling_if_idle(&mut self) {
        let has_active = self.running.values().any(|s| !s.done);
        if !has_active && let Some(src) = self.poll_source.take() {
            src.remove();
        }
    }

    fn run_current(&mut self) {
        self.sync_current_game();
        let Some(ref id) = self.selected.clone() else {
            return;
        };
        let Some(game) = self.store.game_by_id(id).cloned() else {
            return;
        };

        if game.executable.trim().is_empty() {
            self.report_error("请先选择可执行文件");
            return;
        }
        if self.running.get(id.as_str()).is_some_and(|s| !s.done) {
            self.report_error("该游戏已在运行中");
            return;
        }
        if !std::path::Path::new(game.executable.trim()).exists() {
            self.report_error(&format!("可执行文件不存在: {}", game.executable.trim()));
            return;
        }

        let log_buf = self.get_logs(id);
        log_buf.lock().unwrap().clear();

        match self.runner.run_game(&game) {
            Ok(mut child) => {
                // P1-5: 读完日志后关闭管道，防止线程泄漏
                if let Some(out) = child.stdout.take() {
                    let buf = log_buf.clone();
                    thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(out);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => buf.lock().unwrap().push(l),
                                Err(_) => break,
                            }
                        }
                        // 读完后 drop reader 关闭读端
                    });
                }
                if let Some(err) = child.stderr.take() {
                    let buf = log_buf.clone();
                    thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(err);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => buf.lock().unwrap().push(l),
                                Err(_) => break,
                            }
                        }
                    });
                }

                let pid = child.id() as i32;
                self.running
                    .insert(id.clone(), RunningState { child, done: false });
                self.ensure_polling();
                self.refresh_sidebar();
                self.update_running_ui();

                let msg = format!("已启动「{}」(PID {pid})", game.display_name());
                self.set_status(&msg, false);
                self.toast(&msg, false);
            }
            Err(e) => self.report_error(&format!("启动失败: {e}")),
        }
    }

    /// P1-2: 不再阻塞主线程，发信号后由 poll 回收。
    fn stop_current(&mut self) {
        let Some(ref id) = self.selected.clone() else {
            return;
        };
        let Some(state) = self.running.get_mut(id.as_str()) else {
            self.report_error("该游戏当前没有运行中的进程");
            return;
        };
        if state.done {
            self.report_error("该游戏已停止");
            return;
        }

        let pid = state.child.id() as i32;
        // P2-6: 检查 kill 返回值
        unsafe {
            if libc::kill(-pid, libc::SIGTERM) != 0 {
                eprintln!("SIGTERM 发送失败: {}", std::io::Error::last_os_error());
            }
        }
        // 不再 sleep + wait，交给 poll_running 回收
        state.done = true;

        self.refresh_sidebar();
        self.update_running_ui();
        let name = self
            .store
            .game_by_id(id)
            .map(|g| g.display_name().to_string())
            .unwrap_or_default();
        self.set_status(&format!("「{name}」进程已终止"), false);
        self.toast(&format!("「{name}」已停止"), false);
        self.stop_polling_if_idle();
    }

    fn poll_running(&mut self) {
        let mut finished: Vec<(String, String, bool)> = Vec::new();

        for (id, state) in self.running.iter_mut() {
            if state.done {
                continue;
            }
            match state.child.try_wait() {
                Ok(Some(status)) => {
                    state.done = true;
                    let code = status.code().unwrap_or(-1);
                    let name = self
                        .store
                        .game_by_id(id)
                        .map(|g| g.display_name().to_string())
                        .unwrap_or_default();
                    let msg = if code == 0 {
                        format!("「{name}」已正常退出")
                    } else {
                        format!("「{name}」退出，退出码: {code}")
                    };
                    finished.push((id.clone(), msg, code != 0));
                }
                Ok(None) => {}
                Err(e) => {
                    state.done = true;
                    let name = self
                        .store
                        .game_by_id(id)
                        .map(|g| g.display_name().to_string())
                        .unwrap_or_default();
                    finished.push((id.clone(), format!("「{name}」进程错误: {e}"), true));
                }
            }
        }

        if finished.is_empty() {
            return;
        }
        for (id, msg, is_err) in finished {
            self.running.remove(&id);
            self.set_status(&msg, is_err);
            self.toast(&msg, is_err);
        }
        self.refresh_sidebar();
        self.update_running_ui();
        self.stop_polling_if_idle();
    }

    // ─── 界面状态 ───────────────────────────────────────────────────────

    fn show_welcome(&mut self) {
        eprintln!("[SEL] show_welcome");
        // P5：同 show_content_page，切离「手柄状态」页先停振动（A15）
        self.stop_rumble("离开手柄状态页");
        // 底部固定区同步反选（页面已离开手柄状态）
        self.ui
            .gamepad_list_box
            .select_row(None::<&gtk::ListBoxRow>);
        self.ui.stack.set_visible_child_name("welcome");
    }

    /// 切到「手柄状态」页（侧边栏底部入口，A8）。
    fn show_gamepad_page(&mut self) {
        eprintln!("[SEL] show_gamepad_page called");
        if let Some(page) = self.gamepad_page.as_ref() {
            let connected = !self.gamepads.is_empty();
            page.set_connected(connected);
            page.set_devices(&self.gamepad_device_names(), self.gamepad_index);
        }
        // set_devices 的 notify 会因重入保护（try_borrow_mut 失败）被丢弃，这里显式启动
        self.ensure_input_reader();
        // 两个 ListBox 各自独立保留选中态 → 进入手柄页时必须把游戏行反选，
        // 否则「游戏行 + 手柄状态行」会同时高亮（选中只允许一条）。
        self.ui.list_box.select_row(None::<&gtk::ListBoxRow>);
        self.ui.stack.set_visible_child_name("gamepad");
        // 底部固定区同步选中态（点击进入时已被选中，这里只兜底程序化切页的情况；
        // 先判空再选，避免 row_selected 重入）
        if self.ui.gamepad_list_box.selected_row().is_none() {
            self.ui
                .gamepad_list_box
                .select_row(Some(&self.ui.gamepad_row));
        }
        if self.ui.split_view.is_collapsed() {
            self.ui.split_view.set_show_content(true);
        }
        self.set_status("手柄状态", false);
    }

    /// 当前手柄的 event 节点（数值排序，与下拉、显示名同一顺序）。
    fn gamepad_device_nodes(&self) -> Vec<String> {
        let mut nodes: Vec<String> = self.gamepads.keys().cloned().collect();
        nodes.sort_by_key(|n| {
            n.rsplit("event")
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(u32::MAX)
        });
        nodes
    }

    /// 当前已连接手柄的下拉显示名。
    fn gamepad_device_names(&self) -> Vec<String> {
        self.gamepad_device_nodes()
            .iter()
            .map(|n| {
                let pad = &self.gamepads[n];
                format!("{}（{}）", pad.name, pad.event_node)
            })
            .collect()
    }

    /// 下拉选中了第 i 个设备：P3 在这里启停读取线程。
    fn on_device_selected(&mut self, index: usize) {
        if self.gamepad_index == Some(index as u32) && self.gamepad_reader.is_some() {
            return;
        }
        self.gamepad_index = Some(index as u32);
        self.ensure_input_reader();
    }

    /// P3：为当前选中的设备启动读取线程；无设备 / 换设备时先停旧的。
    ///
    /// 读取线程通过 [`InputReader`] 自带的停止标志退出（≤16ms），不阻塞 UI（P3-5）。
    fn ensure_input_reader(&mut self) {
        let nodes = self.gamepad_device_nodes();
        // 首次进入页面时 index 还是 None（set_devices 的 notify 因重入保护被丢弃），
        // 这里兜底选中第一台；无设备则保持 None 并停掉读取。
        if self.gamepad_index.is_none() && !nodes.is_empty() {
            self.gamepad_index = Some(0);
        }
        let node = self
            .gamepad_index
            .and_then(|i| nodes.get(i as usize).cloned());
        let Some(node) = node else {
            self.stop_input_reader();
            return;
        };
        if self.gamepad_node.as_deref() == Some(node.as_str()) {
            return;
        }
        self.stop_input_reader();

        let (rx, reader) = InputReader::spawn(&node);
        self.gamepad_input_rx = Some(rx);
        self.gamepad_reader = Some(reader);
        self.gamepad_node = Some(node.clone());

        // 主循环 16ms 抽一次通道（与读取线程合帧节奏一致，P3-4）
        let me = self.me.clone();
        let source = glib::timeout_add_local(Duration::from_millis(16), move || {
            let alive = with_app_save(&me, |app| {
                app.drain_input();
                app.gamepad_reader.is_some()
            })
            .unwrap_or(false);
            if alive {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
        self.gamepad_input_source = Some(source);
        if let Some(page) = self.gamepad_page.as_ref() {
            page.set_info(&format!("读取中：{node}"));
        }
    }

    /// P3：抽干输入通道并喂给页面（Ready / Frame / Disconnected）。
    fn drain_input(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let Some(rx) = self.gamepad_input_rx.take() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(InputMsg::Ready { caps, snapshot }) => {
                    if let Some(page) = self.gamepad_page.as_ref() {
                        page.set_connected(true);
                        page.set_info(&format!("{} · {}", caps.name, caps.node));
                        // P5：无 FF_RUMBLE 就置灰并说明原因（A15「无能力置灰」）
                        page.set_ff(
                            caps.ff_rumble,
                            if caps.ff_rumble {
                                ""
                            } else {
                                "当前手柄未声明 FF_RUMBLE（EV_FF）能力，无法振动"
                            },
                        );
                        page.update(Some(&caps), Some(&snapshot));
                    }
                }
                Ok(InputMsg::Frame(snap)) => {
                    if let Some(page) = self.gamepad_page.as_ref() {
                        page.update(None, Some(&snap));
                    }
                }
                Ok(InputMsg::Disconnected { node, reason }) => {
                    self.stop_input_reader();
                    if let Some(page) = self.gamepad_page.as_ref() {
                        page.clear();
                        page.set_connected(false);
                        page.set_info("—");
                    }
                    self.set_status(&format!("手柄「{node}」读取终止：{reason}"), true);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stop_input_reader();
                    break;
                }
            }
        }
        if self.gamepad_reader.is_some() {
            self.gamepad_input_rx = Some(rx);
        }
    }

    /// P3：停止读取（换设备 / 断开 / 退出时）。
    fn stop_input_reader(&mut self) {
        // P5：断开 / 切换 / 退出都必须先停振动（A15）
        self.stop_rumble("设备断开或切换");
        self.gamepad_ff = None;
        if let Some(source) = self.gamepad_input_source.take() {
            source.remove();
        }
        self.gamepad_reader = None; // Drop → 置停止标志，线程自行退出
        self.gamepad_input_rx = None;
        self.gamepad_node = None;
    }

    // ─── P5 振动测试（R3 / A15 / D8）───────────────────────────────────────

    /// 按住马达按钮：上传效果并播放；1s 脉冲、2s 硬上限都会自动停。
    ///
    /// 设备侧的停止由 [`Self::stop_rumble`] 统一发；每次开始/停止都会推进
    /// `rumble_epoch`，过期的定时器看到纪元变了就直接跳过，不会误停下一次。
    fn on_rumble_press(&mut self, motor: Motor, pct: u8) {
        let Some(node) = self.gamepad_node.clone() else {
            eprintln!("[振动] 忽略：当前没有正在读取的手柄节点");
            return;
        };
        // 写句柄懒打开：换过设备就重建（节点不同）
        if self.gamepad_ff.as_ref().map(|w| w.node()) != Some(node.as_str()) {
            match FfWriter::open(&node) {
                Ok(w) => self.gamepad_ff = Some(w),
                Err(e) => {
                    eprintln!("[振动] 打开 {node} 失败：{e}");
                    return;
                }
            }
        }
        let (strong, weak) = magnitudes(motor, pct);
        let result = match self.gamepad_ff.as_mut() {
            Some(w) => w.upload(strong, weak).and_then(|_| w.play()),
            None => return,
        };
        if let Err(e) = result {
            eprintln!("[振动] {} {pct}% 启动失败：{e}", motor.name());
            return;
        }
        self.rumble_active = true;
        self.rumble_epoch += 1;
        let epoch = self.rumble_epoch;
        eprintln!(
            "[振动] 开始 · {} · 强度{pct}% · strong=0x{strong:04x} weak=0x{weak:04x} · 脉冲{PULSE_MS}ms / 硬停2000ms",
            motor.name()
        );

        let me = self.me.clone();
        let pulse = u64::from(PULSE_MS);
        glib::timeout_add_local_once(std::time::Duration::from_millis(pulse), move || {
            with_app(&me, |app| app.rumble_timeout(epoch, "1s 脉冲到时"));
        });
        let me = self.me.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(2000), move || {
            with_app(&me, |app| app.rumble_timeout(epoch, "2s 硬上限到时"));
        });
    }

    /// 松开按钮即停（A15「松开停」）。
    fn on_rumble_release(&mut self) {
        self.stop_rumble("松开按钮");
    }

    /// 定时停：只有纪元没变（没有被松开 / 重新按压顶掉）才真正停。
    fn rumble_timeout(&mut self, epoch: u64, why: &str) {
        if self.rumble_epoch == epoch {
            self.stop_rumble(why);
        }
    }

    /// 停止当前振动（幂等）：发停止命令并作废所有待触发的定时停。
    fn stop_rumble(&mut self, why: &str) {
        self.rumble_epoch += 1;
        if !self.rumble_active {
            return;
        }
        self.rumble_active = false;
        if let Some(w) = self.gamepad_ff.as_ref()
            && let Err(e) = w.stop()
        {
            eprintln!("[振动] 停止失败：{e}");
        }
        eprintln!("[振动] 停止 · {why}");
    }

    fn show_content_page(&mut self) {
        eprintln!("[SEL] show_content_page");
        // P5：离开「手柄状态」页即停振动（A15）；不在该页时本调用为空操作
        self.stop_rumble("离开手柄状态页");
        // 底部固定区同步反选，高亮回到当前游戏行
        self.ui
            .gamepad_list_box
            .select_row(None::<&gtk::ListBoxRow>);
        self.ui.stack.set_visible_child_name("config");
        if self.ui.split_view.is_collapsed() {
            self.ui.split_view.set_show_content(true);
        }
    }

    fn update_running_ui(&self) {
        let pid = self
            .selected
            .as_deref()
            .and_then(|id| self.running.get(id))
            .filter(|s| !s.done)
            .map(|s| s.child.id() as i32);
        if let Some(page) = self.config_page.as_ref() {
            page.update_running(pid);
        }
        let active_count = self.running.values().filter(|s| !s.done).count();
        if active_count == 0 {
            self.ui.running_label.set_text("");
        } else {
            self.ui
                .running_label
                .set_text(&format!("运行中: {active_count}"));
        }
    }

    fn set_status(&self, msg: &str, is_err: bool) {
        self.ui.status_label.set_text(msg);
        if is_err {
            self.ui.status_label.add_css_class("error");
        } else {
            self.ui.status_label.remove_css_class("error");
        }
    }

    fn report_error(&self, msg: &str) {
        self.set_status(msg, true);
        self.toast(msg, true);
    }

    fn toast(&self, msg: &str, is_err: bool) {
        let toast = adw::Toast::new(msg);
        toast.set_timeout(3);
        if is_err {
            toast.set_priority(adw::ToastPriority::High);
        }
        self.ui.toast_overlay.add_toast(toast);
    }

    fn get_logs(&mut self, id: &str) -> Arc<Mutex<LogBuffer>> {
        self.logs
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(LogBuffer::new())))
            .clone()
    }

    fn show_log_dialog(&self) {
        let Some(ref id) = self.selected else {
            return;
        };
        let name = self
            .store
            .game_by_id(id)
            .map(|g| g.display_name().to_string())
            .unwrap_or_default();

        let logs = match self.logs.get(id.as_str()) {
            Some(l) => l.clone(),
            None => {
                let dialog = adw::MessageDialog::new(
                    Some(&self.ui.window),
                    Some("运行日志"),
                    Some("该游戏尚未运行，暂无日志"),
                );
                dialog.add_response("close", "关闭");
                dialog.present();
                return;
            }
        };

        // P1-22: 限制显示行数，防止主线程卡顿
        let text = logs.lock().unwrap().tail(LOG_DISPLAY_LINES);
        let dialog = adw::MessageDialog::new(
            Some(&self.ui.window),
            Some(&format!("运行日志 — {name}")),
            None,
        );

        let scrolled = gtk::ScrolledWindow::builder()
            .min_content_width(640)
            .min_content_height(400)
            .build();
        let textview = gtk::TextView::builder()
            .editable(false)
            .monospace(true)
            .left_margin(8)
            .right_margin(8)
            .top_margin(8)
            .bottom_margin(8)
            .build();
        textview.buffer().set_text(&text);
        scrolled.set_child(Some(&textview));
        dialog.set_extra_child(Some(&scrolled));

        dialog.add_response("copy", "复制");
        dialog.set_response_appearance("copy", adw::ResponseAppearance::Suggested);
        dialog.add_response("close", "关闭");
        dialog.set_default_response(Some("close"));
        dialog.set_close_response("close");

        let text_clone = text.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "copy" {
                let clipboard = textview.display().clipboard();
                clipboard.set_text(&text_clone);
            }
        });

        dialog.present();
    }

    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name("Proton 启动管理器")
            .application_icon("proton-launch")
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("liangzhaoyuan12")
            .license_type(gtk::License::MitX11)
            .website("https://github.com/liangzhaoyuan12/proton-launch")
            .build();
        about.add_link(
            "Gitee 仓库",
            "https://gitee.com/liangzhaoyuan12/proton-launch",
        );
        about.add_link(
            "GitHub 仓库",
            "https://github.com/liangzhaoyuan12/proton-launch",
        );
        about.set_transient_for(Some(&self.ui.window));
        about.present();
    }
}

fn add_shortcut(controller: &gtk::ShortcutController, accel: &str, f: Rc<dyn Fn()>) {
    let Some(trigger) = gtk::ShortcutTrigger::parse_string(accel) else {
        return;
    };
    let action = gtk::CallbackAction::new(move |_, _| {
        f();
        gtk::glib::Propagation::Proceed
    });
    controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
}

/// P1-4: 应用退出时终止所有运行中的子进程，防止孤儿进程残留。
pub fn shutdown() {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref()
            && let Ok(mut app) = app.try_borrow_mut()
        {
            // 停掉手柄监听线程：软件不驻留后台，退出即彻底结束
            if let Some(monitor) = app.gamepad_monitor.take() {
                monitor.stop();
            }
            if let Some(source) = app.gamepad_source.take() {
                source.remove();
            }
            // P3:手柄状态页的读取线程与定时器也要停干净
            app.stop_input_reader();
            for (_, mut state) in app.running.drain() {
                if !state.done {
                    let pid = state.child.id() as i32;
                    unsafe {
                        libc::kill(-pid, libc::SIGTERM);
                    }
                }
                let _ = state.child.wait();
            }
        }
    });
}
