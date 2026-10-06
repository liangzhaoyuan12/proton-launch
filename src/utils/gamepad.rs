//! 手柄（游戏控制器）检测：解析内核输入设备清单、识别协议、监听热插拔。
//!
//! 本模块属于逻辑层，禁止 `use gtk` / `use adw` / `use glib`：
//! 输出纯数据 [`GamepadInfo`]，由 `widgets::gamepad_indicator` 渲染，
//! 由 `app` 负责接线、Toast 与桌面通知。
//!
//! 数据源是 `/proc/bus/input/devices`（每个内核输入设备一个块）。位图按内核
//! `input_print_bitmap` 的规则解析：**最高位字（word N）排在最前，word0 在最后**。
//! 该规则已用本机真实设备反推验证：USB 鼠标的 `KEY=1f0000 0 0 0 0` 对应
//! BTN_LEFT..BTN_EXTRA（位 272..276），只有「最高位字优先」才能命中。
//!
//! 热插拔：后台线程监听 `/dev/input` 的 inotify 事件（1 秒超时兜底轮询），
//! 每次唤醒重新扫描，把「变化后的完整快照」推进队列，由主循环定时取走。
//! 软件关闭后不驻留、不监听，因此不存在后台通知。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 内核输入设备清单。
const DEVICES_PATH: &str = "/proc/bus/input/devices";
/// 测试钩子：指向替身文件即可离线验证解析（`scan()` 与监听线程都遵循它）。
const DEVICES_PATH_ENV: &str = "PROTON_LAUNCH_INPUT_DEVICES";
/// inotify 监听目录；设备节点在这里创建/删除。
const INPUT_DIR: &str = "/dev/input";
/// 兜底复核间隔（毫秒）：即便 inotify 不可用，也保证每秒复核一次。
const RESCAN_MS: i32 = 1000;

// ─── 数据结构 ─────────────────────────────────────────────────────────────

/// 手柄协议（用于卡片与通知文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Xbox360,
    XboxOne,
    XboxSeries,
    /// 微软系手柄但型号未在已知 PID 表内。
    Xbox,
    Ps4,
    Ps5,
    /// 索尼系手柄但型号未在已知 PID 表内。
    Playstation,
    Nintendo,
    Generic,
}

impl Protocol {
    pub fn label(&self) -> &'static str {
        match self {
            Protocol::Xbox360 => "Xbox 360",
            Protocol::XboxOne => "Xbox One",
            // XSS 与 XSX 使用同一款手柄，HID 描述符与 PID 完全一致，无法区分。
            Protocol::XboxSeries => "Xbox Series X|S（XSS/XSX）",
            Protocol::Xbox => "Xbox 系手柄（型号未知）",
            Protocol::Ps4 => "PS4（DualShock 4）",
            Protocol::Ps5 => "PS5（DualSense）",
            Protocol::Playstation => "索尼 PlayStation 手柄",
            Protocol::Nintendo => "任天堂 Switch",
            Protocol::Generic => "通用 HID 手柄",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Protocol::Xbox360 => "xbox360",
            Protocol::XboxOne => "xbox-one",
            Protocol::XboxSeries => "xss-xsx",
            Protocol::Xbox => "xbox",
            Protocol::Ps4 => "ps4",
            Protocol::Ps5 => "ps5",
            Protocol::Playstation => "playstation",
            Protocol::Nintendo => "nintendo",
            Protocol::Generic => "generic",
        }
    }
}

/// 一个已连接的手柄（对应一个内核 event 节点）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamepadInfo {
    /// 设备名（内核 HID/USB 产品串，如 `Microsoft X-Box One pad`）。
    pub name: String,
    pub protocol: Protocol,
    /// `/dev/input/eventN`，同时作为唯一主键。
    pub event_node: String,
    /// `/dev/input/jsN`，内核 joydev 未创建时为 `None`。
    pub js_node: Option<String>,
    pub bus: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
    pub phys: String,
    /// 蓝牙 MAC 或 USB 序列号。
    pub uniq: String,
    /// `/sys` 下的设备路径（`/proc` 里省略了 `/sys` 前缀）。
    pub sysfs: String,
    pub driver: Option<String>,
    /// 可用按键数（KEY 位图中 ≥0x100 的位）。
    pub buttons: usize,
    /// 轴数（ABS 位图置位数，含 HAT）。
    pub axes: usize,
    pub force_feedback: bool,
}

