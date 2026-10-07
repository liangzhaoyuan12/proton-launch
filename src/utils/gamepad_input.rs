//! 手柄输入读取层（GOAL.md P1）：evdev 非阻塞读 → 快照 → 通道。
//!
//! **分层约定**：本模块不依赖 `gtk` / `adw` / `glib`，只输出纯数据
//! （`DeviceCaps` / `InputSnapshot`），渲染在 `src/widgets/`，接线在 `src/app.rs`。
//!
//! 线程模型：`InputReader::spawn` 起一条读取线程，线程持有 fd，用 `poll(16ms)`
//! 做合帧节流（≤60fps），通过 `mpsc` 把 `InputMsg` 推给主循环；`Drop` 时置停止
//! 标志并 join（≤16ms 返回），保证「关窗即彻底退出」。
//!
//! 轴范围一律来自 `EVIOCGABS`（min/max/fuzz/flat），不硬编码 —— 键程当量换算
//! 因此对任意手柄都成立（A13、P4）。

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::io;
use std::os::fd::RawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// ── 内核常量（linux/input-event-codes.h、linux/input.h、linux/ioctl.h）────
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const FF_RUMBLE: u16 = 0x50;

const O_RDONLY: libc::c_int = 0;
const O_NONBLOCK: libc::c_int = 0o4000;
const O_CLOEXEC: libc::c_int = 0o2000000;
const POLLIN: i16 = 0x001;
const POLLERR: i16 = 0x008;
const POLLHUP: i16 = 0x010;
const POLLNVAL: i16 = 0x020;
const POLL_TIMEOUT_MS: i32 = 16; // ≤60fps 合帧节流

const IOC_READ: u64 = 2;

/// 现场算 `_IOC(_IOC_READ, 'E', nr, size)`，与 python/C 探针同一算法。
const fn ioc(dir: u64, ty: u8, nr: u64, size: u64) -> libc::c_ulong {
    ((dir << 30) | (size << 16) | ((ty as u64) << 8) | nr) as libc::c_ulong
}
const fn eviocgname(len: u64) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x06, len)
}
const fn eviocgkey(len: u64) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x18, len)
}
const fn eviocgbit(ev: u64, len: u64) -> libc::c_ulong {
    ioc(IOC_READ, b'E', 0x20 + ev, len)
}
const fn eviocgabs(abs: u64) -> libc::c_ulong {
    ioc(
        IOC_READ,
        b'E',
        0x40 + abs,
        std::mem::size_of::<AbsInfo>() as u64,
    )
}

/// `struct input_absinfo`（linux/input.h，6 × __s32）
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

/// `struct input_event`（x86_64：timeval 16B + type/code/value）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InputEvent {
    time_sec: libc::time_t,
    time_usec: libc::suseconds_t,
    r#type: u16,
    code: u16,
    value: i32,
}

// ── 输出数据 ────────────────────────────────────────────────────────────

/// 一个轴的元数据（全部来自 `EVIOCGABS`）。
#[derive(Debug, Clone, PartialEq)]
pub struct AxisInfo {
    pub code: u16,
    pub name: String,
    pub min: i32,
    pub max: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

impl AxisInfo {
    /// 键程当量：把原始值换算成 0–100%（A13）。
    pub fn percent(&self, raw: i32) -> f64 {
        let span = self.max - self.min;
        if span <= 0 {
            return 0.0;
        }
        let ratio = (raw - self.min) as f64 / span as f64;
        (ratio * 100.0).clamp(0.0, 100.0)
    }

    /// 轴的中点（摇杆居中标线用）。
    pub fn midpoint(&self) -> i32 {
        self.min + (self.max - self.min) / 2
    }

