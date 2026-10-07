//! 输入可视化布局（GOAL.md P3）：**纯计算**，不依赖 gtk / cairo / adw。
//!
//! 绘制本身在 [`crate::page::gamepad`] 的 `set_draw_func` 里完成；这里只提供
//! 「按钮格子在哪、摇杆圆点画哪、扳机柱多高」这类可单测的几何与语义映射。

use std::collections::{BTreeMap, BTreeSet};

use crate::utils::gamepad_input::{AxisInfo, DeviceCaps};

/// 绘制用的输入状态（P3）：由 `app.rs` 喂、由页面的 draw 回调消费。
///
/// `dirty` 表示有新帧待重绘，页面 tick 见脏才 `queue_draw`（P3-4 控空闲 CPU）。
#[derive(Debug, Clone, Default)]
pub struct VizState {
    /// 设备能力（决定画哪些按钮/摇杆/扳机，缺轴优雅降级）
    pub caps: Option<DeviceCaps>,
    /// 当前按下的按键 code
    pub pressed: BTreeSet<u16>,
    /// 轴 code → 原始值
    pub axes: BTreeMap<u16, i32>,
    /// 有新数据待重绘
    pub dirty: bool,
}

/// 画布上要展示的标准手柄按键（Linux `input-event-codes.h` 语义）。
///
/// Xbox 布局对应：`BTN_SOUTH=A`、`BTN_EAST=B`、`BTN_WEST=X`、`BTN_NORTH=Y`。
pub const BUTTONS: &[(u16, &str)] = &[
    (0x130, "A"),
    (0x131, "B"),
    (0x134, "X"),
    (0x133, "Y"),
    (0x136, "LB"),
    (0x137, "RB"),
    (0x13a, "Back"),
    (0x13b, "Start"),
    (0x13c, "Guide"),
    (0x13d, "LS"),
    (0x13e, "RS"),
    (0x220, "上"),
    (0x221, "下"),
    (0x222, "左"),
    (0x223, "右"),
];

/// HID 索引式手柄的展示表（见 [`Convention::HidIndex`]）。
///
/// 蓝牙 Xbox（`hid-microsoft`）/ `hid-generic` 走内核 `hid-input.c` 的默认映射：
/// **键码 = `0x130 + (HID usage − 1)`**，而 Xbox 描述符的 usage 顺序是
/// A、B、X、Y、LB、RB、Back、Start、LS、RS，于是 `0x132`（`BTN_C`）其实是 **X**、
/// `0x134`（`BTN_WEST`）其实是 **LB** —— 按语义表显示就会「缺键 + 亮错格」。
///
/// 顺序按标准 Xbox 网格排：`A B X Y LB / RB Back Start LS RS / Guide 上 下 左 右`。
pub const HID_INDEX_BUTTONS: &[(u16, &str)] = &[
    (0x130, "A"),
    (0x131, "B"),
    (0x132, "X"),
    (0x133, "Y"),
    (0x134, "LB"),
    (0x135, "RB"),
    (0x136, "Back"),
    (0x137, "Start"),
    (0x138, "LS"),
    (0x139, "RS"),
    // usage 11（11 键以上的 HID 手柄）
    (0x13a, "Guide"),
    // `KEY_MENU`：Xbox 蓝牙导引键走 System「Sys Main Menu」→ 内核映射为 `KEY_MENU`
    // （SDL3 内置映射表对本机 `050000005e040000e002000003090000` 就是 `guide:b10`，
    //  b10 = 唯一一个小于 `BTN_JOYSTICK` 的键，即 `0x8b`）
    (0x08b, "Guide"),
    (0x220, "上"),
    (0x221, "下"),
    (0x222, "左"),
    (0x223, "右"),
];

