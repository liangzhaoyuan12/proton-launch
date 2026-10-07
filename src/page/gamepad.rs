//! 手柄状态页（展示层）：实时输入可视化 / 键程当量 / 振动测试。
//!
//! 页面只做控件组装与取值：所有「做事」的请求都通过 [`GamepadHandlers`] 交给上层
//! （`app.rs`）处理，页面本身不打开设备、不写振动命令。
//!
//! 阶段（见 GOAL.md）：
//! - P2：骨架（设备下拉 + 四个分组 + 空态）
//! - P3：输入可视化（`DrawingArea` + `add_tick_callback` 自绘动画，D4）
//! - P4：键程当量（归一化数值 + 刻度条）
//! - P5：振动测试（左右马达 + 扳机）
//!
//! 注：本页不写「构造 GTK 控件」的单元测试——那需要 `gtk::init`，会与
//! `widgets::gamepad_indicator` 的显示测试抢全局初始化；几何与换算的单测在
//! [`crate::widgets::gamepad_viz`] 与 [`crate::utils::gamepad_input`]。
//! 纯 cairo 的离屏渲染测试（[`tests::输入状态_摇杆表盘外不出现飞线`]）不碰控件、
//! 不需要显示连接，只校验 `draw_viz` 的绘制结果。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use adw::prelude::*;

use crate::utils::gamepad_ff::{Motor, PULSE_MS};
use crate::utils::gamepad_input::{DeviceCaps, InputSnapshot};
use crate::widgets::gamepad_viz::{self, GaugeItem, VizState};

/// 页面的动作入口，由 `app.rs` 接线。
#[derive(Clone)]
pub struct GamepadHandlers {
    /// 用户在设备下拉里选中了第 `i` 个设备（i 与 `set_devices` 的顺序一致）。
    pub device_selected: Rc<dyn Fn(usize)>,
    /// P5：按住马达按钮（侧别 + 强度 0..=100）。页面不碰设备，交上层执行。
    pub rumble_press: Rc<dyn Fn(Motor, u8)>,
    /// P5：松开按钮即停（用户主动停止；超时/切页/断开由上层处理）。
    pub rumble_release: Rc<dyn Fn()>,
}

/// 手柄状态页的控件句柄。
pub struct GamepadPage {
    /// 交给 `gtk::Stack` 的页面根。
    pub root: gtk::Stack,
    /// 空态 / 内容 两态切换。
    pub stack: gtk::Stack,
    /// 设备下拉（`adw::ComboRow`）。
    pub device_row: adw::ComboRow,
    /// 连接信息行（设备名 · 节点）。
    pub info_row: adw::ActionRow,
    /// 键程当量列表容器（P4 填充）。
    pub gauge_box: gtk::Box,
    /// 振动测试区（P5）。
    pub motor_box: gtk::Box,
    /// 能力 / 原因说明（无 FF 或未连接时显示为什么不能振动）。
    motor_reason: gtk::Label,
    /// 绘制用的输入状态（`Rc` 共享：绘制回调与 [`GamepadPage::update`] 各持一份）。
    state: Rc<RefCell<VizState>>,
    /// 「键程当量」各行的控件句柄（P4，按当前设备能力重建）。
    gauge_rows: RefCell<Vec<GaugeRowUi>>,
    /// 当前行集合签名（节点 + 轴码），不变就只刷数值、不重建控件（P3-4）。
    gauge_sig: RefCell<String>,
    /// 有新帧时把 tick 回调重新挂上（空闲时 tick 会自行 Break 退出，
    /// 否则 GTK 帧时钟会一直跑 —— 实测空闲 CPU 40%+，P3-4）。
    start_tick: Rc<dyn Fn()>,
}

const EMPTY_PAGE: &str = "empty";
const CONTENT_PAGE: &str = "content";

