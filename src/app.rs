//! 应用级状态与接线：数据集散、进程管理、状态栏 / Toast 反馈。
//!
//! 界面控件只通过 [`ConfigHandlers`] 之类的回调与本模块交互，
//! 本模块持有唯一的强引用 [`APP`]，回调里一律只拿 `Weak`，避免引用环。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::Child;
use std::rc::{Rc, Weak};
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

/// 在回调里拿到 `App` 并返回一个值（用于需要返回值的场景，如保存成功判断）。
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
    selected: Option<usize>,
    running: HashMap<usize, RunningState>,
    logs: HashMap<usize, std::sync::Arc<std::sync::Mutex<Vec<String>>>>,

    // 界面
    ui: Ui,
    sidebar_rows: Vec<adw::ActionRow>,
    config_page: Option<ConfigPage>,
    handlers: ConfigHandlers,
    /// 重建侧边栏期间屏蔽 `row-selected`，避免重入借用。
    syncing: Rc<Cell<bool>>,
    /// 内存中有未落盘的修改。
    dirty: Rc<Cell<bool>>,

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
            sidebar_rows: Vec::new(),
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
            me: Weak::new(),
        };
        // P0-4: 配置加载失败时弹窗告知用户
        if let Some(msg) = load_error {
            let window = app.ui.window.clone();
            let msg = msg.clone();
            // 延迟到窗口呈现后再弹窗
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
        {
            let me = self.me.clone();
            let syncing = self.syncing.clone();
            self.ui.list_box.connect_row_selected(move |_, row| {
                if syncing.get() {
                    return;
                }
                let Some(row) = row else { return };
                let idx = row.index();
                if idx < 0 {
                    return;
                }
                with_app(&me, |app| app.select_game(idx as usize));
            });
        }

        // 关于
        {
            let me = self.me.clone();
            self.ui
                .about_button
                .connect_clicked(move |_| with_app(&me, |app| app.show_about()));
        }

        // 进程状态轮询
        {
            let me = self.me.clone();
            gtk::glib::timeout_add_local(Duration::from_millis(200), move || {
                with_app(&me, |app| app.poll_running());
                gtk::glib::ControlFlow::Continue
            });
        }

        // 关闭窗口时若有未保存修改则弹窗确认
        {
            let me = self.me.clone();
            let dirty = self.dirty.clone();
            let window = self.ui.window.clone();
            self.ui.window.connect_close_request(move |_| {
                if !dirty.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                let dialog = adw::MessageDialog::new(
                    Some(&window),
                    Some("有未保存的修改"),
                    Some("当前配置尚未保存到文件，是否保存？"),
                );
                dialog.add_response("cancel", "取消");
                dialog.add_response("discard", "不保存");
                dialog.add_response("save", "保存");
                dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("save"));
                dialog.set_close_response("cancel");
                dialog.present();
                let me = me.clone();
                let window = window.clone();
                dialog.connect_response(None, move |_, response| {
                    match response {
                        "save" => {
                            let saved = with_app_save(&me, |app| {
                                app.sync_current_game();
                                app.store.save().is_ok()
                            });
                            if saved == Some(true) {
                                with_app(&me, |app| app.dirty.set(false));
                                window.close();
                            }
                            // 保存失败时不关闭窗口
                        }
                        "discard" => {
                            with_app(&me, |app| {
                                app.dirty.set(false);
                            });
                            window.close();
                        }
                        _ => {} // 取消
                    }
                });
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
            self.select_game(0);
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
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);
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

    // ─── 侧边栏 / 选中 ──────────────────────────────────────────────────

    fn refresh_sidebar(&mut self) {
        self.syncing.set(true);
        let rows = navigation::refresh_list(
            &self.ui.list_box,
            &self.ui.empty_label,
            &self.store,
            self.selected,
            &|idx| self.running.contains_key(&idx),
        );
        self.syncing.set(false);
        self.sidebar_rows = rows;
    }

    fn select_game(&mut self, idx: usize) {
        if self.store.game(idx).is_none() {
            return;
        }
        // 切换前先把当前页面的未保存修改同步回内存
        self.sync_current_game();
        self.selected = Some(idx);
        self.rebuild_config_page();
        self.show_content_page();
        let name = self
            .store
            .game(idx)
            .map(|g| g.display_name(idx))
            .unwrap_or_default();
        self.set_status(&format!("已选择: {name}"), false);
    }

    fn add_game(&mut self) {
        // P0-7: 添加前先同步当前编辑
        self.sync_current_game();
        let name = format!("新游戏 #{}", self.store.len() + 1);
        let idx = self.store.add_game(GameConfig::new(&name));
        self.selected = Some(idx);
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
        let Some(idx) = self.selected else {
            self.report_error("请先选择要删除的游戏");
            return;
        };
        if self.store.game(idx).is_none() {
            return;
        }
        // P0-7: 删除前先同步当前编辑
        self.sync_current_game();

        // 运行中的进程一并终止
        if let Some(mut state) = self.running.remove(&idx) {
            let pid = state.child.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = state.child.wait();
        }
        // 其余运行索引前移
        let mut remapped = HashMap::new();
        for (key, value) in self.running.drain() {
            remapped.insert(if key > idx { key - 1 } else { key }, value);
        }
        self.running = remapped;

        let name = self
            .store
            .game(idx)
            .map(|g| g.display_name(idx))
            .unwrap_or_default();
        self.store.remove_game(idx);
        self.selected = if self.store.is_empty() {
            None
        } else {
            Some(idx.min(self.store.len() - 1))
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
        let Some(idx) = self.selected else {
            self.config_page = None;
            self.ui.config_bin.set_child(None::<&gtk::Widget>);
            return;
        };
        let Some(game) = self.store.game(idx).cloned() else {
            return;
        };
        let pid = self
            .running
            .get(&idx)
            .filter(|s| !s.done)
            .map(|s| s.child.id() as i32);
        let page = crate::page::config::build(&game, &self.handlers, pid);
        self.ui.config_bin.set_child(Some(&page.root));
        self.config_page = Some(page);
    }

    /// 把界面上的值同步进内存中的配置（不落盘）。
    fn sync_current_game(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(game) = self.store.game(idx).cloned() else {
            return;
        };
        let Some(page) = self.config_page.as_ref() else {
            return;
        };
        let old = game.clone();
        let mut game = game;
        page.apply_to(&mut game);
        if game != old {
            self.store.update_game(idx, game);
            self.dirty.set(true);
        }

        if let Some(row) = self.sidebar_rows.get(idx) {
            navigation::update_row(row, &self.store, idx);
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

    fn run_current(&mut self) {
        self.sync_current_game();
        let Some(idx) = self.selected else { return };
        let Some(game) = self.store.game(idx).cloned() else {
            return;
        };

        if game.executable.trim().is_empty() {
            self.report_error("请先选择可执行文件");
            return;
        }
        // P1-20: done 状态的进程不算运行中
        if self.running.get(&idx).is_some_and(|s| !s.done) {
            self.report_error("该游戏已在运行中");
            return;
        }
        // P1-19: 运行前校验可执行文件是否存在
        if !std::path::Path::new(game.executable.trim()).exists() {
            self.report_error(&format!("可执行文件不存在: {}", game.executable.trim()));
            return;
        }

        let log_buf = self.get_logs(idx);
        // 清空上次日志
        log_buf.lock().unwrap().clear();

        match self.runner.run_game(&game) {
            Ok(mut child) => {
                // 逐行捕获 stdout 到日志
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
                    });
                }
                // 逐行捕获 stderr 到日志
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
                    .insert(idx, RunningState { child, done: false });
                self.refresh_sidebar();
                self.update_running_ui();

                let msg = format!("已启动「{}」(PID {pid})", game.display_name(idx));
                self.set_status(&msg, false);
                self.toast(&msg, false);
            }
            Err(e) => self.report_error(&format!("启动失败: {e}")),
        }
    }

    fn stop_current(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(mut state) = self.running.remove(&idx) else {
            self.report_error("该游戏当前没有运行中的进程");
            return;
        };

        let pid = state.child.id() as i32;
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        thread::sleep(Duration::from_millis(50));
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = state.child.wait();

        self.refresh_sidebar();
        self.update_running_ui();
        let name = self
            .store
            .game(idx)
            .map(|g| g.display_name(idx))
            .unwrap_or_default();
        self.set_status(&format!("「{name}」进程已终止"), false);
        self.toast(&format!("「{name}」已停止"), false);
    }

    fn poll_running(&mut self) {
        let mut finished: Vec<(usize, String, bool)> = Vec::new();

        for (&idx, state) in self.running.iter_mut() {
            if state.done {
                continue;
            }
            match state.child.try_wait() {
                Ok(Some(status)) => {
                    state.done = true;
                    let code = status.code().unwrap_or(-1);
                    let name = self
                        .store
                        .game(idx)
                        .map(|g| g.display_name(idx))
                        .unwrap_or_default();
                    let msg = if code == 0 {
                        format!("「{name}」已正常退出")
                    } else {
                        format!("「{name}」退出，退出码: {code}")
                    };
                    finished.push((idx, msg, code != 0));
                }
                Ok(None) => {}
                Err(e) => {
                    state.done = true;
                    let name = self
                        .store
                        .game(idx)
                        .map(|g| g.display_name(idx))
                        .unwrap_or_default();
                    finished.push((idx, format!("「{name}」进程错误: {e}"), true));
                }
            }
        }

        if finished.is_empty() {
            return;
        }
        for (idx, msg, is_err) in finished {
            self.running.remove(&idx);
            self.set_status(&msg, is_err);
            self.toast(&msg, is_err);
        }
        self.refresh_sidebar();
        self.update_running_ui();
    }

    // ─── 界面状态 ───────────────────────────────────────────────────────

    fn show_welcome(&self) {
        self.ui.stack.set_visible_child_name("welcome");
    }

    fn show_content_page(&self) {
        self.ui.stack.set_visible_child_name("config");
        // 窄窗口下自动折叠为栈式导航：选中后直接进入内容
        if self.ui.split_view.is_collapsed() {
            self.ui.split_view.set_show_content(true);
        }
    }

    fn update_running_ui(&self) {
        let pid = self
            .selected
            .and_then(|idx| self.running.get(&idx))
            .filter(|s| !s.done)
            .map(|s| s.child.id() as i32);
        if let Some(page) = self.config_page.as_ref() {
            page.update_running(pid);
        }
        if self.running.is_empty() {
            self.ui.running_label.set_text("");
        } else {
            self.ui
                .running_label
                .set_text(&format!("运行中: {}", self.running.len()));
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

    fn get_logs(&mut self, idx: usize) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
        self.logs
            .entry(idx)
            .or_insert_with(|| std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
            .clone()
    }

    fn show_log_dialog(&self) {
        let Some(idx) = self.selected else { return };
        let name = self
            .store
            .game(idx)
            .map(|g| g.display_name(idx))
            .unwrap_or_default();

        let logs = match self.logs.get(&idx) {
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

        let text = logs.lock().unwrap().join("\n");
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