/// 按键码约定：决定 code → 标签用哪张表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    /// 语义式：`xpad`（USB Xbox）/ uinput 虚拟手柄按内核
    /// `Documentation/input/gamepad.rst` 的**物理位置**命名
    /// （`BTN_SOUTH=A`、`BTN_EAST=B`、`BTN_WEST=X`、`BTN_NORTH=Y`、
    /// `BTN_TL=LB`、`BTN_SELECT=Back`…），见 [`BUTTONS`]。
    Semantic,
    /// HID 索引式：`hid-generic` / `hid-microsoft`（蓝牙 Xbox）按 usage 序号给键码，
    /// 见 [`HID_INDEX_BUTTONS`]。
    HidIndex,
}

/// 判定设备用哪种按键码约定。
///
/// 判据是能力集合本身：
/// - `BTN_C(0x132)` / `BTN_Z(0x135)` 只会出现在「usage 序号 = 键码」的设备上 ——
///   语义式的现代手柄（`xpad` 的 Xbox360/One）从不声明它们；
/// - 语义式的「完整集」（`BTN_SELECT + BTN_THUMBL + BTN_THUMBR`）同时出现时判语义式：
///   这覆盖注册了整个 `BTN` 段的 uinput 虚拟手柄与 `xpad` 的原始 Xbox 分支。
///
/// 局限：既带 `BTN_C` 又带完整语义集的**真·15 键 HID 手柄**（usage 1..15）会判成语义式，
/// 与修复前行为一致；靠总线/驱动名才能再细分，暂不引入。
pub fn convention(caps: &DeviceCaps) -> Convention {
    let index_like = caps.has_button(0x132) || caps.has_button(0x135);
    let semantic_core = caps.has_button(0x13a) && caps.has_button(0x13d) && caps.has_button(0x13e);
    if index_like && !semantic_core {
        Convention::HidIndex
    } else {
        Convention::Semantic
    }
}

/// 按键区默认列数（P3-1：5 列 ×3 行）。
pub const BUTTON_COLS: usize = 5;

/// 左摇杆轴码：X / Y。
pub const STICK_LEFT: (u16, u16) = (0x00, 0x01);
/// 右摇杆轴码：RX / RY。
pub const STICK_RIGHT: (u16, u16) = (0x03, 0x04);
/// LT / RT 轴码（Xbox 协议下即 `ABS_Z` / `ABS_RZ`）。
pub const TRIGGERS: (u16, u16) = (0x02, 0x05);

/// 一个按钮格子的矩形（左上角 + 宽高），单位是画布像素。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Cell {
    /// 中心点（画文字用）。
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// 按钮网格布局：`n` 个按钮按 `cols` 列在 `width × height` 区域里均匀排布。
///
/// 返回值顺序与传入数量一致；区域过小也不会重叠（格子按比例收缩）。
pub fn button_grid(n: usize, cols: usize, width: f64, height: f64) -> Vec<Cell> {
    if n == 0 {
        return Vec::new();
    }
    let cols = cols.max(1);
    let rows = n.div_ceil(cols);
    let cw = width / cols as f64;
    let ch = height / rows as f64;
    (0..n)
        .map(|i| {
            let (col, row) = (i % cols, i / cols);
            Cell {
                x: col as f64 * cw,
                y: row as f64 * ch,
                w: cw,
                h: ch,
            }
        })
        .collect()
}

/// 摇杆圆点位置：把两个归一化到 `0..=1` 的轴值映射进半径 `r` 的圆。
///
/// `(0.5, 0.5)`（摇杆居中）落在圆心 `(cx, cy)`；越界值会被夹回圆内。
pub fn stick_dot(x_pct: f64, y_pct: f64, cx: f64, cy: f64, r: f64) -> (f64, f64) {
    // 归一化到 -1..1
    let nx = x_pct.clamp(0.0, 1.0) * 2.0 - 1.0;
    let ny = y_pct.clamp(0.0, 1.0) * 2.0 - 1.0;
    // 对角方向夹到单位圆内，避免圆点跑出表盘
    let len = (nx * nx + ny * ny).sqrt();
    let (nx, ny) = if len > 1.0 {
        (nx / len, ny / len)
    } else {
        (nx, ny)
    };
    (cx + nx * r, cy + ny * r)
}