/// 构建页面。
pub fn build(handlers: GamepadHandlers) -> GamepadPage {
    let state = Rc::new(RefCell::new(VizState::default()));

    // ── 设备分组 ─────────────────────────────────────────────────────────
    let device_row = adw::ComboRow::builder()
        .title("手柄")
        .model(&gtk::StringList::new(&["（无可用设备）"]))
        .build();

    let info_row = adw::ActionRow::builder()
        .title("连接信息")
        .subtitle("—")
        .subtitle_lines(2)
        .build();

    let device_group = adw::PreferencesGroup::builder().title("设备").build();
    device_group.add(&device_row);
    device_group.add(&info_row);

    {
        let device_selected = handlers.device_selected.clone();
        device_row.connect_selected_notify(move |row| {
            let index = row.selected() as usize;
            device_selected(index);
        });
    }

    // ── 输入状态分组（P3）──────────────────────────────────────────────
    let input_area = gtk::DrawingArea::new();
    input_area.set_content_height(200);
    input_area.set_hexpand(true);
    input_area.set_margin_top(8);
    input_area.set_margin_bottom(8);
    input_area.set_margin_start(8);
    input_area.set_margin_end(8);
    {
        let state = state.clone();
        input_area.set_draw_func(move |_, cr, w, h| {
            let mut st = state.borrow_mut();
            draw_viz(cr, &st, w as f64, h as f64);
            // 画完即视为已消费：没有新帧就不重复 queue（P3-4 空闲 CPU）
            st.dirty = false;
        });
    }
    let input_group = adw::PreferencesGroup::builder()
        .title("输入状态")
        .description("按键 / 摇杆 / 扳机的实时可视化")
        .build();
    input_group.add(&input_area);

    // ── 键程当量分组（P4 填充）──────────────────────────────────────────
    let gauge_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    gauge_box.set_margin_top(8);
    gauge_box.set_margin_bottom(8);
    gauge_box.set_margin_start(8);
    gauge_box.set_margin_end(8);
    gauge_box.append(&placeholder("等待手柄输入后显示…"));
    let gauge_group = adw::PreferencesGroup::builder()
        .title("键程当量")
        .description("摇杆 / LT / RT 的归一化行程（0–100%）")
        .build();
    gauge_group.add(&gauge_box);

    // ── 振动测试分组（P5）────────────────────────────────────────────────
    let motor_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    motor_box.set_margin_top(8);
    motor_box.set_margin_bottom(8);
    motor_box.set_margin_start(8);
    motor_box.set_margin_end(8);
    motor_box.set_sensitive(false); // 没连手柄前整体置灰，连上后由 set_ff 放开

    // 强度滑条：A15/D8 默认 30%，打一个默认档标记
    let scale_row = |text: &str| {
        let label = gtk::Label::new(Some(text));
        label.set_xalign(0.0);
        label.set_width_chars(9);
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        scale.set_value(30.0);
        scale.set_draw_value(true);
        scale.set_value_pos(gtk::PositionType::Right);
        scale.set_hexpand(true);
        scale.add_mark(30.0, gtk::PositionType::Bottom, None);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&label);
        row.append(&scale);
        motor_box.append(&row);
        scale
    };
    let motor_scale_left = scale_row("左马达强度");
    let motor_scale_right = scale_row("右马达强度");

    // 「按住振动」：GestureClick 的 pressed/released 即「按下起振 / 松开停」
    let press_button = |text: &str,
                        motor: Motor,
                        scale: &gtk::Scale,
                        press: &Rc<dyn Fn(Motor, u8)>,
                        stop: &Rc<dyn Fn()>| {
        let btn = gtk::Button::with_label(text);
        btn.set_hexpand(true);
        let gesture = gtk::GestureClick::new();
        {
            let press = press.clone();
            let scale = scale.clone();
            gesture.connect_pressed(move |_, _, _, _| {
                let pct = scale.value().round().clamp(0.0, 100.0) as u8;
                press(motor, pct);
            });
        }
        {
            let stop = stop.clone();
            gesture.connect_released(move |_, _, _, _| stop());
        }
        btn.add_controller(gesture);

        // 键盘等价路径：焦点在按钮上时按住 Space/回车同样「按下起振、松开停」，
        // 捕获阶段先吃掉按键，避免按钮自身的 clicked 再插一脚。
        let keyc = gtk::EventControllerKey::new();
        keyc.set_propagation_phase(gtk::PropagationPhase::Capture);
        // 按住状态：X 键盘自动重复只发 KeyPress 不发 KeyRelease，
        // 不挡住的话一次长按会被当成十几次重按（反复上传效果、脉冲计时被顶掉）。
        let holding = Rc::new(std::cell::Cell::new(false));
        {
            let press = press.clone();
            let scale = scale.clone();
            let holding = holding.clone();
            keyc.connect_key_pressed(move |_, key, _, _| {
                let name = key.name().map(|n| n.to_string()).unwrap_or_default();
                if name == "space" || name == "Return" || name == "KP_Enter" {
                    if !holding.get() {
                        holding.set(true);
                        let pct = scale.value().round().clamp(0.0, 100.0) as u8;
                        press(motor, pct);
                    }
                    return gtk::glib::Propagation::Stop;
                }
                gtk::glib::Propagation::Proceed
            });
        }
        {
            let stop = stop.clone();
            let holding = holding.clone();
            keyc.connect_key_released(move |_, key, _, _| {
                let name = key.name().map(|n| n.to_string()).unwrap_or_default();
                if matches!(name.as_str(), "space" | "Return" | "KP_Enter") && holding.get() {
                    holding.set(false);
                    stop();
                }
            });
        }
        btn.add_controller(keyc);
        // 焦点在按住期间被移走（Tab / 点其它地方）时清掉按住标记，
        // 否则回来后的第一次按下会被当成重复事件吞掉。
        let focusc = gtk::EventControllerFocus::new();
        {
            let holding = holding.clone();
            focusc.connect_leave(move |_| holding.set(false));
        }
        btn.add_controller(focusc);
        btn
    };
    let press_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let motor_press_left = press_button(
        "按住振动 · 左马达",
        Motor::Left,
        &motor_scale_left,
        &handlers.rumble_press,
        &handlers.rumble_release,
    );
    let motor_press_right = press_button(
        "按住振动 · 右马达",
        Motor::Right,
        &motor_scale_right,
        &handlers.rumble_press,
        &handlers.rumble_release,
    );
    press_row.append(&motor_press_left);
    press_row.append(&motor_press_right);
    motor_box.append(&press_row);

    // 安全阀说明（A15/D8）
    motor_box.append(&placeholder(&format!(
        "单次 {PULSE_MS} ms 脉冲、2 s 硬上限自动停；切页 / 断开 / 关窗即停"
    )));

    // 扳机按钮：A12(c) 恒置灰 + 原因（F7/F18：上游驱动不开放扳机马达）
    let trig_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let trig_left = gtk::Button::with_label("左扳机振动");
    trig_left.set_sensitive(false);
    trig_left.set_hexpand(true);
    let trig_right = gtk::Button::with_label("右扳机振动");
    trig_right.set_sensitive(false);
    trig_right.set_hexpand(true);
    trig_row.append(&trig_left);
    trig_row.append(&trig_right);
    motor_box.append(&trig_row);
    motor_box.append(&placeholder(
        "扳机振动不可达：xpad 把 GIP 包的扳机字节写死 0x00、hid-microsoft 只发主体马达（GOAL F7/F18），按 A12(c) 置灰。",
    ));
    let motor_reason = placeholder("未连接手柄");
    motor_box.append(&motor_reason);

    let motor_group = adw::PreferencesGroup::builder()
        .title("振动测试")
        .description("左右马达独立测试 / 扳机振动按 A12(c) 置灰")
        .build();
    motor_group.add(&motor_box);

    // ── 内容页 ───────────────────────────────────────────────────────────
    let heading = gtk::Label::new(Some("手柄状态"));
    heading.add_css_class("title-3");
    heading.set_halign(gtk::Align::Start);
    heading.set_margin_top(18);
    heading.set_margin_bottom(6);
    heading.set_margin_start(24);

    let page = adw::PreferencesPage::new();
    page.add(&device_group);
    page.add(&input_group);
    page.add(&gauge_group);
    page.add(&motor_group);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&heading);
    content.append(&page);

    // ── 空态 ─────────────────────────────────────────────────────────────
    let empty = adw::StatusPage::builder()
        .icon_name("input-gamepad-symbolic")
        .title("未连接手柄")
        .description("连接手柄后，这里会显示实时输入、键程当量与振动测试。")
        .build();

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);
    stack.add_named(&empty, Some(EMPTY_PAGE));
    stack.add_named(&content, Some(CONTENT_PAGE));
    stack.set_visible_child_name(EMPTY_PAGE);

    // D4：tick 驱动重绘。两个必须满足的条件，否则 GTK 帧时钟会以 60fps 空转
    //（实测主线程 40%+）：① 无新帧就 Break 摘掉 tick；② 只有内容态才 queue_draw
    // —— 空态时 DrawingArea 不可见，排队的 draw 永远不会执行，dirty 会卡死。
    let start_tick: Rc<dyn Fn()> = {
        let area = input_area.clone();
        let state = state.clone();
        let stack = stack.clone();
        Rc::new(move || {
            start_viz_tick(&area, &state, &stack);
            // 关键：只挂 tick 不够 —— GTK 的帧时钟不会因为"有 tick 回调"而启动，
            // 没有排队的绘制就永远不会进帧，画面会停在上一帧（P3 实测）。
            area.queue_draw();
        })
    };
    start_tick();

    GamepadPage {
        root: stack.clone(),
        stack,
        device_row,
        info_row,
        gauge_box,
        motor_box,
        motor_reason,
        state,
        gauge_rows: RefCell::new(Vec::new()),
        gauge_sig: RefCell::new(String::new()),
        start_tick,
    }
}