impl GamepadInfo {
    pub fn id(&self) -> &str {
        &self.event_node
    }

    /// `xxxx:yyyy` 形式的厂商:产品 ID。
    pub fn vendor_product(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor, self.product)
    }

    /// 原始输入 ID（内核 `I:` 行）。
    pub fn input_id(&self) -> String {
        format!(
            "bus=0x{:04x} vendor=0x{:04x} product=0x{:04x} version=0x{:04x}",
            self.bus, self.vendor, self.product, self.version
        )
    }

    pub fn bus_name(&self) -> &'static str {
        match self.bus {
            0x01 => "PCI",
            0x03 => "USB",
            0x04 => "GamePort",
            0x05 => "蓝牙",
            0x06 => "虚拟设备",
            0x10 => "ISA",
            0x11 => "i8042（PS/2）",
            0x19 => "平台总线",
            _ => "其他",
        }
    }

    /// `连接总线` 行的展示值。
    pub fn bus_label(&self) -> String {
        format!("{}（0x{:04x}）", self.bus_name(), self.bus)
    }

    /// `能力` 行的展示值。
    pub fn capabilities(&self) -> String {
        format!(
            "按键 {} 个 · 轴 {} 个 · 力反馈{}",
            self.buttons,
            self.axes,
            if self.force_feedback {
                "支持"
            } else {
                "不支持"
            }
        )
    }
}

// ─── 扫描 / 解析 ──────────────────────────────────────────────────────────

/// 扫描当前系统。可用 `PROTON_LAUNCH_INPUT_DEVICES` 指向替身文件（测试用）。
pub fn scan() -> Vec<GamepadInfo> {
    let path = std::env::var(DEVICES_PATH_ENV).unwrap_or_else(|_| DEVICES_PATH.to_string());
    scan_path(Path::new(&path))
}

/// 从指定文件扫描（`PROTON_LAUNCH_INPUT_DEVICES` 的实现入口，也是单测入口）。
pub fn scan_path(path: &Path) -> Vec<GamepadInfo> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) => {
            eprintln!("读取输入设备清单失败 {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// 解析 `/proc/bus/input/devices` 全文，只保留手柄，按（名称，节点序号）排序。
pub fn parse(text: &str) -> Vec<GamepadInfo> {
    let mut pads: Vec<GamepadInfo> = text.split("\n\n").filter_map(parse_block).collect();
    pads.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| event_index(&a.event_node).cmp(&event_index(&b.event_node)))
    });
    pads
}

/// `/dev/input/event12` → 12（多手柄同名时保持稳定顺序）。
fn event_index(node: &str) -> usize {
    node.rsplit("event")
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX)
}