    /// 是否落在平坦区/死区内（`flat` + `fuzz`，避免中点读数抖动，A13）。
    pub fn in_deadband(&self, raw: i32) -> bool {
        let tolerance = self.flat.max(self.fuzz);
        (raw - self.midpoint()).abs() <= tolerance
    }
}

/// 设备能力（打开时读一次，之后不变）。
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCaps {
    pub node: String,
    pub name: String,
    /// code → 元数据，按键序遍历稳定
    pub axes: BTreeMap<u16, AxisInfo>,
    /// 支持的按键 code 列表
    pub buttons: Vec<u16>,
    /// 是否声明 `FF_RUMBLE`（P5 振动测试的置灰依据）
    pub ff_rumble: bool,
}

impl DeviceCaps {
    pub fn axis(&self, code: u16) -> Option<&AxisInfo> {
        self.axes.get(&code)
    }
    /// LT/RT 是否存在（键程当量展示用；缺轴就少显示一行，P4-3）
    pub fn has_axis(&self, code: u16) -> bool {
        self.axes.contains_key(&code)
    }
    /// 是否声明该按键（P3-1 展示集：没声明的按键只在真按下来时才出现）
    pub fn has_button(&self, code: u16) -> bool {
        self.buttons.contains(&code)
    }
}

/// 一帧输入状态。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputSnapshot {
    /// 当前按下的按键 code
    pub pressed: BTreeSet<u16>,
    /// 轴 code → 原始值
    pub axes: BTreeMap<u16, i32>,
    /// 收到的事件数（诊断用）
    pub events: u64,
}

/// 读取线程推给主循环的消息。
#[derive(Debug)]
pub enum InputMsg {
    /// 设备能力 + 初始状态（打开成功后的第一条消息）
    Ready {
        caps: DeviceCaps,
        snapshot: InputSnapshot,
    },
    /// 输入更新（已按 ≤60fps 合帧）
    Frame(InputSnapshot),
    /// 设备断开或读取失败；页面据此切空态
    Disconnected { node: String, reason: String },
}

// ── code → 名称 ─────────────────────────────────────────────────────────

/// 常用 ABS 轴名（不在此表的按 `ABS_0x??` 显示）。
pub fn axis_name(code: u16) -> String {
    let name = match code {
        0x00 => "ABS_X",
        0x01 => "ABS_Y",
        0x02 => "ABS_Z",
        0x03 => "ABS_RX",
        0x04 => "ABS_RY",
        0x05 => "ABS_RZ",
        0x06 => "ABS_THROTTLE",
        0x07 => "ABS_RUDDER",
        0x08 => "ABS_WHEEL",
        0x09 => "ABS_GAS",
        0x0a => "ABS_BRAKE",
        0x10 => "ABS_HAT0X",
        0x11 => "ABS_HAT0Y",
        0x12 => "ABS_HAT1X",
        0x13 => "ABS_HAT1Y",
        0x14 => "ABS_HAT2X",
        0x15 => "ABS_HAT2Y",
        0x16 => "ABS_HAT3X",
        0x17 => "ABS_HAT3Y",
        _ => return format!("ABS_{code:#04x}"),
    };
    name.to_string()
}

