//! 应用级状态与接线：数据集散、进程管理、状态栏 / Toast 反馈。
//!
//! 界面控件只通过 [`ConfigHandlers`] 之类的回调与本模块交互，
//! 本模块持有唯一的强引用 [`APP`]，回调里一律只拿 `Weak`，避免引用环。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
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
    pub add_button: gtk::Button,
    pub delete_button: gtk::Button,
    pub about_button: gtk::Button,
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
    state.borrow().ui.window.present();
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
                let Some(row) = row else { return };
                let Some(game_id) = row.widget_name().to_string().into() else {
                    return;
                };
                with_app(&me, |app| app.select_game(&game_id));
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
        let rows = navigation::refresh_list(
            &self.ui.list_box,
            &self.ui.empty_label,
            &self.store,
            self.selected.as_deref(),
            &|id| self.running.get(id).is_some_and(|s| !s.done),
        );
        self.syncing.set(false);
        self.sidebar_rows = rows;
    }

    fn select_game(&mut self, id: &str) {
        if self.store.game_by_id(id).is_none() {
            return;
        }
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
        self.refresh_sidebar();
        self.rebuild_config_page();
        self.show_content_page();
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

        self.refresh_sidebar();
        if self.selected.is_some() {
            self.rebuild_config_page();
            self.show_content_page();
        } else {
            self.config_page = None;
            self.ui.config_bin.set_child(None::<&gtk::Widget>);
            self.show_welcome();
        }
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

    fn show_welcome(&self) {
        self.ui.stack.set_visible_child_name("welcome");
    }

    fn show_content_page(&self) {
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