/// 占位灰字。
fn placeholder(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_halign(gtk::Align::Start);
    label.set_margin_top(4);
    label.set_margin_bottom(4);
    label
}

// ─── 键程当量行（P4-1）─────────────────────────────────────────────────────

/// 刻度条配色（浅色主题上的中性轨道 + 主题蓝填充）。
const GAUGE_TRACK: (f64, f64, f64) = (0.65, 0.65, 0.68);
const GAUGE_TICK: (f64, f64, f64) = (0.40, 0.40, 0.44);

/// 一行键程当量的可变值（绘制闭包与刷新逻辑共享）。
#[derive(Clone, Copy)]
struct GaugeVal {
    /// 显示用百分比（摇杆落在平坦区时已吸附到中点）
    pct: f64,
    /// 中点在条上的百分比位置（摇杆居中标线）
    mid_pct: f64,
    /// 平坦区（mid ± flat/fuzz）在条上的百分比范围（淡色带）
    flat_lo: f64,
    flat_hi: f64,
    /// 当前是否落在平坦区（读数已吸附）
    centered: bool,
}

/// 一行键程当量的控件句柄。
struct GaugeRowUi {
    code: u16,
    stick: bool,
    bar: gtk::DrawingArea,
    val: Rc<RefCell<GaugeVal>>,
    pct_label: gtk::Label,
    raw_label: gtk::Label,
}