/// 摇杆死区半径（像素）：由 `flat`（原始码死区）换算到表盘半径。
///
/// `flat` 为0或轴无范围时返回0。
pub fn deadzone_radius(flat: i32, min: i32, max: i32, r: f64) -> f64 {
    let span = max - min;
    if span <= 0 || flat <= 0 {
        return 0.0;
    }
    (flat as f64 / span as f64 * r).clamp(0.0, r)
}

/// 扳机柱填充高度：百分比 → 像素（0..=height，含夹取）。
pub fn trigger_height(pct: f64, height: f64) -> f64 {
    pct.clamp(0.0, 100.0) / 100.0 * height
}

/// 设备没暴露的轴/按键要优雅降级（P4-3 / P3-1）：算出**实际存在**的展示项。
///
/// 标签按设备的按键码约定（[`convention`]）选表：语义式用 [`BUTTONS`]、
/// HID 索引式用 [`HID_INDEX_BUTTONS`]（否则蓝牙 Xbox 会缺键、亮错格）。
///
/// - 十字键：设备声明了 `BTN_DPAD_*` 就按声明显示；只有 `ABS_HAT0X/Y` 时按轴值
///   合成（`-1` = 上/左、`+1` = 下/右，内核 hat 约定），中立位也占格（暗格）。
/// - 展示表里没有、但**正被按下**的按键按
///   [`key_name`](crate::utils::gamepad_input::key_name) 回退补位
///   （`BTN_XXX` / `KEY_0x??`），保证「所有输入」都不漏。
pub fn present_buttons(
    caps: &DeviceCaps,
    pressed: &BTreeSet<u16>,
    axes: &BTreeMap<u16, i32>,
) -> Vec<(u16, String, bool)> {
    let table = match convention(caps) {
        Convention::Semantic => BUTTONS,
        Convention::HidIndex => HID_INDEX_BUTTONS,
    };

    // 十字键可用性：有 BTN_DPAD_* 用声明的；有 HAT 轴则合成
    let dpad_declared = [0x220u16, 0x221, 0x222, 0x223]
        .iter()
        .any(|c| caps.has_button(*c));
    let hat = caps.has_axis(0x10) || caps.has_axis(0x11);
    let dpad_shown = dpad_declared || hat;

    let mut down = pressed.clone();
    if !dpad_declared && hat {
        if let Some(x) = axes.get(&0x10) {
            if *x < 0 {
                down.insert(0x222); // 左
            } else if *x > 0 {
                down.insert(0x223); // 右
            }
        }
        if let Some(y) = axes.get(&0x11) {
            if *y < 0 {
                down.insert(0x220); // 上
            } else if *y > 0 {
                down.insert(0x221); // 下
            }
        }
    }

    let mut out: Vec<(u16, String, bool)> = Vec::new();
    let mut seen = BTreeSet::new();
    for &(code, label) in table {
        let is_dpad = (0x220..=0x223).contains(&code);
        if !(caps.has_button(code) || down.contains(&code) || (is_dpad && dpad_shown)) {
            continue;
        }
        out.push((code, (*label).to_string(), down.contains(&code)));
        seen.insert(code);
    }
    for code in pressed {
        if seen.insert(*code) {
            out.push((*code, crate::utils::gamepad_input::key_name(*code), true));
        }
    }
    out
}

/// 摇杆可用性：两个轴都在才画圆盘；否则整块降级为「轴缺失」。
pub fn stick_available(caps: &DeviceCaps, (x, y): (u16, u16)) -> bool {
    caps.has_axis(x) && caps.has_axis(y)
}

// ── 键程当量（P4）─────────────────────────────────────────────────────────