fn parse_block(block: &str) -> Option<GamepadInfo> {
    let (mut bus, mut vendor, mut product, mut version) = (0u16, 0u16, 0u16, 0u16);
    let mut name = String::new();
    let mut phys = String::new();
    let mut uniq = String::new();
    let mut sysfs = String::new();
    let mut handlers: Vec<String> = Vec::new();
    let (mut ev, mut key, mut abs) = (0u64, Vec::new(), Vec::new());

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("I: ") {
            for token in rest.split_whitespace() {
                let Some((field, value)) = token.split_once('=') else {
                    continue;
                };
                let Ok(number) = u16::from_str_radix(value, 16) else {
                    continue;
                };
                match field {
                    "Bus" => bus = number,
                    "Vendor" => vendor = number,
                    "Product" => product = number,
                    "Version" => version = number,
                    _ => {}
                }
            }
        } else if let Some(rest) = line.strip_prefix("N: Name=\"") {
            name = rest
                .rsplit_once('"')
                .map(|(left, _)| left)
                .unwrap_or(rest)
                .to_string();
        } else if let Some(rest) = line.strip_prefix("P: Phys=") {
            phys = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("U: Uniq=") {
            uniq = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("S: Sysfs=") {
            sysfs = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
            handlers = rest.split_whitespace().map(str::to_string).collect();
        } else if let Some(rest) = line.strip_prefix("B: ") {
            if let Some(value) = rest.strip_prefix("EV=") {
                ev = u64::from_str_radix(value.trim(), 16).unwrap_or(0);
            } else if let Some(value) = rest.strip_prefix("KEY=") {
                key = bitmap(value);
            } else if let Some(value) = rest.strip_prefix("ABS=") {
                abs = bitmap(value);
            }
        }
    }

    if !is_gamepad(ev, &key, &abs, &handlers) {
        return None;
    }
    let event = handlers.iter().find(|h| h.starts_with("event"))?;

    Some(GamepadInfo {
        protocol: classify(vendor, product, &name),
        js_node: handlers
            .iter()
            .find(|h| h.starts_with("js"))
            .map(|h| format!("/dev/input/{h}")),
        event_node: format!("/dev/input/{event}"),
        driver: read_driver(&sysfs),
        buttons: count_bits_from(&key, 0x100),
        axes: abs.iter().map(|w| w.count_ones() as usize).sum(),
        force_feedback: ev & (1 << 21) != 0,
        name,
        bus,
        vendor,
        product,
        version,
        phys,
        uniq,
        sysfs,
    })
}

/// 是否手柄：有 ABS 轴，且（命中手柄按键区 或 内核 joydev 建了 js 节点）。
///
/// 按键区取 0x120..=0x13f（BTN_TRIGGER..BTN_MODE/THUMBR）与 0x220..=0x223（BTN_DPAD_*），
/// 刻意排除 0x110..=0x11f（鼠标键）与 0x14a（BTN_TOUCH）——否则 DS4/DualSense 的
/// 触控板节点、带 ABS 的多媒体键盘都会被误判。
fn is_gamepad(ev: u64, key: &[u64], abs: &[u64], handlers: &[String]) -> bool {
    let has_abs = ev & (1 << 3) != 0 || !abs.is_empty();
    if !has_abs {
        return false;
    }
    let pad_buttons = (0x120u32..=0x13f)
        .chain(0x220u32..=0x223)
        .any(|bit| bit_set(key, bit));
    let has_js = handlers.iter().any(|h| h.starts_with("js"));
    pad_buttons || has_js
}

/// 内核位图解析：`/proc` 先打印最高位字，需反转后才是 word0, word1, …。
fn bitmap(text: &str) -> Vec<u64> {
    let mut words: Vec<u64> = text
        .split_whitespace()
        .filter_map(|token| u64::from_str_radix(token, 16).ok())
        .collect();
    words.reverse();
    words
}

fn bit_set(words: &[u64], bit: u32) -> bool {
    words
        .get((bit / 64) as usize)
        .is_some_and(|word| (word >> (bit % 64)) & 1 == 1)
}

fn count_bits_from(words: &[u64], from: u32) -> usize {
    let total = (words.len() * 64) as u32;
    (from..total).filter(|bit| bit_set(words, *bit)).count()
}

/// 从 sysfs 向上逐级找 `driver` 符号链接（输入设备自身没有，父 HID/serio 设备才有）。
fn read_driver(sysfs: &str) -> Option<String> {
    if sysfs.is_empty() {
        return None;
    }
    let mut path = PathBuf::from("/sys").join(sysfs.trim_start_matches('/'));
    for _ in 0..12 {
        if let Ok(link) = fs::read_link(path.join("driver")) {
            let name = link
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| link.to_string_lossy().into_owned());
            return Some(name);
        }
        path = path.parent()?.to_path_buf();
    }
    None
}

// ─── 协议识别 ─────────────────────────────────────────────────────────────