/// 组装一行：名称 | 刻度条 | 百分比 | `raw / min..max`（P4-1）。
fn build_gauge_row(item: &GaugeItem) -> (gtk::Box, GaugeRowUi) {
    let name_label = gtk::Label::new(Some(item.name));
    name_label.set_xalign(0.0);
    name_label.set_width_chars(10);

    let val = Rc::new(RefCell::new(GaugeVal {
        pct: 50.0,
        mid_pct: 50.0,
        flat_lo: 0.0,
        flat_hi: 0.0,
        centered: false,
    }));
    let bar = gtk::DrawingArea::new();
    bar.set_content_height(12);
    bar.set_hexpand(true);
    {
        let val = val.clone();
        let stick = item.stick;
        bar.set_draw_func(move |_, cr, w, h| {
            draw_gauge(cr, &val.borrow(), stick, w as f64, h as f64);
        });
    }

    let pct_label = gtk::Label::new(Some("—"));
    pct_label.set_xalign(1.0);
    pct_label.set_width_chars(5);
    pct_label.add_css_class("monospace");

    let raw_label = gtk::Label::new(Some("—"));
    raw_label.set_xalign(1.0);
    raw_label.set_width_chars(26);
    raw_label.add_css_class("dim-label");

    let host = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    host.append(&name_label);
    host.append(&bar);
    host.append(&pct_label);
    host.append(&raw_label);

    (
        host,
        GaugeRowUi {
            code: item.code,
            stick: item.stick,
            bar,
            val,
            pct_label,
            raw_label,
        },
    )
}

/// 画一条键程刻度条（P4-1/P4-2）：轨道 + 平坦区淡带 + 填充 + 摇杆中点标线。
///
/// 摇杆从中点向当前值填充（可左右越过中点），扳机从 0 起单向填充。
fn draw_gauge(cr: &gtk::cairo::Context, v: &GaugeVal, stick: bool, w: f64, h: f64) {
    cr.set_source_rgb(GAUGE_TRACK.0, GAUGE_TRACK.1, GAUGE_TRACK.2);
    cr.rectangle(0.0, 0.0, w, h);
    cr.fill().ok();

    if v.flat_hi > v.flat_lo {
        cr.set_source_rgba(CELL_ON.0, CELL_ON.1, CELL_ON.2, 0.18);
        cr.rectangle(
            v.flat_lo / 100.0 * w,
            0.0,
            (v.flat_hi - v.flat_lo) / 100.0 * w,
            h,
        );
        cr.fill().ok();
    }

    let x1 = (v.pct / 100.0 * w).clamp(0.0, w);
    let x0 = if stick {
        (v.mid_pct / 100.0 * w).clamp(0.0, w)
    } else {
        0.0
    };
    let (a, b) = if x1 >= x0 { (x0, x1) } else { (x1, x0) };
    if b - a > 0.5 {
        cr.set_source_rgb(CELL_ON.0, CELL_ON.1, CELL_ON.2);
        cr.rectangle(a, 0.0, b - a, h);
        cr.fill().ok();
    }

    if stick {
        let mx = (v.mid_pct / 100.0 * w).clamp(0.0, w);
        cr.set_line_width(2.0);
        cr.set_source_rgb(GAUGE_TICK.0, GAUGE_TICK.1, GAUGE_TICK.2);
        cr.move_to(mx, 0.0);
        cr.line_to(mx, h);
        cr.stroke().ok();
    }
}

impl GamepadPage {
    /// P5：按设备能力开关振动控件，并显示置灰原因（`enabled=false` 时必填）。
    ///
    /// 无 `FF_RUMBLE` 能力（A15「无能力置灰」）与未连接都走这里。
    pub fn set_ff(&self, enabled: bool, reason: &str) {
        self.motor_box.set_sensitive(enabled);
        self.motor_reason.set_text(reason);
    }

    /// 切换空态 / 内容态。
    pub fn set_connected(&self, connected: bool) {
        self.stack
            .set_visible_child_name(if connected { CONTENT_PAGE } else { EMPTY_PAGE });
    }