/// 「键程当量」一行（P4-1）：一行对应一个轴。
#[derive(Debug, Clone, PartialEq)]
pub struct GaugeItem {
    /// 轴码（`ABS_*`）
    pub code: u16,
    /// 显示名
    pub name: &'static str,
    /// `true` = 摇杆语义（中点居中 + 平坦区吸附）；`false` = 扳机语义（从 0 起算）
    pub stick: bool,
}

/// 按设备能力生成「键程当量」行（P4-3）：缺轴就少一行，一个轴都没有返回空表
/// （页面据此回占位文案，不报错）。
///
/// 顺序固定：左摇杆 X/Y → 右摇杆 X/Y → LT → RT（缺的直接跳过）。
pub fn gauge_rows(caps: &DeviceCaps) -> Vec<GaugeItem> {
    let pairs = [
        (STICK_LEFT, ["左摇杆 X", "左摇杆 Y"]),
        (STICK_RIGHT, ["右摇杆 X", "右摇杆 Y"]),
    ];
    let mut items = Vec::new();
    for ((x, y), names) in pairs {
        for (code, name) in [(x, names[0]), (y, names[1])] {
            if caps.axis(code).is_some() {
                items.push(GaugeItem {
                    code,
                    name,
                    stick: true,
                });
            }
        }
    }
    for (code, name) in [(TRIGGERS.0, "LT"), (TRIGGERS.1, "RT")] {
        if caps.axis(code).is_some() {
            items.push(GaugeItem {
                code,
                name,
                stick: false,
            });
        }
    }
    items
}

