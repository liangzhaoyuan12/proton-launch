//! 振动（`FF_RUMBLE`）写入层（GOAL P5，逻辑层，无 GTK / adw）。
//!
//! 职责边界：页面只发意图（哪个马达、多强），真正的设备写入由 `app.rs` 持有
//! [`FfWriter`] 完成；本模块只负责「把效果上传并播放/停掉」。
//!
//! 离线验收链路（F16 / P5-1）：`scripts/uinput_pad.py create` 造的虚拟手柄会
//! 应答内核投来的 `UI_FF_UPLOAD` 并把收到的参数回打成 `FF_UPLOAD` / `FF_PLAY`
//! 行；测试用它断言「写入的 strong/weak 与读回一致」，无需真手柄。
//!
//! 结构体尺寸与 ioctl 号都以本机 `<linux/input.h>` / `<linux/uinput.h>` 为准
//! （`sizeof(ff_effect)==48`、`EVIOCSFF==0x40304580`，见单测），Linux 会把
//! size 编码进 ioctl 号，对不上直接 EINVAL（踩过一次：`uinput_ff_upload`
//! 实际是 104 字节，头文件里有 `effect + old` 两个 `ff_effect`）。

use std::fs::File;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

/// `FF_RUMBLE`（`linux/input.h`）。
pub const FF_RUMBLE: u16 = 0x50;
/// `EV_FF`（`linux/input-event-codes.h`）。
const EV_FF: u16 = 0x15;
/// `_IOW('E', 0x80, struct ff_effect)`；`sizeof(ff_effect) == 48`（本机实测）。
const EVIOCSFF: libc::c_ulong = 0x4030_4580;
/// 单次振动时长（ms）——A15/D8：一次脉冲 1s，硬上限 2s 由 `app.rs` 兜底。
pub const PULSE_MS: u16 = 1000;