    /// 用设备名列表刷新下拉，`selected` 是要选中的下标（None 表示不变）。
    ///
    /// 与当前模型完全一致时直接返回：本方法会被热插拔轮询反复调用，
    /// 每次重建 `StringList` 会白白触发重排（P3-4 空闲 CPU）。
    pub fn set_devices(&self, names: &[String], selected: Option<u32>) {
        let current: Vec<String> = (0..self.device_row.model().map(|m| m.n_items()).unwrap_or(0))
            .filter_map(|i| {
                self.device_row
                    .model()
                    .and_then(|m| m.item(i))
                    .and_then(|o| o.downcast::<gtk::StringObject>().ok())
                    .map(|s| s.string().to_string())
            })
            .collect();
        let wanted: Vec<String> = if names.is_empty() {
            vec!["（无可用设备）".to_string()]
        } else {
            names.to_vec()
        };
        if current == wanted && selected.is_none_or(|i| self.device_row.selected() == i) {
            return;
        }
        let items: Vec<&str> = wanted.iter().map(String::as_str).collect();
        let model = if items.is_empty() {
            gtk::StringList::new(&["（无可用设备）"])
        } else {
            gtk::StringList::new(&items)
        };
        self.device_row.set_model(Some(&model));
        if let Some(index) = selected
            && (names.is_empty() || index < names.len() as u32)
        {
            self.device_row.set_selected(index);
        }
    }

    /// 设置连接信息副标题。
    pub fn set_info(&self, text: &str) {
        self.info_row.set_subtitle(text);
    }

    /// P3：喂入一帧输入。`caps` 为 None 时沿用上次的能力。
    ///
    /// 只写共享状态并打脏标记，真正重绘由 tick 回调触发（见 `build`）。
    pub fn update(&self, caps: Option<&DeviceCaps>, snapshot: Option<&InputSnapshot>) {
        let mut state = self.state.borrow_mut();
        if let Some(caps) = caps {
            state.caps = Some(caps.clone());
        }
        if let Some(snapshot) = snapshot {
            state.pressed = snapshot.pressed.clone();
            state.axes = snapshot.axes.clone();
        }
        state.dirty = true;
        // P4：同帧顺手把「键程当量」刷到最新（签相同则只改数值/文字）
        if let Some(caps) = state.caps.as_ref() {
            self.refresh_gauges(caps, &state.axes);
        }
        drop(state);
        (self.start_tick)();
    }

    /// 断开 / 切走设备时清空画面（回到「等待手柄输入…」占位）。
    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();
        state.caps = None;
        state.pressed = BTreeSet::new();
        state.axes = BTreeMap::new();
        state.dirty = true;
        drop(state);
        self.reset_gauges();
        self.set_ff(false, "未连接手柄，振动测试不可用");
        (self.start_tick)();
    }

    // ── 键程当量（P4）───────────────────────────────────────────────────

    /// 把当前轴值刷到「键程当量」各行（P4-1）。
    ///
    /// 行集合签名（节点 + 轴码）没变就只更新数值与文字；变了才重建控件。
    /// 文字与刻度条只在值真变化时才写/重绘，避免 16ms 一帧的无谓排版
    ///（P3-4 的空闲 CPU 教训）。
    fn refresh_gauges(&self, caps: &DeviceCaps, axes: &BTreeMap<u16, i32>) {
        let items = gamepad_viz::gauge_rows(caps);
        let sig = format!(
            "{}|{:?}",
            caps.node,
            items.iter().map(|i| (i.code, i.stick)).collect::<Vec<_>>()
        );
        let need_rebuild = self.gauge_sig.borrow().as_str() != sig;
        if need_rebuild {
            *self.gauge_sig.borrow_mut() = sig;
            self.rebuild_gauges(&items);
        }

        let mut rows = self.gauge_rows.borrow_mut();
        for row in rows.iter_mut() {
            let Some(info) = caps.axis(row.code) else {
                continue;
            };
            // 尚未收到过该轴的事件：摇杆取中点（松手），扳机取最小值（未扣）
            let fallback = if row.stick { info.midpoint() } else { info.min };
            let raw = axes.get(&row.code).copied().unwrap_or(fallback);
            let (pct, centered) = gamepad_viz::gauge_display(info, raw, row.stick);
            let mid_pct = info.percent(info.midpoint());
            let tol = info.flat.max(info.fuzz);
            let flat_lo = info.percent(info.midpoint() - tol);
            let flat_hi = info.percent(info.midpoint() + tol);

            let mut v = row.val.borrow_mut();
            let moved = (v.pct - pct).abs() > 0.01
                || v.centered != centered
                || (v.mid_pct - mid_pct).abs() > 0.01
                || (v.flat_lo - flat_lo).abs() > 0.01
                || (v.flat_hi - flat_hi).abs() > 0.01;
            if moved {
                *v = GaugeVal {
                    pct,
                    mid_pct,
                    flat_lo,
                    flat_hi,
                    centered,
                };
            }
            drop(v);

            let pct_text = format!("{pct:.0}%");
            if row.pct_label.text() != pct_text {
                row.pct_label.set_text(&pct_text);
            }
            let mut raw_text = format!("{raw} / {}..{}", info.min, info.max);
            if centered {
                raw_text.push_str(" · 居中");
            }
            if row.raw_label.text() != raw_text {
                row.raw_label.set_text(&raw_text);
            }
            if moved {
                row.bar.queue_draw();
            }
        }
    }

    /// 重建「键程当量」行（设备/能力变化时调用）；一个轴都没有就回占位文案。
    fn rebuild_gauges(&self, items: &[GaugeItem]) {
        while let Some(child) = self.gauge_box.first_child() {
            self.gauge_box.remove(&child);
        }
        let mut rows = self.gauge_rows.borrow_mut();
        rows.clear();
        if items.is_empty() {
            self.gauge_box
                .append(&placeholder("该设备没有可用的摇杆 / 扳机轴…"));
            return;
        }
        for item in items {
            let (host, ui) = build_gauge_row(item);
            self.gauge_box.append(&host);
            rows.push(ui);
        }
    }

    /// 断开 / 切走设备：去掉所有行，回到占位文案。
    fn reset_gauges(&self) {
        self.gauge_sig.borrow_mut().clear();
        while let Some(child) = self.gauge_box.first_child() {
            self.gauge_box.remove(&child);
        }
        self.gauge_rows.borrow_mut().clear();
        self.gauge_box.append(&placeholder("等待手柄输入后显示…"));
    }
}