/// 显示用键程当量（P4-2）：摇杆原始值落在平坦区（`flat`/`fuzz` 取大者）内时
/// 吸附到中点百分比并返回 `true`，让 50% 附近的读数不再抖动；扳机没有居中
/// 语义，不做吸附。
pub fn gauge_display(info: &AxisInfo, raw: i32, stick: bool) -> (f64, bool) {
    if stick && info.in_deadband(raw) {
        (info.percent(info.midpoint()), true)
    } else {
        (info.percent(raw), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 按键网格_不越界且互不重叠() {
        let cells = button_grid(BUTTONS.len(), BUTTON_COLS, 400.0, 180.0);
        assert_eq!(cells.len(), BUTTONS.len());
        for c in &cells {
            assert!(c.x >= 0.0 && c.y >= 0.0);
            assert!(c.x + c.w <= 400.0 + 1e-9);
            assert!(c.y + c.h <= 180.0 + 1e-9);
        }
        // 相邻格子要么左右相接要么上下相接，不能相互覆盖
        for (i, a) in cells.iter().enumerate() {
            for b in cells.iter().skip(i + 1) {
                let overlap_x = a.x < b.x + b.w - 1e-6 && b.x < a.x + a.w - 1e-6;
                let overlap_y = a.y < b.y + b.h - 1e-6 && b.y < a.y + a.h - 1e-6;
                assert!(!(overlap_x && overlap_y), "格子重叠: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn 按键网格_数量不足也铺满列宽() {
        let cells = button_grid(1, BUTTON_COLS, 500.0, 90.0);
        assert_eq!(cells.len(), 1);
        assert!((cells[0].w - 100.0).abs() < 1e-9); // 500 / 5
        assert!((cells[0].h - 90.0).abs() < 1e-9); // 只有一行
    }

    #[test]
    fn 摇杆圆点_居中到圆心_极限压到边缘() {
        let (x, y) = stick_dot(0.5, 0.5, 100.0, 80.0, 40.0);
        assert!((x - 100.0).abs() < 1e-9 && (y - 80.0).abs() < 1e-9);

        let (x, y) = stick_dot(1.0, 0.5, 100.0, 80.0, 40.0);
        assert!((x - 140.0).abs() < 1e-9 && (y - 80.0).abs() < 1e-9);

        // 对角越界要夹回圆上
        let (x, y) = stick_dot(1.0, 1.0, 100.0, 80.0, 40.0);
        let d = ((x - 100.0).powi(2) + (y - 80.0).powi(2)).sqrt();
        assert!((d - 40.0).abs() < 1e-6, "对角点应落在圆上，实际 {d}");

        // 超范围值也不炸
        let (x, y) = stick_dot(-3.0, 7.0, 0.0, 0.0, 10.0);
        let d = (x * x + y * y).sqrt();
        assert!(d <= 10.0 + 1e-6);
    }

    #[test]
    fn 死区半径_按比例换算() {
        assert_eq!(deadzone_radius(0, -32768, 32767, 40.0), 0.0);
        assert_eq!(deadzone_radius(100, 1, 1, 40.0), 0.0); // 无跨度
        let r = deadzone_radius(3277, -32768, 32767, 40.0);
        assert!((r - 40.0 * 3277.0 / 65535.0).abs() < 1e-6);
        assert_eq!(deadzone_radius(999_999, -100, 100, 40.0), 40.0); // 夹到整圆
    }

    #[test]
    fn 扳机柱高度_夹取到画布内() {
        assert_eq!(trigger_height(-10.0, 120.0), 0.0);
        assert_eq!(trigger_height(0.0, 120.0), 0.0);
        assert_eq!(trigger_height(50.0, 120.0), 60.0);
        assert_eq!(trigger_height(100.0, 120.0), 120.0);
        assert_eq!(trigger_height(250.0, 120.0), 120.0);
    }

    #[test]
    fn 缺轴设备_优雅降级() {
        let caps = DeviceCaps {
            name: "x".into(),
            node: "/dev/input/event0".into(),
            axes: std::collections::BTreeMap::new(),
            buttons: vec![0x130, 0x131],
            ff_rumble: false,
        };
        assert!(!stick_available(&caps, STICK_LEFT));
        assert!(!stick_available(&caps, STICK_RIGHT));
        // 只画得出手柄声明过的按键
        let present = present_buttons(&caps, &BTreeSet::new(), &BTreeMap::new());
        assert_eq!(
            present.iter().map(|c| c.0).collect::<Vec<_>>(),
            vec![0x130, 0x131]
        );
        // 即便 caps 未声明，正在按下的键也要显示（运行期事件优先）
        let mut pressed = BTreeSet::new();
        pressed.insert(0x13d); // LS
        let present = present_buttons(&caps, &pressed, &BTreeMap::new());
        assert!(present.iter().any(|c| c.0 == 0x13d && c.2));
    }

    /// 造一个「蓝牙 Xbox 真手柄」能力：`scripts/evdev_probe.py /dev/input/event24`
    /// 实测的 10 个 `BTN_*` + `KEY_MENU(0x8b)` + HAT 十字键 + 摇杆/扳机轴。
    fn xbox_bt_caps() -> DeviceCaps {
        let axis_codes = [
            (0x00, -32768, 32767, 4095),
            (0x01, -32768, 32767, 4095),
            (0x02, 0, 1023, 0),
            (0x03, -32768, 32767, 4095),
            (0x04, -32768, 32767, 4095),
            (0x05, 0, 1023, 0),
            (0x10, -1, 1, 0),
            (0x11, -1, 1, 0),
        ];
        DeviceCaps {
            name: "Xbox Wireless Controller".into(),
            node: "/dev/input/event24".into(),
            axes: axis_codes
                .into_iter()
                .map(|(code, min, max, flat)| (code, axis(code, min, max, 0, flat)))
                .collect(),
            buttons: (0x130..=0x139).chain(std::iter::once(0x8b)).collect(),
            ff_rumble: true,
        }
    }

    #[test]
    fn 索引式蓝牙手柄_标签正确且格子齐全() {
        let caps = xbox_bt_caps();
        assert_eq!(convention(&caps), Convention::HidIndex);

        let present = present_buttons(&caps, &BTreeSet::new(), &BTreeMap::new());
        let label = |code: u16| {
            present
                .iter()
                .find(|c| c.0 == code)
                .map(|c| c.1.clone())
                .unwrap_or_else(|| format!("缺失 {code:#05x}"))
        };
        // 内核按 usage 序号给键码，必须按 HID 表显示（用户报告的「亮错格」）
        assert_eq!(label(0x130), "A");
        assert_eq!(label(0x131), "B");
        assert_eq!(label(0x132), "X"); // BTN_C 其实是 X
        assert_eq!(label(0x133), "Y");
        assert_eq!(label(0x134), "LB"); // BTN_WEST 其实是 LB（语义表会错标成 X）
        assert_eq!(label(0x135), "RB"); // BTN_Z 其实是 RB
        assert_eq!(label(0x136), "Back"); // BTN_TL 其实是 Back（语义表会错标成 LB）
        assert_eq!(label(0x137), "Start");
        assert_eq!(label(0x138), "LS"); // BTN_TL2 其实是 LS 按下
        assert_eq!(label(0x139), "RS");
        assert_eq!(label(0x08b), "Guide"); // KEY_MENU = Xbox 导引键
        // 10 个位域键 + Guide + 十字键 4 格 = 15（5 列正好 3 行）
        assert_eq!(present.len(), 15);
    }

    #[test]
    fn 语义式_虚拟手柄标签保持不变() {
        // uinput 虚拟手柄注册了整个 BTN 段（0x130..=0x13e），仍要判成语义式
        let caps = DeviceCaps {
            name: "手柄A 测试".into(),
            node: "/dev/input/event23".into(),
            axes: [(0x10, -1, 1), (0x11, -1, 1)]
                .into_iter()
                .map(|(code, min, max)| (code, axis(code, min, max, 0, 0)))
                .collect(),
            buttons: (0x130..=0x13e).collect(),
            ff_rumble: false,
        };
        assert_eq!(convention(&caps), Convention::Semantic);

        let present = present_buttons(&caps, &BTreeSet::new(), &BTreeMap::new());
        let label = |code: u16| {
            present
                .iter()
                .find(|c| c.0 == code)
                .map(|c| c.1.clone())
                .unwrap_or_else(|| format!("缺失 {code:#05x}"))
        };
        assert_eq!(label(0x134), "X");
        assert_eq!(label(0x136), "LB");
        assert_eq!(label(0x13a), "Back");
        assert_eq!(label(0x13c), "Guide");
        // 11 个标准键 + 十字键 4 格；语义表没收录的 C/Z/TL2/TR2 不常驻（按到才补位）
        assert_eq!(present.len(), 15);
    }

    #[test]
    fn 十字键_轴合成_中立占格按下才亮() {
        let caps = xbox_bt_caps();
        let empty = BTreeSet::new();

        // 中立位：四个格子都在（暗格），不闪不跳
        let present = present_buttons(&caps, &empty, &BTreeMap::new());
        for code in [0x220, 0x221, 0x222, 0x223] {
            let cell = present
                .iter()
                .find(|c| c.0 == code)
                .expect("十字键格子缺失");
            assert!(!cell.2, "{code:#05x} 中立位不该亮");
        }

        // 按「上 + 左」（HAT 值 -1 = 上/左，+1 = 下/右）
        let axes: BTreeMap<u16, i32> = [(0x10, -1), (0x11, -1)].into_iter().collect();
        let present = present_buttons(&caps, &empty, &axes);
        assert!(present.iter().any(|c| c.0 == 0x220 && c.2), "上 没亮");
        assert!(present.iter().any(|c| c.0 == 0x222 && c.2), "左 没亮");
        assert!(!present.iter().any(|c| c.0 == 0x221 && c.2), "下 不该亮");
        assert!(!present.iter().any(|c| c.0 == 0x223 && c.2), "右 不该亮");
    }

    /// 造一个 `AxisInfo`（测试用）。
    fn axis(code: u16, min: i32, max: i32, fuzz: i32, flat: i32) -> AxisInfo {
        AxisInfo {
            code,
            name: String::new(),
            min,
            max,
            fuzz,
            flat,
            resolution: 0,
        }
    }

    /// 造一个只有给定轴的 `DeviceCaps`（测试用）。
    fn caps_with(axes: Vec<AxisInfo>) -> DeviceCaps {
        DeviceCaps {
            name: "测试手柄".into(),
            node: "/dev/input/event0".into(),
            axes: axes.into_iter().map(|a| (a.code, a)).collect(),
            buttons: vec![],
            ff_rumble: false,
        }
    }

    #[test]
    fn 键程当量行_按能力生成且缺轴降级() {
        // 一个轴都没有 → 空表（页面回占位，不报错）
        assert!(gauge_rows(&caps_with(vec![])).is_empty());

        // 只声明左摇杆两轴 + 两个扳机 → 恰好 4 行，右摇杆整块跳过
        let caps = caps_with(vec![
            axis(0x00, -32768, 32767, 0, 8),
            axis(0x01, -32768, 32767, 0, 8),
            axis(0x02, 0, 1023, 0, 0),
            axis(0x05, 0, 1023, 0, 0),
        ]);
        let rows = gauge_rows(&caps);
        let names: Vec<&str> = rows.iter().map(|r| r.name).collect();
        assert_eq!(names, ["左摇杆 X", "左摇杆 Y", "LT", "RT"]);
        assert_eq!((rows[0].stick, rows[2].stick), (true, false));

        // 只有扳机（不少老手柄/纯赛车设备）→ 只剩两行，且都不是摇杆语义
        let caps = caps_with(vec![axis(0x02, 0, 255, 0, 0), axis(0x05, 0, 255, 0, 0)]);
        let rows = gauge_rows(&caps);
        assert_eq!(
            rows.iter().map(|r| r.name).collect::<Vec<_>>(),
            ["LT", "RT"]
        );
        assert!(rows.iter().all(|r| !r.stick));
    }

    #[test]
    fn 键程当量显示_平坦区吸附与不同量程换算() {
        // 摇杆 -32768..32767，平坦区 ±1000
        let stick = axis(0x00, -32768, 32767, 0, 1000);
        // 平坦区内两端读数完全一致（吸附到中点）→ 50% 附近不再抖
        let (pct_lo, snap_lo) = gauge_display(&stick, -900, true);
        let (pct_hi, snap_hi) = gauge_display(&stick, 900, true);
        assert!(snap_lo && snap_hi);
        assert!((pct_lo - pct_hi).abs() < 1e-9);
        assert!((pct_lo - 50.0).abs() < 0.01);
        // 区间外不吸附，按 raw 换算
        let (pct, snap) = gauge_display(&stick, -20000, true);
        assert!(!snap);
        assert!((pct - (-20000 + 32768) as f64 / 65535.0 * 100.0).abs() < 1e-9);

        // 扳机 0..255：即便 raw 正好是中点也不吸附（扳机没有居中语义）
        let trig = axis(0x02, 0, 255, 10, 100);
        let (pct, snap) = gauge_display(&trig, 128, false);
        assert!(!snap);
        assert!((pct - 128.0 / 255.0 * 100.0).abs() < 1e-9);

        // P4-4 离线断言：不同量程同一 raw 的百分比各自按 span 换算
        let (pct, _) = gauge_display(&trig, 64, false);
        assert!((pct - 64.0 / 255.0 * 100.0).abs() < 1e-9); // 0..255 → 25.1%
        let trig2 = axis(0x05, 0, 1023, 0, 0);
        let (pct, _) = gauge_display(&trig2, 64, false);
        assert!((pct - 64.0 / 1023.0 * 100.0).abs() < 1e-9); // 0..1023 → 6.3%
        let big = axis(0x01, -32768, 32767, 0, 0);
        let (pct, _) = gauge_display(&big, 0, false);
        assert!((pct - 32768.0 / 65535.0 * 100.0).abs() < 1e-6); // -32768..32767
    }
}