/// 识别协议：先用设备名（xpad / hid-* 驱动给出的型号名最可靠，含第三方品牌），
/// 再回落到内核驱动的 VID:PID 表。
///
/// PID 表来源：
/// - 微软 `drivers/input/joystick/xpad.c` 的 `xpad_device[]`
/// - 索尼 `drivers/hid/hid-ids.h`：PS4 = 05c4/09cc/0ba0，PS5 = 0ce6/0df2
/// - 任天堂 `drivers/hid/hid-ids.h`：Joy-Con L/R = 2006/2007，Pro = 2009，
///   充电握把 = 200e，SNES/N64/NGC 复刻手柄 = 2017/2019/201e
fn classify(vendor: u16, product: u16, name: &str) -> Protocol {
    let n = name.to_lowercase();

    // 1) 设备名
    if n.contains("xbox 360") || n.contains("x-box 360") {
        return Protocol::Xbox360;
    }
    if n.contains("xbox one") || n.contains("x-box one") {
        return Protocol::XboxOne;
    }
    if n.contains("xbox series") {
        return Protocol::XboxSeries;
    }
    if n.contains("dualsense") {
        return Protocol::Ps5;
    }
    if n.contains("dualshock") {
        return Protocol::Ps4;
    }
    if n.contains("nintendo") && (n.contains("pro controller") || n.contains("joy-con")) {
        return Protocol::Nintendo;
    }

    // 2) VID:PID
    match vendor {
        0x045e => match product {
            0x0202 | 0x0285 | 0x0287 | 0x0288 | 0x0289 | 0x028e | 0x028f | 0x0291 | 0x02a9
            | 0x0719 => Protocol::Xbox360,
            0x02d1 | 0x02dd | 0x02e0 | 0x02e3 | 0x02ea | 0x0b00 | 0x0b0a => Protocol::XboxOne,
            0x02fd | 0x0b12 | 0x0b13 | 0x0b20 => Protocol::XboxSeries,
            _ => Protocol::Xbox,
        },
        0x054c => match product {
            0x05c4 | 0x09cc | 0x0ba0 => Protocol::Ps4,
            0x0ce6 | 0x0df2 => Protocol::Ps5,
            _ => Protocol::Playstation,
        },
        0x057e => {
            if n.contains("nintendo") || n.contains("switch") || n.contains("joy-con") {
                Protocol::Nintendo
            } else {
                match product {
                    0x2006 | 0x2007 | 0x2009 | 0x200e | 0x2017 | 0x2019 | 0x201e => {
                        Protocol::Nintendo
                    }
                    _ => Protocol::Generic,
                }
            }
        }
        _ => Protocol::Generic,
    }
}

// ─── 热插拔监听 ───────────────────────────────────────────────────────────

/// 热插拔监听器：后台线程 + 队列，主线程定时 [`Monitor::drain`]。
pub struct Monitor {
    queue: Arc<Mutex<Vec<Vec<GamepadInfo>>>>,
    stop: Arc<AtomicBool>,
}

impl Monitor {
    /// `baseline` 是启动时已有的手柄（作为对比基线，**不会**入队，
    /// 因此软件打开前就插着的手柄不触发通知）。
    pub fn start(baseline: Vec<GamepadInfo>) -> Self {
        let queue = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let queue = Arc::clone(&queue);
            let stop = Arc::clone(&stop);
            thread::spawn(move || watch(baseline, queue, stop));
        }
        Self { queue, stop }
    }

    /// 主线程取走队列里积累的快照（通常为空）。
    pub fn drain(&self) -> Vec<Vec<GamepadInfo>> {
        self.queue
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }

    /// 退出时通知线程（最多 1 秒内结束）。
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn watch(
    mut last: Vec<GamepadInfo>,
    queue: Arc<Mutex<Vec<Vec<GamepadInfo>>>>,
    stop: Arc<AtomicBool>,
) {
    let fd = init_inotify();
    while !stop.load(Ordering::Relaxed) {
        if fd >= 0 {
            wait_readable(fd, RESCAN_MS);
            drain_inotify(fd);
        } else {
            thread::sleep(Duration::from_millis(RESCAN_MS as u64));
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let now = scan();
        if now != last {
            last = now.clone();
            if let Ok(mut queue) = queue.lock() {
                queue.push(now);
            }
        }
    }
    if fd >= 0 {
        unsafe {
            libc::close(fd);
        }
    }
}

/// 初始化 `/dev/input` 的 inotify 监听；失败返回 -1（调用方退回定时轮询）。
fn init_inotify() -> i32 {
    unsafe {
        let fd = libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC);
        if fd < 0 {
            return -1;
        }
        let mask = libc::IN_CREATE
            | libc::IN_ATTRIB
            | libc::IN_DELETE
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO;
        if libc::inotify_add_watch(fd, INPUT_DIR.as_ptr().cast(), mask) < 0 {
            libc::close(fd);
            return -1;
        }
        fd
    }
}