/// 常用按键名：手柄游戏键区 + 方向键区；其余按 `KEY_0x??` 显示。
pub fn key_name(code: u16) -> String {
    let name = match code {
        0x130 => "BTN_SOUTH",
        0x131 => "BTN_EAST",
        0x132 => "BTN_C",
        0x133 => "BTN_NORTH",
        0x134 => "BTN_WEST",
        0x135 => "BTN_Z",
        0x136 => "BTN_TL",
        0x137 => "BTN_TR",
        0x138 => "BTN_TL2",
        0x139 => "BTN_TR2",
        0x13a => "BTN_SELECT",
        0x13b => "BTN_START",
        0x13c => "BTN_MODE",
        0x13d => "BTN_THUMBL",
        0x13e => "BTN_THUMBR",
        0x13f => "BTN_TRIGGER_HAPPY1",
        0x140 => "BTN_TRIGGER_HAPPY2",
        0x141 => "BTN_TRIGGER_HAPPY3",
        0x142 => "BTN_TRIGGER_HAPPY4",
        0x143 => "BTN_TRIGGER_HAPPY5",
        0x144 => "BTN_TRIGGER_HAPPY6",
        0x145 => "BTN_TRIGGER_HAPPY7",
        0x146 => "BTN_TRIGGER_HAPPY8",
        0x147 => "BTN_TRIGGER_HAPPY9",
        0x148 => "BTN_TRIGGER_HAPPY10",
        0x149 => "BTN_TRIGGER_HAPPY11",
        0x14a => "BTN_TRIGGER_HAPPY12",
        0x14b => "BTN_TRIGGER_HAPPY13",
        0x14c => "BTN_TRIGGER_HAPPY14",
        0x14d => "BTN_TRIGGER_HAPPY15",
        0x14e => "BTN_TRIGGER_HAPPY16",
        0x14f => "BTN_TRIGGER_HAPPY17",
        0x150 => "BTN_TRIGGER_HAPPY18",
        0x151 => "BTN_TRIGGER_HAPPY19",
        0x152 => "BTN_TRIGGER_HAPPY20",
        0x153 => "BTN_TRIGGER_HAPPY21",
        0x154 => "BTN_TRIGGER_HAPPY22",
        0x155 => "BTN_TRIGGER_HAPPY23",
        0x156 => "BTN_TRIGGER_HAPPY24",
        0x157 => "BTN_TRIGGER_HAPPY25",
        0x158 => "BTN_TRIGGER_HAPPY26",
        0x159 => "BTN_TRIGGER_HAPPY27",
        0x15a => "BTN_TRIGGER_HAPPY28",
        0x15b => "BTN_TRIGGER_HAPPY29",
        0x15c => "BTN_TRIGGER_HAPPY30",
        0x15d => "BTN_TRIGGER_HAPPY31",
        0x15e => "BTN_TRIGGER_HAPPY32",
        0x220 => "BTN_DPAD_UP",
        0x221 => "BTN_DPAD_DOWN",
        0x222 => "BTN_DPAD_LEFT",
        0x223 => "BTN_DPAD_RIGHT",
        0x100 => "BTN_TRIGGER",
        0x101 => "BTN_TOP",
        0x102 => "BTN_PINKIE",
        0x103 => "BTN_BASE",
        _ => return format!("KEY_{code:#05x}"),
    };
    name.to_string()
}

// ── 位图解析 ────────────────────────────────────────────────────────────

/// 把 `EVIOCGBIT` 结果解析成已置位的 code 列表（升序）。
fn bits(buf: &[u8]) -> Vec<u16> {
    let mut out = Vec::new();
    for (i, byte) in buf.iter().enumerate() {
        for b in 0..8 {
            if byte >> b & 1 != 0 {
                let code = (i * 8 + b) as u16;
                out.push(code);
            }
        }
    }
    out
}

// ── fd 级操作 ───────────────────────────────────────────────────────────