// ── 与内核逐字节对齐的效果结构（linux/input.h）───────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfReplay {
    length: u16,
    delay: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfTrigger {
    button: u16,
    interval: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfEnvelope {
    attack_length: u16,
    attack_level: u16,
    fade_length: u16,
    fade_level: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfConstantEffect {
    level: i16,
    envelope: FfEnvelope,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfRampEffect {
    start_level: i16,
    end_level: i16,
    envelope: FfEnvelope,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfConditionEffect {
    right_saturation: u16,
    left_saturation: u16,
    right_coeff: i16,
    left_coeff: i16,
    deadband: u16,
    center: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfPeriodicEffect {
    waveform: u16,
    period: u16,
    magnitude: i16,
    offset: i16,
    phase: u16,
    envelope: FfEnvelope,
    custom_len: u32,
    custom_data: *mut i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfRumbleEffect {
    strong_magnitude: u16,
    weak_magnitude: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct FfHapticEffect {
    hid_usage: u16,
    vendor_id: u16,
    vendor_waveform_page: u8,
    intensity: u16,
    repeat_count: u16,
    retrigger_period: u16,
}

/// `union ff_effect_u`：以 `ff_periodic_effect`（含指针，32B/对齐8）撑满，
/// 保证 `ff_effect` 总尺寸 48、对齐 8，与内核一致。
#[repr(C)]
#[derive(Clone, Copy)]
union FfEffectU {
    rumble: FfRumbleEffect,
    periodic: FfPeriodicEffect,
    constant: FfConstantEffect,
    ramp: FfRampEffect,
    condition: [FfConditionEffect; 2],
    haptic: FfHapticEffect,
}

/// `struct ff_effect`（linux/input.h，48 字节）。
#[repr(C)]
#[derive(Clone, Copy)]
struct FfEffect {
    effect_type: u16,
    id: i16,
    direction: u16,
    trigger: FfTrigger,
    replay: FfReplay,
    u: FfEffectU,
}

impl FfEffect {
    /// 一个 `FF_RUMBLE` 效果；`id = -1` 表示「新建」，内核回填分配的 id。
    fn rumble(strong_magnitude: u16, weak_magnitude: u16, length_ms: u16) -> Self {
        Self {
            effect_type: FF_RUMBLE,
            id: -1,
            direction: 0,
            trigger: FfTrigger {
                button: 0,
                interval: 0,
            },
            replay: FfReplay {
                length: length_ms,
                delay: 0,
            },
            u: FfEffectU {
                rumble: FfRumbleEffect {
                    strong_magnitude,
                    weak_magnitude,
                },
            },
        }
    }
}

// ── 马达与强度 ────────────────────────────────────────────────────────────

/// 马达侧别：左 = `strong_magnitude`（重马达），右 = `weak_magnitude`（轻马达）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motor {
    Left,
    Right,
}

impl Motor {
    /// 中文名（UI/日志用）。
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "左马达",
            Self::Right => "右马达",
        }
    }
}

/// 强度百分比（0..=100，越界夹住）→ `(strong, weak)` 幅值（0..=0xFFFF）。
///
/// 只给被选中的一侧幅值，另一侧为 0 —— 这就是「左/右马达独立测试」。
pub fn magnitudes(motor: Motor, pct: u8) -> (u16, u16) {
    let pct = pct.min(100) as u32;
    let mag = (pct * u16::MAX as u32 / 100) as u16;
    match motor {
        Motor::Left => (mag, 0),
        Motor::Right => (0, mag),
    }
}

// ── 写入端 ────────────────────────────────────────────────────────────────

/// 一个可写的设备节点句柄：上传效果 + 播放/停止。
///
/// 打开需要 `O_RDWR`（uaccess ACL 给手柄节点的是 `rw-`），与读取线程的
/// `O_RDONLY` 句柄互不干扰。
pub struct FfWriter {
    file: File,
    node: String,
    /// 内核分配的效果 id（-1 = 还没上传过）
    effect_id: i16,
    /// 是否已经上传成功过（stop 在没上传时是空操作）
    uploaded: bool,
}

impl FfWriter {
    /// 打开设备节点（`/dev/input/eventN`）。
    pub fn open(node: &str) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(node)?;
        Ok(Self {
            file,
            node: node.to_string(),
            effect_id: -1,
            uploaded: false,
        })
    }

    /// 当前绑定的设备节点。
    pub fn node(&self) -> &str {
        &self.node
    }

    /// 上传（或更新）一个 rumble 效果，返回内核分配的效果 id。
    ///
    /// 首次调用以 `id = -1` 新建；之后复用同一 id 更新参数（避免占满
    /// `ff_effects_max` 个槽位）。虚拟手柄场景下这一步会阻塞到
    /// `uinput_pad.py` 应答 `UI_FF_UPLOAD`（实测 ≤ 0.1s，脚本轮询周期）。
    pub fn upload(&mut self, strong_magnitude: u16, weak_magnitude: u16) -> io::Result<i32> {
        let mut effect = FfEffect::rumble(strong_magnitude, weak_magnitude, PULSE_MS);
        effect.id = self.effect_id;
        let rc = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                EVIOCSFF,
                &mut effect as *mut FfEffect,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        self.effect_id = effect.id;
        self.uploaded = true;
        Ok(i32::from(self.effect_id))
    }

    /// 启动当前效果（`EV_FF value=1`）。
    pub fn play(&self) -> io::Result<()> {
        if !self.uploaded {
            return Ok(());
        }
        self.send(1)
    }

    /// 停止当前效果（`EV_FF value=0`）；未上传过则空操作（幂等，可反复调用）。
    pub fn stop(&self) -> io::Result<()> {
        if !self.uploaded {
            return Ok(());
        }
        self.send(0)
    }

    /// 写一条 `input_event`（24 字节：time 16B + type/code/value）。
    fn send(&self, value: i32) -> io::Result<()> {
        use std::io::Write;
        let mut ev = [0u8; 24];
        ev[16..18].copy_from_slice(&EV_FF.to_ne_bytes());
        ev[18..20].copy_from_slice(&(self.effect_id as u16).to_ne_bytes());
        ev[20..24].copy_from_slice(&value.to_ne_bytes());
        let mut file = &self.file;
        file.write_all(&ev)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 结构体尺寸与ioctl号与内核一致() {
        // Linux 把 size 编码进 ioctl 号；对不上会 EINVAL（实测踩过 104≠56）
        assert_eq!(std::mem::size_of::<FfEffect>(), 48);
        assert_eq!(std::mem::offset_of!(FfEffect, id), 2);
        assert_eq!(std::mem::offset_of!(FfEffect, u), 16);
        assert_eq!(std::mem::size_of::<FfEffectU>(), 32);
        assert_eq!(EVIOCSFF, 0x4030_4580);
        // 本机 <linux/uinput.h> 实测值（gcc /tmp/sizes.c 打印）
        assert_eq!(FF_RUMBLE, 0x50);
    }

    #[test]
    fn 强度换算_默认30_边界与越界() {
        // A15/D8 默认 30% → 30 * 65535 / 100 = 19660 = 0x4CCC
        assert_eq!(magnitudes(Motor::Left, 30), (0x4ccc, 0));
        assert_eq!(magnitudes(Motor::Right, 30), (0, 0x4ccc));
        assert_eq!(magnitudes(Motor::Left, 100), (0xffff, 0));
        assert_eq!(magnitudes(Motor::Left, 0), (0, 0));
        // 越界的强度被夹到 100%
        assert_eq!(magnitudes(Motor::Right, 255), (0, 0xffff));
    }

    #[test]
    fn 马达名_中文化() {
        assert_eq!(Motor::Left.name(), "左马达");
        assert_eq!(Motor::Right.name(), "右马达");
    }

    /// P5-1 离线回环断言：写入的 `strong/weak` 与虚拟手柄读回的一致。
    ///
    /// 链路：造虚拟手柄 → `FfWriter::upload/play/stop` → 脚本应答
    /// `UI_FF_UPLOAD` 并回打 `FF_UPLOAD` / `FF_PLAY` 行 → 断言行内容。
    /// 真机马达手感（能否感到左/右差异）需真机，列 §8 可延后。
    #[test]
    #[ignore = "需要 /dev/uinput 与虚拟手柄，走 cargo test -- --ignored"]
    fn 振动上传回环_写入与读回一致() {
        use std::io::Read;
        use std::process::{Command, Stdio};

        let mut child = Command::new("python3")
            .args([
                "scripts/uinput_pad.py",
                "create",
                "--seconds",
                "30",
                "--name",
                "FF 循环测试 Pad",
                "--id",
                "045e:02dd",
            ])
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .expect("启动虚拟手柄脚本失败");

        let mut out = child.stdout.take().expect("取 stdout");
        // 手动按字节读到 EVENT_NODE= 行，**不经过 BufReader**：
        // 缓冲会把后面的 FF_UPLOAD/FF_PLAY 行预读走，drop 时就丢了。
        let mut acc: Vec<u8> = Vec::new();
        let node;
        loop {
            let mut buf = [0u8; 1024];
            let n = out.read(&mut buf).expect("读脚本输出");
            assert!(n > 0, "脚本提前退出");
            acc.extend_from_slice(&buf[..n]);
            let marker = b"EVENT_NODE="; // 11 字节（踩过：windows(12) 永远匹配不上）
            if let Some(pos) = acc.windows(marker.len()).position(|w| w == marker) {
                let tail = &acc[pos + marker.len()..];
                let end = tail
                    .iter()
                    .position(|b| *b == b'\n')
                    .expect("EVENT_NODE 行没读完");
                let path = String::from_utf8_lossy(&tail[..end]).trim().to_string();
                assert!(path.starts_with("/dev/input/"), "节点异常: {path}");
                node = path;
                break;
            }
        }

        // ACL/内核注册可能略慢，带重试地打开
        let mut writer = None;
        for _ in 0..40 {
            if let Ok(w) = FfWriter::open(&node) {
                writer = Some(w);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let mut writer = writer.expect("打开设备节点失败");

        // 左马达 30%（0x4CCC）、右马达不动；随后切到右马达 100%
        let id = writer.upload(0x4ccc, 0).expect("上传失败");
        assert!(id >= 0, "效果 id 异常: {id}");
        writer.play().expect("启动失败");
        std::thread::sleep(std::time::Duration::from_millis(300));
        writer.stop().expect("停止失败");

        // 读脚本回打（非阻塞，最多等 3s）
        let fd = out.as_raw_fd();
        unsafe {
            let fl = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, fl | libc::O_NONBLOCK);
        }
        let mut rest = Vec::new();
        for _ in 0..30 {
            let mut buf = [0u8; 4096];
            match out.read(&mut buf) {
                Ok(0) => std::thread::sleep(std::time::Duration::from_millis(100)),
                Ok(n) => rest.extend_from_slice(&buf[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(100))
                }
                Err(e) => panic!("读脚本输出失败: {e}"),
            }
            let text = String::from_utf8_lossy(&rest);
            if text.contains("FF_PLAY") && text.contains("value=0") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&rest).to_string();

        let _ = child.kill();
        let _ = child.wait();

        // 断言：上传参数一致 + 播放/停止各来一次
        assert!(
            text.contains("strong=0x4ccc") && text.contains("weak=0x0000"),
            "回读的幅值与写入不一致：\n{text}"
        );
        assert!(
            text.contains(&format!("FF_PLAY id={id} value=1")),
            "缺少启动事件：\n{text}"
        );
        assert!(
            text.contains(&format!("FF_PLAY id={id} value=0")),
            "缺少停止事件：\n{text}"
        );
    }
}