fn wait_readable(fd: i32, timeout_ms: i32) {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe {
        libc::poll(&mut pollfd, 1, timeout_ms);
    }
}

fn drain_inotify(fd: i32) {
    let mut buffer = [0u8; 4096];
    loop {
        let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        if read <= 0 {
            break;
        }
    }
}

// ─── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 本机真实 USB 鼠标：BTN_LEFT..BTN_EXTRA = 位 272..276。
    /// 只有「最高位字优先」的解析规则能把 0x1f0000 放到 word4（位 256..319）。
    #[test]
    fn 内核位图按最高位字优先排列() {
        let words = bitmap("1f0000 0 0 0 0");
        assert!(bit_set(&words, 272), "位 272（BTN_LEFT）应被命中");
        assert!(!bit_set(&words, 20), "按最低位字解析会误命中位 20，应为空");
        assert_eq!(words.len(), 5);
    }

    const XBOX360: &str = r#"
I: Bus=0003 Vendor=045e Product=028e Version=0110
N: Name="Microsoft X-Box 360 pad"
P: Phys=usb-0000:00:14.0-2/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.0/input/input20
U: Uniq=
H: Handlers=js0 event20
B: PROP=0
B: EV=30001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    const XBOX_ONE: &str = r#"
I: Bus=0003 Vendor=045e Product=02dd Version=0114
N: Name="Microsoft X-Box One pad"
P: Phys=usb-0000:00:14.0-3/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-3/1-3:1.0/input/input30
U: Uniq=
H: Handlers=js1 event30
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    /// 蓝牙下 Series 与 One S 的设备名都是 "Xbox Wireless Controller"，只能靠 PID 区分。
    const XBOX_SERIES_BT: &str = r#"
I: Bus=0005 Vendor=045e Product=0b13 Version=111c
N: Name="Xbox Wireless Controller"
P: Phys=
S: Sysfs=/devices/virtual/misc/empty/input/input40
U: Uniq=8c:ce:4e:11:22:33
H: Handlers=js2 event40
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    /// DualSense：一个物理手柄占 3 个节点（本体 / 体感 / 触控板），只有本体该被统计。
    const DUALSENSE_SET: &str = r#"
I: Bus=0005 Vendor=054c Product=0ce6 Version=8111
N: Name="Sony Interactive Entertainment DualSense Wireless Controller"
P: Phys=
S: Sysfs=/devices/virtual/misc/empty/input/input50
U: Uniq=ac:83:e3:aa:bb:cc
H: Handlers=js3 event50
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f

I: Bus=0005 Vendor=054c Product=0ce6 Version=8111
N: Name="Sony Interactive Entertainment DualSense Wireless Controller Motion Sensors"
P: Phys=
S: Sysfs=/devices/virtual/misc/empty/input/input51
U: Uniq=ac:83:e3:aa:bb:cc
H: Handlers=event51
B: PROP=0
B: EV=000009
B: ABS=0000fff

I: Bus=0005 Vendor=054c Product=0ce6 Version=8111
N: Name="Sony Interactive Entertainment DualSense Wireless Controller Touchpad"
P: Phys=
S: Sysfs=/devices/virtual/misc/empty/input/input52
U: Uniq=ac:83:e3:aa:bb:cc
H: Handlers=mouse2 event52
B: PROP=0
B: EV=10001f
B: KEY=400 3f0000 0 0 0 0
B: ABS=10003f
"#;

    const SWITCH_PRO: &str = r#"