fn open_node(node: &str) -> io::Result<RawFd> {
    let cpath = CString::new(node).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let fd = unsafe { libc::open(cpath.as_ptr(), O_RDONLY | O_NONBLOCK | O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

fn close_fd(fd: RawFd) {
    unsafe { libc::close(fd) };
}

fn ioctl_buf(fd: RawFd, req: libc::c_ulong, buf: &mut [u8]) -> bool {
    unsafe { libc::ioctl(fd, req, buf.as_mut_ptr()) >= 0 }
}

/// 读设备名（`EVIOCGNAME`）。
fn read_name(fd: RawFd) -> String {
    let mut buf = vec![0u8; 256];
    if !ioctl_buf(fd, eviocgname(buf.len() as u64), &mut buf) {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// 读能力集：轴（含 `EVIOCGABS` 元数据）、按键、`FF_RUMBLE`。
fn read_caps(fd: RawFd, node: &str) -> DeviceCaps {
    let mut ev_buf = vec![0u8; 8];
    let ev_ok = ioctl_buf(fd, eviocgbit(0, ev_buf.len() as u64), &mut ev_buf);
    let ev_bits: BTreeSet<u16> = if ev_ok {
        bits(&ev_buf).into_iter().collect()
    } else {
        BTreeSet::new()
    };

    let mut axes = BTreeMap::new();
    if ev_bits.contains(&EV_ABS) {
        let mut abs_buf = vec![0u8; 8];
        if ioctl_buf(
            fd,
            eviocgbit(EV_ABS as u64, abs_buf.len() as u64),
            &mut abs_buf,
        ) {
            for code in bits(&abs_buf) {
                let mut info = AbsInfo::default();
                let raw = &mut info as *mut AbsInfo as *mut u8;
                let slice =
                    unsafe { std::slice::from_raw_parts_mut(raw, std::mem::size_of::<AbsInfo>()) };
                if unsafe { libc::ioctl(fd, eviocgabs(code as u64), slice.as_mut_ptr()) } >= 0 {
                    axes.insert(
                        code,
                        AxisInfo {
                            code,
                            name: axis_name(code),
                            min: info.minimum,
                            max: info.maximum,
                            fuzz: info.fuzz,
                            flat: info.flat,
                            resolution: info.resolution,
                        },
                    );
                }
            }
        }
    }

    let mut buttons = Vec::new();
    if ev_bits.contains(&EV_KEY) {
        let mut key_buf = vec![0u8; 96];
        if ioctl_buf(
            fd,
            eviocgbit(EV_KEY as u64, key_buf.len() as u64),
            &mut key_buf,
        ) {
            buttons = bits(&key_buf);
        }
    }

    let mut ff_rumble = false;
    if ev_bits.contains(&EV_FF) {
        let mut ff_buf = vec![0u8; 16];
        if ioctl_buf(
            fd,
            eviocgbit(EV_FF as u64, ff_buf.len() as u64),
            &mut ff_buf,
        ) {
            ff_rumble = bits(&ff_buf).contains(&FF_RUMBLE);
        }
    }

    DeviceCaps {
        node: node.to_string(),
        name: read_name(fd),
        axes,
        buttons,
        ff_rumble,
    }
}

/// 初始状态：`EVIOCGKEY` 取当前按下的键，`EVIOCGABS.value` 取各轴当前值。
fn read_initial(fd: RawFd, caps: &DeviceCaps) -> InputSnapshot {
    let mut snapshot = InputSnapshot::default();
    let mut key_buf = vec![0u8; 96];
    if ioctl_buf(fd, eviocgkey(key_buf.len() as u64), &mut key_buf) {
        snapshot.pressed = bits(&key_buf).into_iter().collect();
    }
    for code in caps.axes.keys() {
        let mut info = AbsInfo::default();
        let raw = &mut info as *mut AbsInfo as *mut u8;
        let slice = unsafe { std::slice::from_raw_parts_mut(raw, std::mem::size_of::<AbsInfo>()) };
        if unsafe { libc::ioctl(fd, eviocgabs(*code as u64), slice.as_mut_ptr()) } >= 0 {
            snapshot.axes.insert(*code, info.value);
        }
    }
    snapshot
}

/// 把一个事件应用到快照上；返回是否改变了状态。
fn apply_event(snapshot: &mut InputSnapshot, ev: &InputEvent) -> bool {
    snapshot.events += 1;
    match ev.r#type {
        t if t == EV_KEY => {
            if ev.value != 0 {
                snapshot.pressed.insert(ev.code);
            } else {
                snapshot.pressed.remove(&ev.code);
            }
            true
        }
        t if t == EV_ABS => {
            snapshot.axes.insert(ev.code, ev.value);
            true
        }
        t if t == EV_SYN => false, // 合帧边界，不构成状态变化（EV_MSC 等同理）
        _ => false,
    }
}

// ── 读取线程 ────────────────────────────────────────────────────────────

/// 读取句柄：`Drop` 即停止并 join（≤16ms），保证关窗后无残留线程。
pub struct InputReader {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl InputReader {
    /// 启动读取线程，返回（消息接收端, 读取句柄）。
    ///
    /// 第一条消息是 `Ready`（打开成功）或 `Disconnected`（打不开，带原因）。
    /// 丢弃 `InputReader` 即停止线程；`Drop` 会 join，不留后台线程。
    pub fn spawn(node: impl Into<String>) -> (Receiver<InputMsg>, InputReader) {
        let node = node.into();
        let (tx, rx) = channel::<InputMsg>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_in_thread = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("gamepad-input".into())
            .spawn(move || reader_loop(node, tx, stop_in_thread))
            .expect("启动手柄读取线程失败");
        (
            rx,
            InputReader {
                stop,
                handle: Some(handle),
            },
        )
    }
}

impl Drop for InputReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn reader_loop(node: String, tx: Sender<InputMsg>, stop: Arc<AtomicBool>) {
    let fd = match open_node(&node) {
        Ok(fd) => fd,
        Err(err) => {
            let _ = tx.send(InputMsg::Disconnected {
                node: node.clone(),
                reason: format!("无法打开 {node}: {err}"),
            });
            return;
        }
    };

    let caps = read_caps(fd, &node);
    let mut snapshot = read_initial(fd, &caps);
    if tx
        .send(InputMsg::Ready {
            caps: caps.clone(),
            snapshot: snapshot.clone(),
        })
        .is_err()
    {
        close_fd(fd);
        return;
    }

    let mut dirty = false;
    let mut last_frame = Instant::now();
    let mut buf = [0u8; std::mem::size_of::<InputEvent>() * 64];

    'outer: while !stop.load(Ordering::SeqCst) {
        let mut pollfd = libc::pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let rc = unsafe { libc::poll(&mut pollfd, 1, POLL_TIMEOUT_MS) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            let _ = tx.send(InputMsg::Disconnected {
                node: node.clone(),
                reason: format!("poll 失败: {err}"),
            });
            break;
        }
        if pollfd.revents & (POLLERR | POLLHUP | POLLNVAL) != 0 {
            let _ = tx.send(InputMsg::Disconnected {
                node: node.clone(),
                reason: "设备已断开".to_string(),
            });
            break;
        }
        if pollfd.revents & POLLIN != 0 {
            loop {
                let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n < 0 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EAGAIN)
                        || err.raw_os_error() == Some(libc::EWOULDBLOCK)
                    {
                        break;
                    }
                    let _ = tx.send(InputMsg::Disconnected {
                        node: node.clone(),
                        reason: format!("读取失败: {err}"),
                    });
                    break 'outer;
                }
                if n == 0 {
                    let _ = tx.send(InputMsg::Disconnected {
                        node: node.clone(),
                        reason: "设备已断开（EOF）".to_string(),
                    });
                    break 'outer;
                }
                let count = n as usize / std::mem::size_of::<InputEvent>();
                for i in 0..count {
                    let ev = unsafe { *(buf.as_ptr() as *const InputEvent).add(i) };
                    if apply_event(&mut snapshot, &ev) {
                        dirty = true;
                    }
                }
            }
        }

        // 合帧：状态有变化且距上一帧 ≥16ms 才推，空闲时不刷屏
        if dirty && last_frame.elapsed() >= Duration::from_millis(POLL_TIMEOUT_MS as u64) {
            if tx.send(InputMsg::Frame(snapshot.clone())).is_err() {
                break;
            }
            dirty = false;
            last_frame = Instant::now();
        }
    }

    close_fd(fd);
}

// ── 单元测试（无设备即可跑；虚拟手柄集成测试见 tests/）────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(min: i32, max: i32, flat: i32, fuzz: i32) -> AxisInfo {
        AxisInfo {
            code: 0x02,
            name: "ABS_Z".into(),
            min,
            max,
            fuzz,
            flat,
            resolution: 0,
        }
    }

    #[test]
    fn 键程当量换算_边界与中点() {
        let a = axis(-32768, 32767, 128, 16);
        assert!((a.percent(a.min) - 0.0).abs() < 1e-9);
        assert!((a.percent(a.max) - 100.0).abs() < 1e-9);
        // 中点附近：0 相对 -32768..32767 应落在 50% 附近（差值 < 0.01%）
        assert!((a.percent(0) - (32768.0 / 65535.0 * 100.0)).abs() < 1e-9);
        assert!((a.percent(0) - 50.0).abs() < 0.01);

        let t = axis(0, 1023, 0, 0); // 扳机
        assert!((t.percent(0) - 0.0).abs() < 1e-9);
        assert!((t.percent(1023) - 100.0).abs() < 1e-9);
        assert!((t.percent(512) - (512.0 / 1023.0 * 100.0)).abs() < 1e-9);
    }

    #[test]
    fn 键程当量换算_越界被夹住() {
        let a = axis(0, 255, 10, 1);
        assert_eq!(a.percent(-5), 0.0);
        assert_eq!(a.percent(9999), 100.0);
    }

    #[test]
    fn 键程当量换算_零跨度不panic() {
        let a = axis(0, 0, 0, 0);
        assert_eq!(a.percent(123), 0.0);
        assert_eq!(a.midpoint(), 0);
    }

    #[test]
    fn 死区判定用flat与fuzz中的大者() {
        let a = axis(-32768, 32767, 128, 16);
        assert!(a.in_deadband(0), "中点应在死区内");
        assert!(a.in_deadband(100), "flat=128 时 100 仍在死区");
        assert!(!a.in_deadband(500), "500 超出死区");
    }

    #[test]
    fn code到名称_已知与回退() {
        assert_eq!(key_name(0x130), "BTN_SOUTH");
        assert_eq!(key_name(0x13d), "BTN_THUMBL");
        assert_eq!(key_name(0x1234), "KEY_0x1234");
        assert_eq!(axis_name(0x05), "ABS_RZ");
        assert_eq!(axis_name(0x02), "ABS_Z");
        assert_eq!(axis_name(0x3f), "ABS_0x3f");
    }

    #[test]
    fn 位图解析_按字节与位序() {
        let buf = [0b0000_0001u8, 0b0000_0010];
        assert_eq!(bits(&buf), vec![0, 9]);
        assert!(bits(&[0u8; 4]).is_empty());
    }

    #[test]
    fn 快照应用_按键与轴() {
        let mut snap = InputSnapshot::default();
        let press = InputEvent {
            time_sec: 0,
            time_usec: 0,
            r#type: EV_KEY,
            code: 0x130,
            value: 1,
        };
        assert!(apply_event(&mut snap, &press));
        assert!(snap.pressed.contains(&0x130));

        let release = InputEvent { value: 0, ..press };
        assert!(apply_event(&mut snap, &release));
        assert!(!snap.pressed.contains(&0x130));

        let axis_ev = InputEvent {
            time_sec: 0,
            time_usec: 0,
            r#type: EV_ABS,
            code: 0x02,
            value: 512,
        };
        assert!(apply_event(&mut snap, &axis_ev));
        assert_eq!(snap.axes.get(&0x02), Some(&512));

        let syn = InputEvent {
            r#type: EV_SYN,
            code: 0,
            value: 0,
            ..axis_ev
        };
        assert!(!apply_event(&mut snap, &syn), "SYN 不改变状态");
        assert_eq!(snap.events, 4);
    }

    #[test]
    fn 设备能力_缺轴时优雅查询() {
        let caps = DeviceCaps {
            node: "/dev/input/event0".into(),
            name: "无轴设备".into(),
            axes: BTreeMap::new(),
            buttons: vec![0x130],
            ff_rumble: false,
        };
        assert!(!caps.has_axis(0x02), "缺轴应返回 false 而不是报错");
        assert!(caps.axis(0x02).is_none());
        assert!(!caps.ff_rumble, "无 FF 能力的设备应为 false（P5 置灰依据）");
    }

    /// P1-6：虚拟手柄注入 → 读取线程收到快照 → 拔掉 → 收到断开。
    ///
    /// 需要 `/dev/uinput` 可写与 `scripts/uinput_pad.py`，故默认 ignore；
    /// 运行：`cargo test -- --ignored`。
    #[test]
    #[ignore = "需要 /dev/uinput 写权限与 scripts/uinput_pad.py"]
    fn 虚拟手柄注入读取与断开() {
        use std::io::{BufRead, BufReader, Write};
        use std::process::{Child, Command, Stdio};
        use std::sync::mpsc::channel;

        const PAD_NAME: &str = "P1 Integration Pad";
        let mut child: Child = Command::new("python3")
            .args([
                "scripts/uinput_pad.py",
                "create",
                "--seconds",
                "30",
                "--name",
                PAD_NAME,
                "--id",
                "1111:2222",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("启动虚拟手柄脚本失败");

        let stdout = child.stdout.take().expect("取 stdout");
        let (line_tx, line_rx) = channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                eprintln!("[pad-stdout] {line}"); // 诊断：脚本每行输出都打出来
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });

        // 等脚本打印 EVENT_NODE=...
        let node = loop {
            let line = line_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("等待虚拟手柄创建超时");
            if let Some(rest) = line.strip_prefix("EVENT_NODE=") {
                break rest.trim().to_string();
            }
        };
        assert!(node.starts_with("/dev/input/event"), "节点格式不对: {node}");

        let started = std::time::Instant::now();
        let (rx, _reader) = InputReader::spawn(&node);
        eprintln!("[test] t={:?} spawn 完成", started.elapsed());
        let ready = rx.recv_timeout(std::time::Duration::from_secs(5));
        let (caps, mut snapshot) = match ready {
            Ok(InputMsg::Ready { caps, snapshot }) => (caps, snapshot),
            Ok(other) => panic!("首条消息应为 Ready，实际 {other:?}"),
            Err(err) => panic!("等待 Ready 超时: {err}"),
        };
        assert_eq!(caps.name, PAD_NAME, "设备名应与脚本一致");
        assert!(caps.ff_rumble, "默认造的手柄应声明 FF_RUMBLE");
        assert!(caps.has_axis(0x00), "应暴露 ABS_X");
        assert!(caps.buttons.contains(&0x130), "应暴露 BTN_SOUTH");

        // 注入：按下 A 键 + 把 ABS_X 拉到 12345
        let mut stdin = child.stdin.take().expect("取 stdin");
        write!(stdin, "key BTN_SOUTH 1\nabs ABS_X 12345\nsync\n").unwrap();
        stdin.flush().unwrap();
        eprintln!("[test] t={:?} 已写入注入命令", started.elapsed());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(InputMsg::Frame(snap)) => {
                    snapshot = snap;
                    if snapshot.pressed.contains(&0x130) && snapshot.axes.get(&0x00) == Some(&12345)
                    {
                        break;
                    }
                }
                Ok(other) => panic!("注入期间收到意外消息 {other:?}"),
                Err(_) if std::time::Instant::now() < deadline => continue,
                Err(err) => panic!("等注入帧超时: {err}（已见 snapshot {snapshot:?}）"),
            }
        }

        // 松开按键
        write!(stdin, "key BTN_SOUTH 0\nsync\n").unwrap();
        stdin.flush().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(InputMsg::Frame(snap)) => {
                    let released = !snap.pressed.contains(&0x130);
                    if released {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) if std::time::Instant::now() < deadline => continue,
                Err(err) => panic!("等松开帧超时: {err}"),
            }
        }

        // 拔掉虚拟手柄（杀脚本 → uinput 设备销毁）→ 应收到断开
        drop(stdin);
        child.kill().expect("结束虚拟手柄脚本失败");
        let _ = child.wait();
        let disconnected = rx.recv_timeout(std::time::Duration::from_secs(5));
        match disconnected {
            Ok(InputMsg::Disconnected { node: n, reason }) => {
                assert_eq!(n, node);
                assert!(!reason.is_empty());
            }
            Ok(other) => panic!("拔出后应收到 Disconnected，实际 {other:?}"),
            Err(err) => panic!("等断开消息超时: {err}"),
        }
    }

    /// P1-4：两台虚拟手柄并存时各读各的节点，注入到 A 的事件绝不出现在 B 上。
    #[test]
    #[ignore = "需要 /dev/uinput 与 scripts/uinput_pad.py"]
    fn 多设备并存互不串扰() {
        let (mut child_a, _stdin_a, node_a) = start_pad("Multi Pad A", "1111:0001");
        let (mut child_b, _stdin_b, node_b) = start_pad("Multi Pad B", "1111:0002");
        assert_ne!(node_a, node_b, "两台虚拟手柄必须是不同节点");

        let (rx_a, reader_a) = InputReader::spawn(&node_a);
        let (rx_b, reader_b) = InputReader::spawn(&node_b);
        for (tag, rx) in [("A", &rx_a), ("B", &rx_b)] {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(InputMsg::Ready { caps, .. }) => {
                    assert_eq!(caps.name, format!("Multi Pad {tag}"));
                    assert_eq!(
                        caps.node,
                        if tag == "A" {
                            node_a.clone()
                        } else {
                            node_b.clone()
                        }
                    );
                }
                other => panic!("设备 {tag} 未就绪: {other:?}"),
            }
        }

        // 只往 A 注入：BTN_SOUTH 按下 + ABS_X=12345
        let out = std::process::Command::new("python3")
            .args([
                "scripts/uinput_pad.py",
                "inject",
                "--node",
                &node_a,
                "--event",
                "key:BTN_SOUTH:1",
                "--event",
                "abs:ABS_X:12345",
            ])
            .output()
            .expect("执行注入命令失败");
        assert!(
            out.status.success(),
            "inject 失败: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // A 必须读到
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut seen = false;
        while std::time::Instant::now() < deadline && !seen {
            match rx_a.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(InputMsg::Frame(snap)) => {
                    seen = snap.pressed.contains(&0x130) && snap.axes.get(&0x00) == Some(&12345);
                }
                Ok(InputMsg::Disconnected { node, reason }) => {
                    panic!("A 意外断开: {node} {reason}")
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(seen, "设备 A 没读到自己身上的注入事件");

        // B 在 400ms 内不得出现这些值
        let stop = std::time::Instant::now() + std::time::Duration::from_millis(400);
        while std::time::Instant::now() < stop {
            match rx_b.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(InputMsg::Frame(snap)) => {
                    assert!(!snap.pressed.contains(&0x130), "B 读到了只注入给 A 的按键");
                    assert_ne!(
                        snap.axes.get(&0x00),
                        Some(&12345),
                        "B 读到了只注入给 A 的轴值"
                    );
                }
                Ok(InputMsg::Disconnected { node, reason }) => {
                    panic!("B 意外断开: {node} {reason}")
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }

        drop(reader_a);
        drop(reader_b);
        child_a.kill().ok();
        child_b.kill().ok();
        let _ = child_a.wait();
        let _ = child_b.wait();
    }

    /// 启动一台虚拟手柄并等到它打印节点：返回（子进程、stdin 句柄、event 节点）。
    fn start_pad(name: &str, id: &str) -> (std::process::Child, std::process::ChildStdin, String) {
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;
        let mut child = std::process::Command::new("python3")
            .args([
                "scripts/uinput_pad.py",
                "create",
                "--seconds",
                "60",
                "--name",
                name,
                "--id",
                id,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动虚拟手柄脚本失败");
        let stdin = child.stdin.take().expect("取 stdin");
        let stdout = child.stdout.take().expect("取 stdout");
        let (tx, rx) = channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let node = loop {
            let line = rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("等待虚拟手柄创建超时");
            if let Some(rest) = line.strip_prefix("EVENT_NODE=") {
                break rest.trim().to_string();
            }
        };
        (child, stdin, node)
    }
}