thread_local! {
    /// tick 是否已挂（GTK 单线程、本页唯一，用线程局部即可）。
    static TICKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 挂一帧 tick：有新帧就 `queue_draw`，画完（`dirty` 被 draw 回调清掉）后
/// 返回 `Break` 自摘除，帧时钟随之停机；下次 [`GamepadPage::update`] 再挂。
///
/// `ticking` 标记避免重复注册（`add_tick_callback` 不是幂等的）。
fn start_viz_tick(area: &gtk::DrawingArea, state: &Rc<RefCell<VizState>>, stack: &gtk::Stack) {
    if TICKING.with(|slot| {
        let already = slot.get();
        slot.set(true);
        already
    }) {
        return;
    }
    let area_for_cb = area.clone();
    let state = state.clone();
    let stack = stack.clone();
    area.add_tick_callback(move |_, _| {
        let dirty = state.borrow().dirty;
        let on_content = stack
            .visible_child_name()
            .is_some_and(|n| n == CONTENT_PAGE);
        if dirty && on_content {
            area_for_cb.queue_draw();
            adw::glib::ControlFlow::Continue
        } else {
            // 无新帧，或画布不在内容态（隐藏时 draw 不会执行、dirty 会卡死）：
            // 摘掉自己让帧时钟停机；dirty 保留，下次 update/clear 会重新挂上。
            TICKING.with(|slot| slot.set(false));
            adw::glib::ControlFlow::Break
        }
    });
}

// ─── 绘制（P3-1 按键 / P3-2 摇杆 / P3-3 扳机）─────────────────────────────

/// 画布配色（深色底 + 主题蓝高亮）。
const BG: (f64, f64, f64) = (0.13, 0.13, 0.15);
const CELL_OFF: (f64, f64, f64) = (0.27, 0.27, 0.30);
const CELL_ON: (f64, f64, f64) = (0.24, 0.47, 0.93);
const DIM: (f64, f64, f64) = (0.55, 0.55, 0.58);
const DIAL: (f64, f64, f64) = (0.42, 0.42, 0.46);
const TRACK: (f64, f64, f64) = (0.20, 0.20, 0.23);

/// 主绘制：按钮格 + 两支摇杆 + LT/RT 柱。
fn draw_viz(cr: &gtk::cairo::Context, state: &VizState, w: f64, h: f64) {
    cr.set_source_rgb(BG.0, BG.1, BG.2);
    cr.rectangle(0.0, 0.0, w, h);
    cr.fill().ok();

    let Some(caps) = state.caps.as_ref() else {
        cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
        cr.set_font_size(13.0);
        cr.move_to(12.0, h / 2.0);
        cr.show_text("等待手柄输入…").ok();
        return;
    };

    // ── 按键区（左侧约 42%） ─────────────────────────────────────────────
    let pad = 8.0;
    let grid_w = (w * 0.42 - pad).max(40.0);
    let grid_h = (h - pad * 2.0).max(40.0);
    let buttons = gamepad_viz::present_buttons(caps, &state.pressed);
    let cells = gamepad_viz::button_grid(buttons.len(), gamepad_viz::BUTTON_COLS, grid_w, grid_h);
    cr.set_font_size(10.0);
    for ((_, label, down), cell) in buttons.iter().zip(cells.iter()) {
        let color = if *down { CELL_ON } else { CELL_OFF };
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.rectangle(cell.x + pad, cell.y + pad, cell.w - 4.0, cell.h - 4.0);
        cr.fill().ok();
        let (cx, cy) = cell.center();
        let label_w = label.chars().count() as f64 * 6.0;
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.move_to(cx + pad - label_w / 2.0, cy + pad + 3.5);
        cr.show_text(label).ok();
    }

    // ── 摇杆（中段两支：圆盘 + 中心十字 + 死区圈 + 圆点） ────────────────
    let dial_r = (h * 0.30).min(52.0);
    let centers = [
        (w * 0.55, h * 0.50, gamepad_viz::STICK_LEFT, "左摇杆"),
        (w * 0.72, h * 0.50, gamepad_viz::STICK_RIGHT, "右摇杆"),
    ];
    for (cx, cy, axes, name) in centers {
        // 关键：标签的 `move_to` 会留下一个悬空当前点（`show_text` 不动路径），
        // 而 cairo 的 `arc` 会从当前点连一条直线到弧的起点 —— 那就是摇杆上的
        // 「飞线」。先清干净路径再画圆盘。
        cr.new_path();
        cr.set_line_width(2.0);
        cr.set_source_rgb(DIAL.0, DIAL.1, DIAL.2);
        cr.arc(cx, cy, dial_r, 0.0, std::f64::consts::TAU);
        cr.stroke().ok();

        if let Some(info) = caps.axis(axes.0) {
            let dz = gamepad_viz::deadzone_radius(info.flat, info.min, info.max, dial_r);
            if dz > 0.5 {
                cr.set_line_width(1.0);
                cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
                cr.arc(cx, cy, dz, 0.0, std::f64::consts::TAU);
                cr.stroke().ok();
            }
        }

        cr.set_line_width(1.0);
        cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
        cr.move_to(cx - 6.0, cy);
        cr.line_to(cx + 6.0, cy);
        cr.move_to(cx, cy - 6.0);
        cr.line_to(cx, cy + 6.0);
        cr.stroke().ok();

        if gamepad_viz::stick_available(caps, axes) {
            let px = percent_of(caps, &state.axes, axes.0);
            let py = percent_of(caps, &state.axes, axes.1);
            // `percent_of` 返回 0–100，`stick_dot` 吃 0–1（否则 clamp 全顶边）
            let (dx, dy) = gamepad_viz::stick_dot(px / 100.0, py / 100.0, cx, cy, dial_r - 4.0);
            cr.set_source_rgb(CELL_ON.0, CELL_ON.1, CELL_ON.2);
            cr.arc(dx, dy, 8.0, 0.0, std::f64::consts::TAU);
            cr.fill().ok();
        }

        cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
        cr.set_font_size(11.0);
        let label_w = name.chars().count() as f64 * 7.0;
        cr.move_to(cx - label_w / 2.0, cy + dial_r + 14.0);
        cr.show_text(name).ok();
    }

    // ── 扳机柱（右侧两根：LT / RT） ──────────────────────────────────────
    let bar_w = (w * 0.05).clamp(18.0, 34.0);
    let bar_top = 26.0;
    let bar_h = (h - bar_top - 34.0).max(30.0);
    let bars = [
        (w * 0.87, gamepad_viz::TRIGGERS.0, "LT"),
        (w * 0.87 + bar_w + 14.0, gamepad_viz::TRIGGERS.1, "RT"),
    ];
    for (x, code, name) in bars {
        cr.set_source_rgb(TRACK.0, TRACK.1, TRACK.2);
        cr.rectangle(x, bar_top, bar_w, bar_h);
        cr.fill().ok();
        if let Some(info) = caps.axis(code) {
            // 扳机没收到过事件时按「未扣」= min 显示（与键程当量行一致）
            let raw = state.axes.get(&code).copied().unwrap_or(info.min);
            let pct = info.percent(raw);
            let fill = gamepad_viz::trigger_height(pct, bar_h);
            cr.set_source_rgb(CELL_ON.0, CELL_ON.1, CELL_ON.2);
            cr.rectangle(x, bar_top + bar_h - fill, bar_w, fill);
            cr.fill().ok();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_font_size(10.0);
            let text = format!("{pct:.0}%");
            cr.move_to(x, bar_top - 8.0);
            cr.show_text(&text).ok();
        }
        cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
        cr.set_font_size(11.0);
        cr.move_to(x, bar_top + bar_h + 16.0);
        cr.show_text(name).ok();
    }
}

/// 取某轴当前百分比；轴没值时用中点（松手状态）。
fn percent_of(caps: &DeviceCaps, axes: &BTreeMap<u16, i32>, code: u16) -> f64 {
    match caps.axis(code) {
        Some(info) => {
            let raw = axes.get(&code).copied().unwrap_or_else(|| info.midpoint());
            info.percent(raw)
        }
        None => 50.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::gamepad_input::AxisInfo;

    /// 离屏画布尺寸（与页面 `content_height(200)` 一致，宽取够用即可）。
    const W: i32 = 939;
    const H: i32 = 200;

    /// 造一个「标准 Xbox 布局 + 六轴」的设备能力（复刻出问题的那张截图的设备）。
    fn test_caps() -> DeviceCaps {
        let stick = |code: u16| AxisInfo {
            code,
            name: format!("ABS_{code}"),
            min: -32768,
            max: 32767,
            fuzz: 0,
            flat: 16,
            resolution: 0,
        };
        let trigger = |code: u16| AxisInfo {
            code,
            name: format!("ABS_{code}"),
            min: 0,
            max: 1023,
            fuzz: 0,
            flat: 0,
            resolution: 0,
        };
        DeviceCaps {
            node: "/dev/input/event0".into(),
            name: "测试手柄".into(),
            axes: [0u16, 1, 3, 4]
                .into_iter()
                .map(|c| (c, stick(c)))
                .chain([(0x02u16, trigger(0x02)), (0x05, trigger(0x05))])
                .collect(),
            buttons: [
                0x130, 0x131, 0x134, 0x133, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e,
            ]
            .to_vec(),
            ff_rumble: false,
        }
    }

    /// 把 `draw_viz` 画到离屏 ARGB32 位图上，返回像素（含行距 stride）。
    /// 纯 cairo，不需要 `gtk::init` / 显示连接。
    fn render(state: &VizState) -> (Vec<u8>, usize) {
        let mut surface =
            gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, W, H).expect("离屏位图");
        {
            let cr = gtk::cairo::Context::new(&surface).expect("cairo 上下文");
            draw_viz(&cr, state, W as f64, H as f64);
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("读回像素").to_vec();
        (data, stride)
    }

    /// 回归测试：输入状态画布上，摇杆表盘之外不该出现任何表盘配色的杂散像素
    /// （用户看到的「两条飞线」：最后一个按键标签的 `move_to` 悬空点 → 左表盘弧起点、
    /// 左摇杆标签的 `move_to` 悬空点 → 右表盘弧起点）。
    #[test]
    fn 输入状态_摇杆表盘外不出现飞线() {
        let state = VizState {
            caps: Some(test_caps()),
            // 左摇杆压到 81%（复刻截图），右摇杆居中，扳机各按一点
            axes: [
                (0x00, 19_000),
                (0x01, 0),
                (0x03, 0),
                (0x04, 0),
                (0x02, 64),
                (0x05, 191),
            ]
            .into_iter()
            .collect(),
            ..VizState::default()
        };

        let (data, stride) = render(&state);

        let dial_r = (H as f64 * 0.30).min(52.0);
        let centers = [W as f64 * 0.55, W as f64 * 0.72];
        let cy = H as f64 * 0.50;
        // 表盘描边配色 `DIAL` = (0.42, 0.42, 0.46)，取「线芯满色」±2：
        // 飞线是 2px 描边，中间一列必定是满色；而文字抗锯齿的混色
        //（`DIM` 或白色 × 背景）r/g 落不进 107±2，不会误报。
        let is_dial = |r: i32, g: i32, b: i32| {
            (r - 107).abs() <= 2 && (g - 107).abs() <= 2 && (b - 117).abs() <= 2
        };

        let (mut on_dial, mut stray) = (0usize, 0usize);
        let mut stray_px = Vec::new();
        for y in 0..H as usize {
            for x in 0..W as usize {
                let i = y * stride + x * 4;
                let (b, g, r) = (data[i] as i32, data[i + 1] as i32, data[i + 2] as i32);
                if !is_dial(r, g, b) {
                    continue;
                }
                let d = centers
                    .iter()
                    .map(|cx| ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt())
                    .filter(|d| (d - dial_r).abs() <= 4.0)
                    .count();
                if d > 0 {
                    on_dial += 1;
                } else {
                    stray += 1;
                    stray_px.push((x, y, r, g, b));
                }
            }
        }
        // 先自证「确实画出了表盘」，否则 stray=0 可能只是什么都没画
        assert!(on_dial > 100, "表盘描边没画出来？on_dial={on_dial}");
        assert_eq!(
            stray, 0,
            "表盘外出现 {stray} 个表盘配色像素（就是那两条飞线）: {stray_px:?}"
        );
    }
}