I: Bus=0005 Vendor=057e Product=2009 Version=8111
N: Name="Nintendo Switch Pro Controller"
P: Phys=
S: Sysfs=/devices/virtual/misc/empty/input/input60
U: Uniq=98:b6:e9:11:22:33
H: Handlers=js4 event60
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    const STEAM_VIRTUAL: &str = r#"
I: Bus=0006 Vendor=28de Product=1142 Version=0100
N: Name="Steam Virtual Gamepad"
P: Phys=virtual
S: Sysfs=/devices/virtual/misc/empty/input/input70
U: Uniq=
H: Handlers=js5 event70
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    /// 本机真实设备（非手柄）：带 ABS 的多媒体键盘 + 普通鼠标。
    const REAL_NON_GAMEPAD: &str = r#"
I: Bus=0003 Vendor=4e53 Product=5407 Version=0111
N: Name="USB OPTICAL MOUSE "
P: Phys=usb-0000:00:14.0-6.3/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-6/1-6.3/1-6.3:1.0/0003:4E53:5407.0002/input/input3
U: Uniq=
H: Handlers=mouse0 event3
B: PROP=0
B: EV=17
B: KEY=1f0000 0 0 0 0
B: REL=903
B: MSC=10

I: Bus=0003 Vendor=4e53 Product=5407 Version=0110
N: Name="USB OPTICAL MOUSE  Keyboard"
P: Phys=usb-0000:00:14.0-6.3/input1
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-6/1-6.3/1-6.3:1.0/0003:4E53:5407.0003/input/input4
U: Uniq=
H: Handlers=sysrq kbd event4
B: EV=10001f
B: KEY=733eff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17c007 ffff7bfad941dfff febeffdfffefffff fffffffffffffffe
B: ABS=100000000
B: MSC=10
"#;

    #[test]
    fn 解析单个手柄的全部字段() {
        let pads = parse(XBOX360);
        assert_eq!(pads.len(), 1);
        let pad = &pads[0];
        assert_eq!(pad.name, "Microsoft X-Box 360 pad");
        assert_eq!(pad.protocol, Protocol::Xbox360);
        assert_eq!(pad.protocol.label(), "Xbox 360");
        assert_eq!(pad.event_node, "/dev/input/event20");
        assert_eq!(pad.js_node.as_deref(), Some("/dev/input/js0"));
        assert_eq!(pad.vendor_product(), "045e:028e");
        assert_eq!(pad.bus_name(), "USB");
        assert_eq!(pad.buttons, 15, "位 304..318 共 15 个按键");
        assert_eq!(pad.axes, 8, "ABS_X/Y/Z/RX/RY/RZ + HAT0X/Y");
        assert!(pad.force_feedback);
        assert_eq!(pad.phys, "usb-0000:00:14.0-2/input0");
    }

    #[test]
    fn 协议识别_名称优先() {
        assert_eq!(parse(XBOX_ONE)[0].protocol, Protocol::XboxOne);
        assert_eq!(parse(SWITCH_PRO)[0].protocol, Protocol::Nintendo);
        // 蓝牙下名称不含系列，靠 045e:0b13 判定 Series
        assert_eq!(parse(XBOX_SERIES_BT)[0].protocol, Protocol::XboxSeries);
        assert_eq!(parse(STEAM_VIRTUAL)[0].protocol, Protocol::Generic);
    }

    #[test]
    fn 协议识别_pid表() {
        // One S 蓝牙 PID 与 Series 蓝牙 PID 同名，必须靠 PID 区分
        assert_eq!(
            classify(0x045e, 0x02e0, "Xbox Wireless Controller"),
            Protocol::XboxOne
        );
        assert_eq!(
            classify(0x045e, 0x0b13, "Xbox Wireless Controller"),
            Protocol::XboxSeries
        );
        assert_eq!(
            classify(0x045e, 0x028e, "Controller (XBOX 360 For Windows)"),
            Protocol::Xbox360
        );
        assert_eq!(
            classify(0x054c, 0x09cc, "Wireless Controller"),
            Protocol::Ps4
        );
        assert_eq!(
            classify(0x054c, 0x0df2, "Wireless Controller"),
            Protocol::Ps5
        );
        assert_eq!(
            classify(0x057e, 0x2006, "Nintendo Joy-Con Left"),
            Protocol::Nintendo
        );
        // 未知型号给出诚实的兜底，而不是瞎猜
        assert_eq!(classify(0x045e, 0x9999, "Mystery Pad"), Protocol::Xbox);
        assert_eq!(
            classify(0x054c, 0x9999, "Mystery Pad"),
            Protocol::Playstation
        );
        assert_eq!(classify(0x1234, 0x5678, "Mystery Pad"), Protocol::Generic);
    }

    #[test]
    fn dualsense_只统计本体节点() {
        let pads = parse(DUALSENSE_SET);
        assert_eq!(pads.len(), 1, "体感与触控板节点不应计入手柄数");
        let pad = &pads[0];
        assert_eq!(pad.event_node, "/dev/input/event50");
        assert_eq!(pad.protocol, Protocol::Ps5);
        assert_eq!(pad.js_node.as_deref(), Some("/dev/input/js3"));
        assert_eq!(pad.uniq, "ac:83:e3:aa:bb:cc");
        assert_eq!(pad.vendor_product(), "054c:0ce6");
    }

    #[test]
    fn 多手柄按名称与节点排序且各自独立() {
        // 两个同名 360 手柄（event5 / event20）：同名时按节点**数字**排序，
        // 字符串排序会把 event20 排到 event5 前面。
        let twin = XBOX360.replace("event20", "event5");
        let text = format!("{XBOX360}\n{twin}\n{XBOX_ONE}\n{XBOX_SERIES_BT}\n{SWITCH_PRO}");
        let pads = parse(&text);
        assert_eq!(pads.len(), 5);
        let nodes: Vec<&str> = pads.iter().map(|p| p.event_node.as_str()).collect();
        assert_eq!(
            nodes,
            vec![
                "/dev/input/event5",
                "/dev/input/event20",
                "/dev/input/event30",
                "/dev/input/event60",
                "/dev/input/event40",
            ],
            "先按设备名、再按 event 序号稳定排序"
        );
        let unique: std::collections::HashSet<&str> = nodes.iter().copied().collect();
        assert_eq!(unique.len(), 5, "每个 event 节点一条记录");
    }

    #[test]
    fn 键盘与鼠标不会被误判为手柄() {
        assert!(
            parse(REAL_NON_GAMEPAD).is_empty(),
            "带 ABS 的键盘/鼠标不应入选"
        );
        // 触控板的 BTN_TOUCH(0x14a) 与 BTN_MOUSE(0x110) 都在按键判定区之外
        let touchpad_only = r#"
I: Bus=0005 Vendor=054c Product=05c4 Version=0100
N: Name="Wireless Controller Touchpad"
H: Handlers=mouse3 event80
B: EV=10001f
B: KEY=400 3f0000 0 0 0 0
B: ABS=10003f
"#;
        assert!(parse(touchpad_only).is_empty());
    }

    /// 本机真实清单：允许环境里插着手柄，但键鼠绝不能入选。
    #[test]
    fn 真实清单里键鼠不入选() {
        let Ok(text) = fs::read_to_string(DEVICES_PATH) else {
            return;
        };
        for pad in parse(&text) {
            let n = pad.name.to_lowercase();
            assert!(
                !n.contains("keyboard") && !n.contains("mouse"),
                "键鼠被误判成手柄: {}",
                pad.name
            );
        }
    }

    #[test]
    fn 缺失文件返回空列表而不是panic() {
        assert!(scan_path(Path::new("/nonexistent/input/devices")).is_empty());
    }

    /// `PROTON_LAUNCH_INPUT_DEVICES` 可把扫描源换成替身文件（离线验证 / 排障）。
    #[test]
    fn 环境变量可指定替身清单() {
        let path = std::env::temp_dir().join("proton-launch-fake-input-devices.txt");
        fs::write(&path, XBOX360).expect("写入替身清单失败");
        // 与其它测试并行时互不影响：只有本用例读该变量
        unsafe { std::env::set_var(DEVICES_PATH_ENV, &path) };
        let pads = scan();
        unsafe { std::env::remove_var(DEVICES_PATH_ENV) };
        let _ = fs::remove_file(&path);

        assert_eq!(pads.len(), 1, "替身清单里的手柄应被扫出");
        assert_eq!(pads[0].name, "Microsoft X-Box 360 pad");
    }
}
