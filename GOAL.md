# GOAL — 「手柄状态」页规划（输入实时可视化 + 振动测试）

- 文档版本：v0.1（规划稿）
- 创建日期：2026-10-07
- 当前阶段：**仅规划，未写任何功能代码**
- 项目：Proton 启动管理器（GTK4 + libadwaita，Rust / cargo，仓库根目录即本文件所在处）

---

## 0. 反幻觉规则（全程适用）

1. 打勾（`[x]`）必须附**真实执行过的命令及其输出片段**（复制粘贴，不许改写）；没有证据只能标 `[ ]` 或 🟡。
2. 没跑过的命令一律标 🟡「待执行」，不许写成「已验证」。
3. 不确定的内核 / 驱动 / 协议行为写「待调研」，**禁止**把记忆或推测写成结论。
4. 任何「已实现 / 已通过 / 可用」的断言，必须能在本文件找到对应的验证命令与结果。
5. 事实与建议分栏：事实附证据命令；建议放「待定决策表」，未获你确认前不算已定。

---

## 1. 需求（你给的原始条目）

> 增加一页功能：读取手柄状态

| # | 需求 | 说明 |
|---|------|------|
| R1 | 实时显示手柄所有输入状态 | 手柄上所有按键 / 轴的当前值实时可见 |
| R2 | 按键 / 摇杆 / 扳机**动画可视化** | 不是纯文本列表，要有动态图形反馈 |
| R3 | 左右马达振动测试 | 主体双马达（低频/高频）分别可控测试 |
| R4 | 左右扳机振动测试 | **你 2026-10-07 纠正：是「xone 协议的扳机振动」**，即 Xbox One 系手柄的 Impulse Triggers（脉冲扳机马达），不是 DualSense 自适应扳机 |
| R5 | 键程当量显示（摇杆、LT/RT 等） | 把原始轴值换算成行程量显示（理解见 §4.4，理解有误请纠正） |

约束（沿用上一功能的既有要求）：

- 多手柄：机器可同时连多个手柄，本页要能处理（切换 / 各自独立读取，见待定决策 D2）。
- 软件不驻留后台：关闭即彻底退出，本页相关线程必须在退出时一并停掉（沿用 `App::shutdown` 的停止逻辑）。
- 分层约定（本仓库既有）：**逻辑层不依赖 gtk/adw/glib**（见 `src/utils/gamepad.rs` 头注释），渲染在 `src/widgets/`，接线在 `src/app.rs`。

---

## 2. 已定决策表

| ID | 决策 | 来源 | 日期 |
|----|------|------|------|
| A1 | 新增**独立页面**承载该功能（不是塞进右下角卡片） | 你的指令「增加一页功能」 | 2026-10-06 |
| A2 | 实时显示手柄全部输入状态 | 你（R1） | 2026-10-06 |
| A3 | 按键/摇杆/扳机做动画可视化 | 你（R2） | 2026-10-06 |
| A4 | 左右马达振动测试 | 你（R3） | 2026-10-06 |
| A5 | 扳机振动 = **Xbox One 协议（xone）扳机振动 / Impulse Triggers**，非 DualSense 自适应扳机 | 你纠正（原稿「自适应扳机」作废） | 2026-10-07 |
| A6 | 键程当量显示（摇杆、LT/RT 等） | 你（R5） | 2026-10-06 |
| A7 | 本阶段**只产出规划文档，不写代码** | 你 | 2026-10-06 |
| A8 | 页面入口：**侧栏视窗最底部**固定一行「手柄状态」。该行**不进游戏列表**，放在独立固定区（列表滚动、增删重建、窗口缩放都不影响它，恒贴视窗底部）；其**上方为标准分隔线**（全宽 `gtk::Separator`，不再用带 12px 边距的 `ListBoxRow` 包裹） | **你修订**（2026-10-07 验收前指令，覆盖原「列表最底部 + 列表内分隔符」口径） | 2026-10-07 |
| A9 | 多手柄：页面内下拉切换、一次只读一只（D2） | 你拍板（D2–D8 一并按建议） | 2026-10-07 |
| A10 | 输入源用 `/dev/input/eventN`（evdev），不用 `jsN`（D3） | 你拍板 | 2026-10-07 |
| A11 | 可视化用 `gtk::DrawingArea` 自绘 + `add_tick_callback`（D4） | 你拍板 | 2026-10-07 |
| A12 | 扳机振动按降级顺序推进：先试现有驱动 → 需装驱动/补丁则先征得你同意 → 都不行则置灰并说明原因；**范围限定 Xbox One 协议扳机振动**（D5 + 你的 xone 口径澄清） | 你拍板 | 2026-10-07 |
| A13 | 键程当量口径：0–100% 百分比 + 刻度条 + `raw/min/max`，摇杆标中点与 `flat` 死区（D6） | 你拍板 | 2026-10-07 |
| A14 | 页面命名「手柄状态」（D7） | 你拍板 | 2026-10-07 |
| A15 | 振动默认 30% 强度、单次 1s、硬上限 2s 自动停，切页/关窗/断开即停（D8） | 你拍板 | 2026-10-07 |
| A16 | **测试策略以虚拟手柄为主**：你不便连真手柄，允许我自建 uinput 虚拟手柄完成离线验收；虚拟手段验不了的（马达真实手感）标注「需真机、可延后」 | 你指令「能不能自己创建虚拟手柄进行测试…写进文档」 | 2026-10-07 |

> 「xone」一词**已由你确认 = Xbox One 协议**（2026-10-07），不是第三方 `xone` 驱动（本机也没有该驱动，见 F6）。

---

## 3. 现状事实（✅ = 已真实跑过命令；🟡 = 待执行）

| ID | 事实 | 证据命令 | 输出摘要 |
|----|------|----------|----------|
| F1 ✅ | 当前连着一只**真实** Xbox 无线手柄，走蓝牙（uhid），驱动 `hid_microsoft`，事件节点 `event23`、`js0` | `awk '/Xbox Wireless/{f=1} f{print} f&&/^$/{exit}' /proc/bus/input/devices` | `N: Name="Xbox Wireless Controller"` / `S: Sysfs=...uhid/0005:045E:02E0...` / `H: Handlers=kbd event23 js0` |
| F2 ✅ | 该手柄轴能力 `ABS=3003f` → 6 个轴：左右摇杆 X/Y + RX/RY，以及 `ABS_Z`/`ABS_RZ`（即 LT/RT）；按键位图非空 | 同上 | `B: ABS=3003f`、`B: KEY=3ff000000000000 0 800 0 0` |
| F3 ✅ | 我**可读**该手柄的 evdev 节点（logind 给了 ACL），无权限阻塞 | `test -r /dev/input/event23 && echo OK` + `getfacl /dev/input/event23` | `OK`、`user:liangzhaoyuan12:rw-` |
| F4 ✅ | 该设备声明了 `EV_FF`（`EV=20001b` 含 bit21=0x15=EV_FF）且 FF 位图非空 | `/proc/bus/input/devices` | `B: EV=20001b`、`B: FF=107030000 0` |
| F5 ✅ | FF 位图已离线解码：含 **`FF_RUMBLE`(0x50)**、`FF_PERIODIC`(0x51)、`FF_SQUARE`(0x58)、`FF_TRIANGLE`(0x59)、`FF_SINE`(0x5a)、`FF_GAIN`(0x60) → 该手柄**声明支持 rumble**（实写振动是否真动马达仍需 P0-2 实测） | 解码依据：`grep -n "#define FF_RUMBLE\|#define FF_PERIODIC\|#define FF_GAIN" /usr/include/linux/input.h` → `493:#define FF_RUMBLE 0x50`、`494:#define FF_PERIODIC 0x51`、`523:#define FF_GAIN 0x60`；位图 `FF=107030000 0` 按 64 位组高位在前解出 bit 80/81/88/89/90/96 |
| F6 ✅ | 本机无第三方 `xone` 驱动；`xpad` 模块存在但未加载（无有线 Xbox 设备）；`hid_microsoft`、`joydev`、`uinput` 已加载 | `modinfo xone` / `modinfo xpad` / `lsmod` | `modinfo: ERROR: Module xone not found.`、`xpad.ko.xz` 存在、`hid_microsoft 16384 0` |
| F7 ✅ | **上游 xpad 驱动不驱动扳机马达**：`XTYPE_XBOXONE` 的 GIP 振动包里 left/right trigger 两字节写死 0x00，只把 `strong/weak` 发给左右主体马达；且只接受 `FF_RUMBLE` | `grep -n "left trigger\|right trigger\|effect->type" /tmp/xpad.c` → 见 `sed -n '1540,1625p' /tmp/xpad.c` | `data[6] = 0x00; /* left trigger */`、`data[7] = 0x00; /* right trigger */`、`if (effect->type != FF_RUMBLE) return 0;` |
| F8 ✅ | evdev 头文件在位，含 `EVIOCGABS`、`FF_RUMBLE` 定义（键程当量所需） | `grep -c "EVIOCGABS\|FF_RUMBLE" /usr/include/linux/input.h` | `4` |
| F9 ✅ | 页面接入点：内容区是 `gtk::Stack`（页名 `"config"`），侧栏在 `navigation.rs`，split view 在 `window.rs` | `grep -rn "add_named\|set_content" src/window.rs` | `window.rs:41: stack.add_named(&config_bin, Some("config"))`、`window.rs:48: split_view.set_content(...)` |
| F10 ✅ | 现有手柄基础设施可复用：`src/utils/gamepad.rs`（27914 字节，解析 `/proc/bus/input/devices` + 热插拔线程）、`src/widgets/gamepad_indicator.rs`（右下角计数与卡片） | `ls -l src/utils/ src/widgets/` | 见上表时间戳 |
| F11 ✅ | 可视化依赖的动画 API 存在性已确认，详见 F14 | 见 F14 | — |
| F12 ✅ | 本机有可用的 GUI 截图取证链路（X11：`xdotool` + `import`），上一功能已用它取证 | 上一功能截图 `~/.hermes/cache/scratch/shot-*.png` 已成功生成 | — |
| F13 ✅ | `/proc/bus/input/devices` 位图打印格式已校准（64 位一组、高位在前、省略前导零组）：`KEY=3ff000000000000 ...` 中的 `0x3ff` 恰好落在 bit308–317 = `BTN_WEST`…`BTN_THUMBL`，与 Xbox 手柄按键集合吻合 —— F5 的解码依赖此格式 | `awk '/Xbox Wireless/{f=1} f&&/^$/{exit} f' /proc/bus/input/devices` 与 `/usr/include/linux/input-event-codes.h` 的 `BTN_WEST 0x134`…`BTN_THUMBL 0x13d` 对照 | `B: KEY=3ff000000000000 0 800 0 0` |
| F14 ✅ | 动画 / 自绘 API 在本机 registry 里真实存在：`adw::TimedAnimation`、`add_tick_callback`、`gtk::DrawingArea` | `grep -rln "TimedAnimation" ~/.cargo/registry/src/*/libadwaita-0.9*/src` 等三条 | `libadwaita-0.9.2/src/auto/timed_animation.rs`、`gtk4-0.11.5/src/widget.rs`、`gtk4-0.11.5/src/auto/drawing_area.rs` |
| F15 ✅ | **虚拟手柄链路已跑通**：`/dev/uinput` 对本用户可写，自写脚本 `~/.hermes/cache/scratch/uinput_pad.py` 用 raw ioctl 创建过 3 个虚拟手柄，被本软件 `gamepad.rs` 扫描识别，连接/断开通知、计数、卡片都端到端验证过 | 上一功能真实日志：`[通知] 手柄已连接 — Microsoft X-Box 360 pad · Xbox 360` / `[通知] 手柄已断开 — …`（共 4 条）；`/proc/bus/input/devices` 计数由 23 → 26 → 23 | 见上；脚本参数 `<存活秒数> "名称:VID:PID"` |
| F18 ✅ | **第二条扳机路径也封死**：`hid-microsoft`（Xbox 蓝牙路径）的 FF 实现只发 7 字节的 `xb1s_ff_report {report_id=3, enable, magnitude[2], duration, delay, loop}` —— 只有左/右**主体**马达两个强度字段，**没有扳机马达字段**；且 `ms_play_effect` 只接受 `FF_RUMBLE`（`if (effect->type != FF_RUMBLE) return 0`），强度按 `strong→左执行器 / weak→右执行器` 缩放到 0..100 | `grep -nE "xb1s_ff_report|MS_QUIRK_FF|effect->type" /tmp/hid-microsoft.c` → `44:struct xb1s_ff_report`、`290: r->report_id = XB1S_FF_REPORT`、`296-297: magnitude[MAGNITUDE_STRONG]/[MAGNITUDE_WEAK] = 左/右执行器`、`313: if (effect->type != FF_RUMBLE) return 0`；`grep -n "left trigger\|right trigger" /tmp/xpad.c` → 见 F7 | — |
| F17 ✅ | **虚拟手柄要拿 evdev 访问权，必须让 udev 判定为摇杆**：只有 `ID_INPUT=1` 不够，必须同时声明**完整的 gamepad 按键集（BTN_SOUTH..BTN_THUMBR）+ 摇杆轴**，udev 才打 `ID_INPUT_JOYSTICK=1`，logind 才给 `uaccess` ACL；否则节点 `crw-rw---- root input`、本用户 `EACCES` 打不开（这会让 P1/P3/P4/P5 的注入测试全部失败） | 对照实验：只设 1 个按键的 C 探针设备 → `udevadm info -q property -n <node> \| grep ID_INPUT` 只有 `ID_INPUT=1`、`getfacl` 无 user 项、`open` 报 Permission denied；按键集补全后 → `ID_INPUT=1` + `ID_INPUT_JOYSTICK=1` + `user:liangzhaoyuan12:rw-` | 见左列；已固化进 `scripts/uinput_pad.py` 与 `scripts/ff_loopback_probe.c` 的按键集 |
| F16 ✅ | 虚拟手柄 **FF 回环成立**：`EVIOCSFF` 上传成功（`ret=0 effect.id=0`），`write {EV_FF,id,1}` 与停止命令都能从 uinput fd 读回（`EV_FF code=0 value=1` / `value=0`）→ **振动命令可离线自动断言**；但事件**不出现在同一 evdev 节点**（`playback_evdev=0`），断言点必须放在持有 uinput fd 的一侧 | `gcc -O2 -pthread -o /tmp/ff_probe scripts/ff_loopback_probe.c && timeout 30 /tmp/ff_probe` → `FF_LOOPBACK=OK`，`exit=0`，1.9s | — |

---

## 4. 技术方案概要（结论待 P0 校准，此处先定骨架）

### 4.1 数据层（新建 `src/utils/gamepad_input.rs`，**不依赖 gtk**）

- 打开选中手柄的 `/dev/input/eventN`，`O_NONBLOCK` + `poll` 循环读 `input_event`。
- 线程内维护「按键位图 + 各轴原始值」的最新快照；`mpsc` 送到主线程，主循环按 ~16ms（60fps 上限）合并取最新一帧渲染，避免每个事件都触发 UI。
- 轴元数据用 `EVIOCGABS(code)` 取 `absmin / absmax / fuzz / flat`，键程当量换算就用它（不硬编码范围）。
- 设备拔出（读到 `EIO` / 节点消失）→ 关闭 fd、上报「断开」，页面切到空态。
- 沿用 `gamepad.rs` 的手柄发现与协议识别，不重复造轮子。

### 4.2 页面层（新建 `src/page/gamepad.rs` 或 `src/widgets/` 下对应文件，**只做渲染**）

- 页面骨架：设备选择器（下拉，来自已连接手柄列表）+ 输入可视化区 + 振动测试区 + 断开空态。
- 可视化区实现：`gtk::DrawingArea` 自绘（摇杆圆点、扳机柱、按键高亮格），动画用 `add_tick_callback` 或 `adw::TimedAnimation`（API 存在性已核实，见 F11/F14 ✅；A11）。
- 页面关闭 / 应用退出时停读取线程（挂到现有 `App::shutdown` 一并处理）。

### 4.3 振动测试（依赖 P0 结论，见 §6 阻塞项）

- 左右马达：`FF_RUMBLE` 效果，`strong_magnitude`（左/低频）与 `weak_magnitude`（右/高频）分别滑条可控；用 `EVIOCSFF` 提交效果 + 写 `EV_FF` 事件启动 / 停止。
- 扳机振动：**方案待 P0**。已知事实 F7 表明上游 `xpad` 把扳机字节写死为 0；当前手柄是蓝牙走 `hid_microsoft`，其 FF 行为未知（F5）。候选路径（都是待验证的假设，不是结论）：
  - P-a：`hid_microsoft` 是否支持 `EV_FF`/`FF_RUMBLE`，以及是否带动扳机马达；
  - P-b：有线接 `xpad` 时能否通过内核补丁 / 其它驱动开放扳机通道；
  - P-c：~~用户态驱动（`xone` 之类）~~ —— **已排除**：你确认 xone = Xbox One 协议（A12），第三方 `xone` 驱动不在范围内；若将来要装，会按 A12 先征得你同意。
- 安全阀（无论走哪条路都要有）：振动默认低强度、**单次最长 2s 自动停**、切走页面 / 关闭应用 / 设备断开立即停、无 `EV_FF` 能力时整块控件置灰并说明原因。

### 4.4 键程当量（口径已拍板，A13）

把轴的原始值换算成「行程量」显示：

```
percent = (raw - absmin) / (absmax - absmin) × 100%      // 摇杆：0%~100%，中点约 50%
LT/RT：同样按 EVIOCGABS 范围换算，另显示 raw / min / max 三行原始值
可选：再标一档 0~255 或 0~65535 的等效档位
```

- 死区 / 平坦区（`flat`）在摇杆上要标出（居中指示线），否则 50% 附近的抖动会让读数跳动。
- 口径已拍板（A13/D6）：0–100% 百分比 + 刻度条 + `raw/min/max`，摇杆标中点与 `flat` 死区。

### 4.5 测试策略：**虚拟手柄为主，真机只做最后的手感确认**（你明确不方便连真手柄）

你不在场、真机不接的情况下，绝大部分验收都能自动完成。可用与不可用的边界如下（**这条边界就是反幻觉红线**）：

| 能用虚拟手柄验证（✅/待 P0 确认） | 不能用虚拟手柄验证（必须真机） |
|---|---|
| 手柄插入/拔出识别、页面实时显示、按键/摇杆/扳机动画（注入 `EV_KEY`/`EV_ABS` 事件 → 截图比对） | **马达是否真的动、手感强弱**（uinput 设备背后没有物理马达） |
| 键程当量换算（自定义 `absmin/absmax/flat`，注入固定 raw → 断言百分比） | 扳机马达是否被驱动（R4 的最终结论） |
| 多手柄并存与切换（脚本参数即：`<存活秒数> "名称:VID:PID"`，可造 N 只） | |
| 无 FF 能力设备的置灰降级（造一只不声明 `EV_FF` 的手柄） | |
| 振动按钮**确实发出了效果命令**（若 P0-8 证实 uinput 能回环读回 `EV_FF`，即可自动断言，无需真机） | |

落地要求：

1. 把 `uinput_pad.py` 收进仓库（建议 `scripts/uinput_pad.py`），并在 P0 阶段补齐两件事：a) 支持声明 `EV_FF`/`FF_RUMBLE` 能力；b) 支持设置自定义 `ABS` 范围（`min/max/flat`）——现有脚本只造基础按键+轴（见 F15）。
2. 所有需要「注入输入」的验收项，验证命令统一写成：**脚本注入 → 期望读数/截图**，不写「人工按手柄」。
3. 只有 §8 第 3、4 条的**手感**部分标注「需真机在场，可延后到你方便时」，其余条目全部可离线完成。

---

## 5. 阶段计划（每项都带验证命令；命令一律真实可执行）

### P0 — 调研与可行性（不写功能代码，只出结论）

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P0-1 | 解析 FF 能力位图，确认当前手柄支持哪些 FF 类型（F5） | `grep -n "#define FF_RUMBLE\|#define FF_GAIN" /usr/include/linux/input.h` | [x] ✅ 输出：`493:#define FF_RUMBLE 0x50`、`523:#define FF_GAIN 0x60`；解码结论见 F5（`FF_RUMBLE` 已确认） |
| P0-2 | 实测写一次 `FF_RUMBLE`：**先用虚拟手柄做回环**（命令是否发出、返回值），**真机手感留给你方便时**（R3 最终确认） | `timeout 30 /tmp/ff_probe`（scripts/ff_loopback_probe.c） | [x] ✅ 虚拟侧完成：`EVIOCSFF ret=0 errno=0`、上传 `magnitude=0x8000`、启停命令均回读成功；**真机手感列 §8 可延后项** |
| P0-3 | 扳机三条路径（P-a/P-b/P-c）调研，给出**可达性结论**（R4 前置） | `grep -n "left trigger\|right trigger" /tmp/xpad.c`（F7）、`grep -nE "xb1s_ff_report\|effect->type" /tmp/hid-microsoft.c`（F18）、`modinfo xone`（F6） | [x] ✅ **结论：默认不可达**。P-a（当前蓝牙/hid_microsoft）：FF report 只有主体马达两档强度，无扳机字段（F18）；P-b（有线/xpad）：GIP 包里扳机字节写死 0x00（F7）；P-c（第三方 xone 驱动）：本机未装且按 A12 已排除。→ 按 **A12 c 分支**：扳机按钮置灰 + 显示原因；若你要走 b 分支（内核补丁/换驱动），我先列步骤征得你同意。**真机复核列 §8 可延后项**（源码证据充分，但未在真机上摸过扳机马达） |
| P0-4 | 键程当量换算所需的 `EVIOCGABS` 字段确认（min/max/fuzz/flat 可读） | `python3 scripts/evdev_probe.py /dev/input/eventN` | [x] ✅ 输出（虚拟手柄）：`ABS_X min=-32768 max=32767 fuzz=16 flat=128`、`ABS_Z min=0 max=1023 fuzz=0 flat=0`、自定义轴 `ABS_Z min=0 max=255 fuzz=1 flat=10`、`ABS_X min=0 max=1000 fuzz=0 flat=5` → min/max/fuzz/flat 全部可读，键程当量换算输入齐备 |
| P0-5 | 动画 / 自绘 API 存在性（F11/F14） | `grep -rln "TimedAnimation" ~/.cargo/registry/src/*/libadwaita-0.9*/src` 等三条 | [x] ✅ 输出：`libadwaita-0.9.2/src/auto/timed_animation.rs`、`gtk4-0.11.5/src/widget.rs`（`add_tick_callback`）、`gtk4-0.11.5/src/auto/drawing_area.rs` |
| P0-6 | 阻塞时通知与「不驻留后台」约束复核：退出时必须杀读取线程 | `grep -n "fn shutdown\|gamepad_source\|gamepad_monitor" src/app.rs` | [x] ✅ 输出：`970: pub fn shutdown()`、`976: if let Some(monitor) = app.gamepad_monitor.take()`、`979: if let Some(source) = app.gamepad_source.take()` → 现有 shutdown 已停手柄线程与轮询定时器，新读取线程挂进同一处即可 |
| P0-7 | 汇总 P0 结论到本文件「阻塞项登记」，未解决的转 D5 决策 | 本文件 §6 被更新且每行有证据 | [x] ✅ B1/B2 由「开放」改「已定论」、B4 关闭，见 §6 |
| P0-8 | **虚拟手柄 FF 回环可行性**（决定振动测试能否离线自动验收，F16） | `gcc -O2 -pthread -o /tmp/ff_probe scripts/ff_loopback_probe.c && timeout 30 /tmp/ff_probe` | [x] ✅ `FF_LOOPBACK=OK`、`exit=0`、1.9s；详见 F16（含 `UI_FF_UPLOAD`/`UI_FF_ERASE` 必须应答、清理顺序两个坑） |
| P0-9 | 把虚拟手柄脚本收进仓库 `scripts/uinput_pad.py`，补齐声明 `EV_FF`/`FF_RUMBLE`、自定义 `ABS` 范围（min/max/fuzz/flat），另加 stdin 注入（key/abs/sync/quit）与 `inject` 子命令 | `python3 scripts/uinput_pad.py --help`；`create --no-ff --abs ABS_Z:0:255:1:10` 后 `evdev_probe.py` 回读 | [x] ✅ 输出：`--help` 正常；两只虚拟手柄 `/proc` 计数 23→26；`EV_FF=0`（`--no-ff` 生效）、`ABS_Z min=0 max=255 fuzz=1 flat=10`、`ABS_X min=0 max=1000 fuzz=0 flat=5`（自定义范围生效）、FF 开启时 `EV_FF=1`/`FF_RUMBLE=1` |

### P1 — 输入数据层（逻辑层，无 UI）

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P1-1 | 新建 `src/utils/gamepad_input.rs`：打开 event 节点、非阻塞读、快照结构 | `cargo build 2>&1 \| tail -3` → `Finished dev profile` | [x] |
| P1-2 | 轴归一化 + 键程当量换算（用 `EVIOCGABS` 范围，不硬编码） | `cargo test 2>&1 \| tail -3` → 含换算边界用例（0%/50%/100%、flat 区） | [x] |
| P1-3 | 事件 → 通道 → 主循环 16ms 合帧；断开检测（`EIO`/节点消失） | 单测快照合并逻辑；`cargo test` 通过 | [x] |
| P1-4 | 多手柄：按 `gamepad.rs` 的设备表选择目标节点，互不串扰 | `cargo test` 覆盖两个设备节点并存的用例 | [x] |
| P1-5 | 质量门禁 | `cargo fmt --check && cargo clippy --all-targets 2>&1 \| grep -cE "^(warning\|error)"` → 期望 `0` | [x] ✅ 当时 45 个 dead_code 如预期，P3–P5 接线后归零；P6 复测 `grep -c warning` = `0` |
| P1-6 | 虚拟手柄集成验证：`scripts/uinput_pad.py` 造设备 → 注入已知按键/轴 → 快照读到对应值；拔掉脚本进程 → 触发断开 | 两步命令各打印期望值（注入前后 diff） | [x] |

### P2 — 页面接入

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P2-1 | 内容区 `gtk::Stack` 增加页（页名 `"gamepad"`）；侧栏入口按 A8：分隔符 +「手柄状态」固定在列表最底部（`refresh_list` 的 `remove_all` 之后必须重建到末尾）。**2026-10-07 修订**：入口已移出游戏列表、改为视窗底部独立固定区，`append_fixed_entry` 已删除（详见 A8） | `grep -rn "add_named" src/window.rs src/app.rs` → 实测 `src/app.rs:224: add_named(&page.root, Some("gamepad"))`（页在 app.rs boot 挂载，非 window.rs）；`grep -n "手柄状态\|Separator" src/navigation.rs` → 行49 `.title("手柄状态")`、行61 `gtk::Separator`、行111 `append_fixed_entry`、行166 恒可见说明；截图确认该行在最底部 | [x] |
| P2-2 | 页面骨架：设备下拉 + 三个分区（输入可视化 / 键程当量 / 振动测试）+ 空态 | `xdotool search --name "Proton 启动管理器"` → `67108868`；截图 `/tmp/p2-tall.png`（窗口拉高后）：大标题「手柄状态」+ 四分组「设备/输入状态/键程当量/振动测试」+ 占位文字全部可见，设备下拉显示 `P2 Scroll Pad (/dev/in…`，状态栏「已连接 1 个手柄」 | [x] |
| P2-3 | 无手柄时显示空态（沿用右下角卡片的文案风格） | 手柄拔空后截图 `/tmp/t6b.png`：空态「未连接手柄」+ 描述「连接手柄后，这里会显示实时输入、键程当量与振动测试。」，侧边栏该行高亮 | [x] |
| P2-4 | 页面切换不打断游戏配置页（Stack 只换显示，不重建配置页） | 往返切换实测：点「原神」→配置页、点「手柄状态」→状态页（截图 `/tmp/p2-groups2.png` 等）；`grep -n "add_named\|visible_child" src/window.rs src/app.rs` → 只有 `set_visible_child_name("welcome"/"gamepad"/"config")`，无重建调用 | [x] |

### P3 — 实时输入显示 + 动画可视化

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P3-1 | 按键区：按 `EV_KEY` 位图逐个高亮，名称从 evdev 语义映射表取（未映射显示原始码） | 按 A16 改虚拟注入：`python3 scripts/uinput_pad.py inject --node /dev/input/event24 --event key:BTN_SOUTH:1` → 最终构建截图 `/tmp/tab.png` 中 A 格实心蓝；像素统计蓝色系 5429px、A 格簇 `x[364..414] y[424..580] n=2925`；映射表单测随 `cargo test`（26 passed） | [x] ✅ 真机按压按 A16 记为「仅手感延后」，高亮链路由虚拟注入取证 |
| P3-2 | 摇杆区：左右各一圆形摇杆动画（圆点随轴值移动，带中心/死区标记） | `inject --event abs:ABS_X:32767 --event abs:ABS_Y:-32768` → `/tmp/tab.png` 读图：两盘画出中心十字，左摇杆蓝点偏离中心；盘内蓝色细簇 `x[435..454]/x[487..505]/x[543..567]/x[607..616]` | [x] ✅ |
| P3-3 | 扳机区：LT/RT 柱状动画随 `ABS_Z/ABS_RZ` 起落 | `inject --event abs:ABS_Z:64 --event abs:ABS_RZ:191` → `/tmp/tab.png`：LT/RT 蓝色填充柱 + 读数 LT 6%、RT 19%，与量程换算一致（`scripts/uinput_pad.py:81` 默认 `ABS_Z 0..1023`：64/1023=6.3%、191/1023=18.7%） | [x] ✅ |
| P3-4 | 动画流畅度：空闲 CPU 与帧率不劣化（合帧渲染，≤60fps） | `top -b -n11 -d1 -p $(pgrep -x proton-launch)` （页已打开 + 手柄已连 + 读取线程在跑）→ 10 采样 `0.0 0.0 0.0 2.0 0.0 0.0 1.0 0.0 1.0 0.0`（均值 0.4%）。中途曾测 40–55%，根因不在本页：按键记录器捕获 4s 内 **279 个 Tab**（ydotoold 虚拟设备卡键），`pkill -x ydotoold` 后同进程 CPU 立刻 `0.0`；基线 `git worktree add /tmp/pl-base HEAD` 构建同法测 0–1%。空闲无新帧时 tick `Break` 自摘除（`src/page/gamepad.rs` start_tick，仅内容态才 `queue_draw`），帧由 16ms 合帧驱动 → ≤60fps | [x] ✅ 不劣化（基线 0–1% vs 当前均值 0.4%） |
| P3-5 | 读取线程不阻塞 UI（页面秒切、配置页可用） | `grep -n "poll\|O_NONBLOCK\|channel" src/utils/gamepad_input.rs` → 行287 `O_RDONLY\|O_NONBLOCK\|O_CLOEXEC`、行496/501 `pollfd + poll(16ms)`、行19/440 `mpsc channel`（读在独立线程，主线程只收快照）；实测注入持续期间 Tab 切页与截图无卡顿 | [x] ✅ |
| P3-6 | **虚拟手柄注入驱动画面**：脚本注入指定按键/轴 → 页面对应高亮/形变（无需真手柄） | `create --name "P3 验证 Pad" --id 045e:02dd` → `EVENT_NODE=/dev/input/event24` → 注入 A + ABS_Z + ABS_RZ + ABS_X/ABS_Y → **无任何临时代码的最终构建**截图 `/tmp/tab.png`（键盘 Tab 打开该页）：A 蓝、摇杆点、LT/RT 柱与 6%/19% 读数全部与注入值一致 | [x] ✅ |

### P4 — 键程当量显示（R5）

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P4-1 | 摇杆 / LT / RT 每轴显示：百分比 + 刻度条 + `raw/min/max` | AT-SPI 精确读数（`a11y_sub.py` 遍历控件树）6 行齐全：`左摇杆 X 81% · 20000 / -32768..32767`、`左摇杆 Y 50% · … · 居中`、`右摇杆 X 35%`、`右摇杆 Y 50% · 居中`、`LT 6% · 64 / 0..1023`、`RT 19% · 191 / 0..1023`；截图 `/tmp/p4-final.png`；像素测量（条 `x464..706`）蓝色填充 30.5% / 0% / 15.2% / 0% / 6.2% / 18.9% 与换算值 31/0/15/0/6.3/18.7 吻合；换算单测随 `cargo test` 28 passed | [x] ✅ |
| P4-2 | 中点居中、死区/平坦区标识（避免 50% 附近抖动跳数） | 单测 `键程当量显示_平坦区吸附与不同量程换算`（flat 区两端 raw 读数完全相同 = 吸附到 50%）；实机两轮注入（左 Y `50→100`、右 Y `100→-60`，都在 flat=128 内）AT-SPI 读数恒为 `50% · 居中`，raw 小字随注入变化 → 读数不抖；中点竖线像素 `x585=(102,102,112)`（仅摇杆行有，扳机行无） | [x] ✅ |
| P4-3 | 未连接 / 轴数不足的设备优雅降级（缺轴就少显示一行，不报错） | 拔插对照：`kill` 掉虚拟手柄进程 → 设备计数 0，AT-SPI 读到空态 `未连接手柄` + `连接手柄后，这里会显示实时输入、键程当量与振动测试。`，键程当量分组连行一起消失（截图 `/tmp/p4-unplug.png`）；重插 → 6 行回来且静止读数正确（摇杆 `50% · 居中`、扳机 `0%`）。缺轴样本单测 `键程当量行_按能力生成且缺轴降级`（无轴→空表、缺右摇杆→4 行、只有扳机→2 行） | [x] ✅ |
| P4-4 | **虚拟手柄不同轴范围注入**：造 `0..255`、`-32768..32767` 等不同 `absmin/absmax` 的虚拟轴，注入固定 raw → 页面百分比与断言一致 | 离线断言（可进 CI 式回归）：单测对 `0..255` 注入 64 → 25.1%、`0..1023` 注入 64 → 6.3%、`-32768..32767` 注入 0 → 50.0% 逐一断言；实机同屏两种量程注入后 AT-SPI 读数与换算一致（摇杆 20000→81%、-10000→35%；扳机 64→6%、191→19%），脚本量程见 `scripts/uinput_pad.py:76` `DEFAULT_AXES` | [x] ✅ |

### P5 — 振动测试（R3 / R4）

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P5-1 | 左/右马达独立强度滑条 + 按下振动 / 松开停 / 定时停 | 虚拟侧：`cargo test 振动上传回环 -- --ignored` → `1 passed`（`FfWriter::upload` 写入 `strong=0x4ccc/weak=0x1234` == 虚拟手柄 `UI_FF_UPLOAD` 读回值，脚本 `FF_PLAY value=1→0` 配对）；另有 `scripts/uinput_pad.py` 独立冒烟（`SMOKE=OK`，修正 `UI_BEGIN_FF_UPLOAD` 结构体 104B 后上传/播放/停止不挂死）。实录 `/tmp/p5app.log`：`[振动] 开始 · 左马达 · 强度30% · strong=0x4ccc weak=0x0000 · 脉冲1000ms / 硬停2000ms` → 按住松开 → `[振动] 停止 · 松开按钮`；纯按住轮次 → `[振动] 停止 · 1s 脉冲到时`；手柄侧 `/tmp/p5pad2.log` 回显 `FF_UPLOAD id=0 type=80 strong=0x4ccc weak=0x0000 length=1000` + `FF_PLAY id=0 value=1` / `value=0`。左右独立：右马达轮次 `strong=0x0000 weak=0x4ccc` 且脚本回显同值。注：本环境 XTEST 长按约 1s 会自动释放（工具侧，与脉冲停同窗竞速），故定时停以纯按住轮次的 `1s 脉冲到时` 取证；**真机手感（左右马达可辨）→ 需真机，可延后（B5）** | [x] ✅ |
| P5-2 | 左/右扳机振动按钮（按 P0-3 结论实现；若结论为不可达 → 按 D5 降级并置灰说明） | 降级分支（B1/B2/F18 → A12 c）：扳机两键恒 `sensitive=False`，组内原因文案（AT-SPI 读到）`扳机振动不可达：xpad 把 GIP 包的扳机字节写死 0x00、hid-microsoft 只发主体马达（GOAL F7/F18），按 A12(c) 置灰。`；无 FF 虚拟手柄轮次：`python3 scripts/uinput_pad.py create --name "无FF测试 Pad" --id 045e:02e6 --no-ff` → 热插拔 +0.6s 公告 `[通知] 手柄已连接 — 无FF测试 Pad · Xbox 系手柄（型号未知）`，AT-SPI 读到能力原因 `当前手柄未声明 FF_RUMBLE（EV_FF）能力，无法振动`，整组不可聚焦（Tab 到不了振动按钮）、按空格日志零输出，截图 `/tmp/p5-noff.png`。**真机手感扳机马达 → 需真机，可延后（B5）** | [x] ✅ |
| P5-3 | 安全阀：默认低强度、最长 2s 自动停、离开页面即停、断开即停、无能力置灰 | 逐条实录：① 默认 30% —— 滑条 AT-SPI 值 `30`，日志 `strong=0x4ccc`（=30%×65535/100）；② 自动停 ≤2s —— `[振动] 停止 · 1s 脉冲到时`（按住期间实发），2s 硬上限定时器在 `src/app.rs:1123`（1s 总先触发，2s 为兜底）；③ 离开页面即停 —— 按住中 `shift+Tab`×7 到「鸣潮」+ `Enter` → `[振动] 停止 · 离开手柄状态页`（停止点收敛在 `show_content_page`/`show_welcome`，`src/app.rs:916/1157`）；④ 断开即停 —— 按住中 `kill` 手柄进程（t=0.05s）→ `[振动] 停止 · 设备断开或切换`（`src/app.rs:1069`；设备已消失先打 `停止失败：没有那个设备 (os error 19)` 属预期）；⑤ 无能力置灰 —— 见 P5-2 无 FF 轮次。`grep -n "duration\|timeout\|stop" src/page/gamepad.rs` → `164/177/178/207/213`（松开停 handler）+ `302`（页面过渡时长，无关）；定时与停止实现 `grep -n "timeout_add_local_once\|stop_rumble(" src/app.rs` → `916/1069/1119/1123/1130/1136/1141/1157` | [x] ✅ |
| P5-4 | 振动期间读取线程不掉线（写 FF 不影响读事件） | 按住振动期间注入 `abs:ABS_X:-15000`（此前 20000）与 `key:BTN_SOUTH:1/0`：AT-SPI 键程当量行在**振动进行中**由 `81% · 20000 / -32768..32767` 变为 `27% · -15000 / -32768..32767`，期间无断开日志、停止仅出现在注入之后（读线程/主线程均未被 FF 写入阻塞）。注：鼠标按住手势路径本会话因 X 指针冻结无法触达，键盘按住与鼠标按住走同一 `rumble_press/rumble_release` handlers；鼠标路径待指针恢复后补 `xdotool mousedown` 复核 | [x] ✅ |

### P6 — 打磨与验收

| # | 任务 | 验证命令 | 状态 |
|---|------|----------|------|
| P6-1 | 多手柄：下拉切换后读取目标正确切换（按 D2 结论） | 四只手柄同在（`手柄A 测试` event24、`手柄B 测试` event26 + 桌面 Steam 两只 X-Box 360 虚拟手柄 event25/27，右下角 `已连接 4 个手柄`），注入区分值 A=`abs:ABS_X:20000`(81%)+`abs:ABS_Z:64`(6%)、B=`abs:ABS_X:0`(50%)+`abs:ABS_Z:512`(50%)。下拉切换（弹层内 Return 展开、Up/Down 移动、Return 确认；**关闭态按方向键不换选**）后 AT-SPI 实读：选中 B → `sel=手柄B 测试（/dev/input/event26）`、`左摇杆 X 50% … LT 50%`；选中 A → `sel=手柄A 测试（/dev/input/event24）`、`左摇杆 X 81% … LT 6%` —— 读数跟随所选设备、不串读。按键隔离：B 全程按住 A 键（`key:BTN_SOUTH:1` 不抬起），选中 A 截图 `/tmp/p6-iso.png` vs 选中 B 截图 `/tmp/p6-follow.png`，PIL 差分 **3424 px**（bbox 312,150–1240,686）——高亮只在选中「真正按下它的那台」时出现。注：本会话 X 指针冻结 + Wayland，切换/截图走 AT-SPI+键盘；`import -window $WID` 截不到弹层，弹层截图用 `spectacle -b -n -f` 全屏后 PIL 裁剪 | [x] ✅ |
| P6-2 | 全量质量门禁 | `cargo fmt --check` → `FMT=OK`；`cargo clippy --all-targets 2>&1 \| grep -c warning` → `0`；`cargo test` → `31 passed; 0 failed; 4 ignored`；`cargo build --release` → `Finished \`release\` profile [optimized]`（产物 `target/release/proton-launch`，1 181 424 B，08:16 构建） | [x] ✅ |
| P6-3 | 退出即彻底退出：读取线程随窗口关闭结束 | 本会话为 Wayland：`xdotool key alt+F4` 无效；`xdotool windowclose` 只会打掉 surface（日志 `GdkSurface … unexpectedly destroyed`、进程残留 pid 254579）**不能作为关窗取证**。改为 X11 直送 `WM_DELETE_WINDOW` ClientMessage（`~/.hermes/cache/scratch/p6_close.py`，`XSendEvent: 1`）走正常 close-request 路径，干净实例（`GDK_BACKEND=x11` 重启，热插拔监听线程运行中）实测：`window after: (无窗口)`、`pid after: (pgrep 无输出)`，`/tmp/p6app.log` 无任何异常告警。进程整体退出即证明监听线程等已随 `shutdown()` 收停（非 daemon 线程存活会阻止退出）；读取线程的停止另有 P3-5/P5-3④ 取证 | [x] ✅ |
| P6-4 | 文档同步：README 特性、`docs/UI规范书.md` 增补本页、`docs/项目结构规划书.md` 增补新模块 | `grep -rn "手柄状态" README*.md docs/*.md` → 9 处命中（`README.zh-CN.md:26/28`、`docs/手柄虚拟测试指南.md:1/3`、`docs/项目结构规划书.md:56/122`、`docs/UI规范书.md:106/108/112`）。改动：README.zh-CN 新增「手柄状态页」特性条、README.md 新增 `Controller Status Page` 条；UI规范书 +侧栏菜单「手柄状态」行 + 新 §3.5 手柄状态页规范（入口/分组/安全阀/置灰/无障碍/离线测试）；项目结构规划书 目录树补 `page/gamepad.rs`、`widgets/gamepad_viz.rs`、`widgets/gamepad_indicator.rs`、`utils/gamepad.rs / gamepad_input.rs / gamepad_ff.rs / notify.rs`、`scripts/`、`shots/`，新增 §3.9 模块职责表；**新建 `docs/手柄虚拟测试指南.md`**（你点名要的虚拟手柄测试进文档：前置/创建/注入/FF 回环/读数对照/空状态/清理，每条命令均为本会话真实跑过） | [x] ✅ |
| P6-5 | 取证：关键界面截图存档 | 仓库内 `shots/`：`shots/shot-gamepad-overview.png`（整页四组+设备信息+读数，视觉复核确认 X81%/LT6%）、`shots/shot-gamepad-devices.png`（下拉展开 4 设备、高亮当前项，视觉复核确认）、`shots/shot-gamepad-vibration.png`（振动测试组+焦点「按住振动 · 左马达」）、`shots/shot-gamepad-empty.png`（拔出空状态，P4 取证复用）、`shots/shot-gamepad-noff.png`（无 FF 能力整组置灰+原因文案，P5 取证复用）；运行时差分对照 `/tmp/p6-iso.png`、`/tmp/p6-follow.png` | [x] ✅ |

---

## 6. 阻塞项登记

| ID | 问题 | 影响 | 需要什么 | 状态 |
|----|------|------|----------|------|
| B1 | 扳机振动在蓝牙 / hid_microsoft 路径下**不可达**（F18：FF report 无扳机字段） | R4 / P5-2 | 已有源码证据；真机复核列 §8 可延后 | ✅ 已定论（2026-10-07，处置=A12 c 分支置灰说明；要走 b 分支需你同意） |
| B2 | 上游 `xpad` 把 GIP 振动包的扳机字节写死 0x00（F7），有线路径同样不通 | R4 | 已有源码证据；处置同 B1 | ✅ 已定论（2026-10-07，处置=A12 c 分支） |
| B3 | ~~「xone」是否指第三方 `xone` 驱动~~ | R4 范围 | 你已确认 **xone = Xbox One 协议** | ✅ 已解除（2026-10-07） |
| B4 | ~~FF 位图语义未解析~~ | R3 | 已由 F5 解码 + `evdev_probe.py` 实读确认 | ✅ 已关闭（2026-10-07） |
| B5 | 真机手柄不在场（你已明确不方便连真手柄，F1 的蓝牙手柄也可能移走） | **仅影响「手感」最终确认**：马达是否真动、扳机是否真动 | 功能/交互/读数/命令发出 全部用虚拟手柄离线验收（§4.5）；手感两条列在 §8 标注「需真机，可延后」 | 🟡 已降级（2026-10-07） |
| B6 | `2026-10-07 08:28` `/dev/uinput` 节点被重建为 `crw------- root root`（此前可写），此后 `uinput_pad.py create` 报 `PermissionError: /dev/uinput` | 只影响**今后复跑**虚拟手柄测试（本会话全部虚拟手柄取证在故障前完成） | 重新登录触发 uaccess ACL 恢复，或配 udev `uaccess` 规则 / input 组权限 | 🟡 登记（2026-10-07 08:34） |

---

## 7. 拍板记录（2026-10-07 你已全部拍板；结论同步进 §2 的 A8–A15，此节保留过程与理由）

| ID | 决策点 | 拍板结果 | 理由 / 备注 |
|----|--------|------|------|
| D1 | 页面入口形式 | **你修正后定稿**：入口行位于**侧栏列表最底部**，命名「手柄状态」，其**上方有一个分隔符**，且该行**恒为列表最底部**（列表重建后仍回到最底部）；HeaderBar 不加按钮 | 落地要点：`navigation.rs::refresh_list` 用 `remove_all()` 重建时会连带删掉固定行 → 必须在重建后重新「分隔符 → 手柄状态行」追加到末尾，或改为只移除游戏行；游戏列表为空时入口仍在。**2026-10-07 已被 A8 修订取代**：改为视窗底部独立固定区，不再追加进列表 |
| D2 | 多手柄读取方式 | 页面内**下拉选择一个手柄**读取；设备下拉列出全部已连接手柄（含协议名与 event 节点） | 同时渲染多手柄会让动画区翻倍复杂，且用户实际调试时一次只盯一只；列表本身仍显示全部已连接 |
| D3 | 输入源 | `/dev/input/eventN`（evdev），不用 `jsN` | evdev 能查轴元数据（`EVIOCGABS`，键程当量必需）且能写 FF（振动必需）；joydev 只有增量事件，两件事都做不了 |
| D4 | 可视化实现 | `gtk::DrawingArea` 自绘 + `add_tick_callback` 动画 | 摇杆圆点/扳机柱/按键格需要连续形变，自绘比堆 widget 省事且性能可控；若 P0-5 发现更合适的 API 再调整 |
| D5 | 扳机振动不可达时的降级 | 优先顺序：a) 走当前驱动可达的最强方案；b) 需要装额外驱动/内核补丁 → 列出安装步骤**先征求你同意**再做；c) 两条都不行 → 按钮置灰并显示「当前驱动不支持扳机振动（原因：…）」。**范围已澄清：xone = Xbox One 协议，不涉及第三方 xone 驱动** | 不擅自改内核/装驱动，符合反幻觉与安全约束；范围澄清由你 2026-10-07 给出 |
| D6 | 键程当量显示口径 | 百分比（0~100%）+ 刻度条 + `raw/min/max` 原始值三行；摇杆中点标线、标出 `flat` 死区 | 覆盖「当量」的可读性，同时保留原始数据便于排查；若你要别的口径（如 0~255 档位为主）请纠正 |
| D7 | 页面命名 | 「手柄状态」 | 与需求原话一致 |
| D8 | 振动默认强度与时长 | 默认 30% 强度、单次 1s、上限 2s 自动停 | 手柄在腿上/桌上时高强度长时间振动会吓到人，安全第一 |

---

## 8. 整体验收清单（全部满足才算完）

1. 打开本页，连着的手柄**无需操作**即可看到按键/摇杆/扳机的实时状态（R1、R2）。
2. 按键按下、摇杆推动、扳机扣动，画面有动画反馈且不卡顿（R2、P3-4）。
3. 左/右马达能分别振动测试，强度可控、超时自动停（R3、P5-3）。**离线验收到「命令已发出且参数正确」（虚拟回环）；「马达真的动」需真机，可延后到你方便时。**
4. 左/右扳机能分别振动测试；若技术上不可达，页面明确说明原因而非静默失效（R4、D5）。**同上：离线验收到命令/置灰说明为止，手感需真机，可延后。**
5. 摇杆与 LT/RT 显示键程当量（百分比 + 原始值），松手读数稳定（R5、P4-2）。
6. 多手柄可切换，互不串扰（D2、P6-1）。
7. 质量门禁全绿：`cargo fmt --check && cargo clippy --all-targets && cargo test && cargo build --release`。
8. 关窗即退出，无残留线程与后台进程（沿用「不留后台」约束）。
9. **虚拟手柄全链路离线验收**（不接真手柄也能跑）：`python3 scripts/uinput_pad.py …` 注入 → 页面读数/动画/计数/断开 全部符合预期，关键步骤留截图（§4.5、P1-6/P3-6/P4-4）。

---

## 9. 进度行（append-only，只追加不改写）

- `2026-10-07 02:09` | 创建本规划文档 v0.1：固化 R1–R5 需求、记录扳机振动口径纠正（A5）、完成 P0 事实侦察（F1–F12，含真机手柄 event23 可读、`xpad` 扳机字节恒 0 的源码证据 F7）、划分 P0–P6 阶段与验证命令、登记 B1–B5 阻塞项、定稿 D1–D8 待定决策。**未写任何功能代码。**
- `2026-10-07 02:15` | P0 部分前置调研回填：F5（FF 位图解码出 `FF_RUMBLE`）、F13（位图打印格式校准）、F14（动画 API 存在性）、P0-1、P0-5、P0-6 转 ✅；剩余 P0-2/P0-3/P0-4/P0-7 仍为 🟡（需探针与真机手感）。仍未写任何功能代码。
- `2026-10-07 02:23` | **D1–D8 全部拍板**：D1 由你修正为「侧栏列表最底部固定『手柄状态』行 + 上方分隔符 + 恒居底」；D5 澄清 xone = Xbox One 协议（B3 解除）；D2/D3/D4/D6/D7/D8 按建议通过。结论同步为 A8–A15。仍未写任何功能代码。
- `2026-10-07 02:29` | 按你要求把**虚拟手柄测试策略**写进文档
- `2026-10-07 02:52` | **P0 调研阶段完成（P0-1…P0-9 全部 ✅）**：新增 `scripts/uinput_pad.py`（含 stdin 注入/自定义 ABS/`--no-ff`）、`scripts/evdev_probe.py`（EVIOCGABS/位图探针）、`scripts/ff_loopback_probe.c`（FF 回环）；结论回填 F16（FF 回环成立）、F17（虚拟手柄需完整 gamepad 按键集才拿得到 uaccess ACL）、F18（hid-microsoft FF report 无扳机字段）；P0-3 定论：**扳机振动默认不可达**，按 A12 c 分支置灰说明（B1/B2 已定论、B4 已关闭）。A7 由「只写文档」改为你本次「自动执行任务」授权。下一步 P1。：新增 A16（测试以虚拟手柄为主）、§4.5 可测/不可测边界表、F15（uinput 虚拟手柄链路已跑通）与 F16（FF 回环待验证）、P0-8/P0-9、P1-6/P3-6/P4-4（注入式离线验收）、B5 降级（仅剩手感需真机）、§8 新增第 9 条虚拟手柄全链路验收。仍未写功能代码。
- `2026-10-07 04:00` | **P1 + P2 完成**（P1-5 留 🟡）。P1-1/2/3/4/6 ✅：`cargo test` 20 passed、`cargo test -- --ignored` 3 passed（显示测试 + P1-6 注入→读帧→断开、新增 P1-4「多设备并存互不串扰」1.03s）、`cargo fmt --check` ✅；clippy=45，全部是 gamepad_input 尚未接线的 dead_code（P3 接线后归零，P6 复测门禁）。P2-1/2/3/4 ✅，证据见表格（页实际挂在 `app.rs:224`）。执行中修掉两个真 bug：`uinput_pad.py` 的 stdin 用 `sys.stdin.readline()` 被 TextIO 缓冲吞掉后续命令（改 `os.read(0,…)` 自行按行切分）、`cmd_inject` 把 int fd 传给要 `.write()` 的 `send()`（改 `os.fdopen`）。测试链路备忘：Wayland 下注入点击前必须 `xdotool windowactivate --sync` 取得焦点（否则事件被终端吃掉），ydotool 点击码 `0xC0`（`0x00` 是空操作）、ydotool 绝对定位有约 -110px 的 Y 偏移需「停靠→逼近→悬停截图校准」，后台起虚拟手柄必须保活 stdin（否则 EOF 立即自毁）。截图证据：`/tmp/p2-sidebar.png`（底部固定行+分隔符）、`/tmp/t6b.png`（空态）、`/tmp/p2-tall.png`（四分组+设备下拉）。
- `2026-10-07 06:41` | **P3 完成（P3-1…P3-6 全 ✅，证据见表格）**。注入链路：`uinput_pad.py create`（`P3 验证 Pad` → `/dev/input/event24`）+ `inject`（BTN_SOUTH、ABS_Z=64、ABS_RZ=191、ABS_X/Y）→ 最终构建（无任何临时代码）截图 `/tmp/tab.png`：键盘 Tab 打开「手柄状态」页，A 格实心蓝、摇杆蓝点偏离中心、LT/RT 柱 6%/19%（与 `ABS_Z 0..1023` 量程换算 6.3%/18.7% 吻合），像素统计蓝色系 5429px（`magick txt` + 蓝色判定）。**P3-4 澄清**：曾测得空闲 40–55%，根因不是本页——按键记录器 4s 捕获 279 个 Tab（ydotoold 虚拟设备卡键）触发 GTK 焦点导航风暴，`pkill -x ydotoold` 后同进程 CPU 立刻 `0.0`；当前页开+手柄连+读取线程在跑时 `top -b -n11` 采样均值 0.4%，基线（`git worktree` 出 HEAD 构建）0–1%，判定不劣化。**本轮修掉 3 个问题**：① `set_draw_func` 丢失 `draw_viz` 调用导致画布不渲染（诊断清理时误删，已恢复）；② P2 遗留真 bug——停在手柄页时 `refresh_sidebar` 恢复游戏行选中，把页面拽回配置页（已改：手柄页态恢复手柄行选中）+ `show_gamepad_page` 幂等保护（防信号处理中重入焦点路径）；③ 调试期临时诊断代码（env 开关、[NAV]/[HOT]/按键日志）全部移除。门禁：`cargo fmt --check` ✅、`cargo build` ✅、`cargo test` 26 passed + 3 ignored、clippy 2 warnings（`gauge_box`/`motor_box`/`in_deadband` = P4/P5 未接线字段）。**环境备忘**：本机 X 指针冻结（`XQueryPointer` 恒 `(1085,1040)`、button5 卡 0x10、xdotool/ydotool mousemove 均无效）→ 鼠标注入改用键盘（`xdotool key Tab` 可达）或 AT-SPI（按钮有 `click` action）；ydotoold 保持停止（它就是卡键源头）。下一步 P4（键程当量接线）。
- `2026-10-07 07:04` | **P4 完成（P4-1…P4-4 全 ✅，证据见表格）**。实现：`widgets/gamepad_viz.rs` 新增纯函数 `gauge_rows`（按设备能力生成行，缺轴少行）与 `gauge_display`（摇杆平坦区吸附到中点、扳机不吸附）+ 2 个单测；`page/gamepad.rs` 新增「键程当量」行 UI（名称 \| 刻度条 \| 百分比 \| `raw / min..max`），行集合签名（节点+轴码）变化才重建控件，数值/文字只在真变化时才写与重绘（沿用 P3-4 空闲 CPU 纪律）；扳机未收到事件时按 `min`（未扣）显示，与 P3 扳机柱口径统一。**取证方式升级：用 pyatspi 读 AT-SPI 控件树拿精确文本**（不靠看图猜），六行读数 `左摇杆 X 81% · 20000 / -32768..32767`、`左摇杆 Y 50% · 100 / … · 居中`、`右摇杆 X 35%`、`右摇杆 Y 50% · -60 / … · 居中`、`LT 6% · 64 / 0..1023`、`RT 19% · 191 / 0..1023` 与换算逐一吻合；像素测量六条填充 30.5/0/15.2/0/6.2/18.9%（预期 31/0/15/0/6.3/18.7），中点竖线 `(102,102,112)@x585` 仅摇杆行存在。P4-2 实机两轮注入（左 Y `50→100`、右 Y `100→-60`，都在 flat=128 内）读数恒 `50% · 居中`；P4-3 拔插对照（`kill` 手柄进程→空态占位→重插→6 行回位、静止读数正确）；P4-4 离线断言单测覆盖 `0..255 / 0..1023 / -32768..32767` 三种量程。注入后空闲 CPU `0.0 0.0 1.0 0.0 0.0 1.0 1.0 0.0`（≈0.4%，无回退）。门禁：`cargo fmt --check` ✅、`cargo build` ✅、`cargo test` **28 passed + 3 ignored**（ignored 复跑 3 passed）、clippy 仅剩 `motor_box` never read（P5 待接线）。**测试环境备忘**：`import -window root` 现在报 `missing an image filename`（改用 `-window <WID>` 截窗口）；指针冻结下 `xdotool click 5` 滚轮不生效（改 `xdotool windowsize $WID 1130 1500` 让整页可见）。截图证据：`/tmp/p4-final.png`（六行读数+刻度条）、`/tmp/p4-unplug.png`（拔掉后空态）。下一步 P5（马达振动；扳机振动按 F18/A12(c) 置灰说明）。
- `2026-10-07 08:07` | **P5 完成（P5-1…P5-4 全 ✅，证据见表格）**。实现：新增 `src/utils/gamepad_ff.rs`（`FfWriter` 打开/上传/播放/停止 + `Motor`/`magnitudes` 强度换算 + `size_of::<FfEffect>()==48` 断言 + 忽略态回环集成测试）；`page/gamepad.rs` 振动测试分组（左右强度滑条默认 30% 带档标、「按住振动」鼠标手势与键盘等价路径、X 自动重复用 `holding` 标志抑制成单次、焦点被移走时清态、扳机恒置灰 + 原因文案、`set_ff` 按能力开关）；`app.rs` 纪元式定时停（`rumble_epoch` 让过期定时器失效，1s 脉冲 + 2s 硬停双保险；停止点收敛四处：松开 / 定时 / 离开页 / 断开），`Ready` 按 `caps.ff_rumble` 放开或置灰；`uinput_pad.py` 修 FF 应答（`struct uinput_ff_upload` 实为 **104 字节**（两个 `ff_effect`），按 56 拼包导致 `UI_BEGIN_FF_UPLOAD` EINVAL → 脚本崩 → `EVIOCSFF` 连锁 ENODEV）。门禁全绿：`cargo fmt --check` ✅、`cargo build` ✅、`cargo clippy --all-targets` **0 警告**、`cargo test` **31 passed + 4 ignored**，回环测试 `cargo test 振动上传回环 -- --ignored` → 1 passed。**测试环境备忘**：① `xdotool keydown` 长按约 1s 会被本环境自动释放（与 1s 脉冲停同窗竞速，定时停以纯按住轮次 `停止 · 1s 脉冲到时` 取证）；② `import -window` 截图轮次以 `松开按钮` 结束（疑截图动作引发键释放），「按住中页面仍响应」证据改用注入前后 AT-SPI 文本对照；③ 后台虚拟手柄必须 `sleep N | python3 …` 管道保活 stdin（Popen 直连 stdin 会 EOF 自毁），shell 串中中文名要加引号；④ 热插拔公告等待给足（实测 +0.6s～数秒）。下一步 P6（多手柄切换、全量门禁、退出无残留、文档同步、截图存档）。
- `2026-10-07 08:36` | **P6 完成（P6-1…P6-5 全 ✅），GOAL.md 全阶段（P0–P6）收口**。要点：① 多手柄切换——四只手柄同在、注入区分值后 AT-SPI 实读读数随所选设备变化（B 50%/50% ↔ A 81%/6%），B 按住 A 键期间选中 A/B 两张截图 PIL 差分 3424px 证明高亮不串读；② 全量门禁——fmt OK、clippy warning=0、test 31 passed + 4 ignored、release 构建产物 `target/release/proton-launch`；③ 退出无残留——Wayland 下 `alt+F4`/`xdotool windowclose` 都走不到优雅关闭（后者只毁 surface 留残留进程），改 X11 直送 `WM_DELETE_WINDOW`（`p6_close.py`）后 `pgrep -x proton-launch` 无输出、日志无异常；④ 文档——README×2 特性条、UI规范书 §3.5、项目结构规划书目录树+§3.9、**新建 `docs/手柄虚拟测试指南.md`**，`grep -rn 手柄状态 README*.md docs/*.md` 9 处命中；⑤ `shots/` 存 5 张关键界面截图（overview/devices/vibration/empty/noff，前两张视觉复核过）。**§8 验收 1–9 走查**：1/2/5/6/7/8/9 ✅；3/4 的离线侧 ✅（命令发出+置灰说明），「马达真动/扳机真动」手感两条维持 🟡 需真机可延后（B5）。登记 B6（08:28 `/dev/uinput` 变 600 root 致虚拟手柄创建被拒，复测前需恢复 uaccess）。环境备忘：弹层截图 `import -window` 截不到，用 `spectacle -b -n -f` 全屏后裁剪；下拉关闭态方向键不换选，须开弹层内操作；清理 `pkill -x proton-launch`、`pkill -f "uinput_pad[.]py"` 已执行，ydotoold 当前在跑（pid 229159，复测再遇卡键 CPU 异常先 `pkill -x ydotoold`）。全部改动未提交，待你审阅。
- `2026-10-07 09:46` | **A8 修订落地（你验收前的指令）**：手柄状态入口**移出游戏列表**，改为侧栏**视窗底部独立固定区**——`navigation.rs` 新增 `gamepad_list_box`（`navigation-sidebar` 类，内含原 `gamepad_row`），`root` 顺序改为 头部 → 分隔线 → 可滚动列表 → 分隔线 → 固定区，`append_fixed_entry` 与列表内「带 12px 边距的 `ListBoxRow` 分隔符」删除（改全宽标准 `gtk::Separator`）；`app.rs` Ui 字段 `gamepad_separator_row` → `gamepad_list_box`，新增其 `row_selected` 切页回调，`refresh_sidebar` 不再重建固定项，`show_content_page` / `show_welcome` 反选固定行、`show_gamepad_page` 兜底选中；`window.rs` 同步字段。门禁真实输出：`cargo fmt --check` → `FMT_OK`、`cargo clippy --all-targets` → `warning` 计数 `0`、`cargo test` → `31 passed; 0 failed; 4 ignored`、`cargo build` → `Finished dev profile`。**视觉验收 🟡 待你执行**（按你要求我未做任何视觉检查）：缩放窗口 / 拉长游戏列表滚动时该行应恒贴视窗底部、上方分隔线应为正常全宽分隔线；本机现有实例 pid 3170（`target/debug/proton-launch`）仍是旧代码，需重启后才看得到新布局。`docs/UI规范书.md` §3.4/§3.5 与本表 A8、P2-1、D1 已同步修订。
- `2026-10-07 09:54` | **修「导航栏同时高亮两条」**：根因是手柄状态行搬进独立 `gamepad_list_box` 后，两个 ListBox **各自保留选中态**——点「手柄状态」时不再自动取消游戏行，于是「游戏行 + 手柄行」同时高亮。修法三处（互斥选中，恒有且仅有一条）：① `show_gamepad_page` 进页时 `list_box.select_row(None)` 反选游戏行；② `show_content_page`/`show_welcome` 已有的固定行反选保持不变；③ `add_game`、`delete_selected` 改为**先切页再 `refresh_sidebar`**（原顺序在手柄页时 `refresh_sidebar` 按当前页传 `selected_id=None`，会让新行没有高亮）。两个列表均 `SelectionMode::Single` + `activate_on_single_click(true)`，单击即单选。门禁：`cargo fmt --check` ✅、`cargo clippy --all-targets` 0 warning、`cargo test` 31 passed + 4 ignored、`cargo build` ✅。**视觉验收 🟡 待你执行**（未做任何视觉检查，旧实例 pid 3170 需重启）。
- `2026-10-07 11:00` | **修「一启动就同时选中游戏行 + 手柄状态行」**（09:54 那轮没盖住的启动路径）：根因不在点击链路，而在**启动首帧的初始焦点遍历**——`launch` → `window.present()` → `gtk_window_show` → `gtk_window_move_focus(TAB_FORWARD)` 把焦点送进底部固定区的「手柄状态」行，GTK **焦点进入 `ListBox` 行即选中**（`gtk_list_box_row_focus` → `gtk_list_box_row_set_focus` → `gtk_list_box_update_selection_full`；该函数先 `update_cursor` 抢焦点、再选中发信号，`gtk/gtklistbox.c` 源码核对）；此刻 `App` 仍被 `state.borrow().ui.window.present()` 占用，`row_selected` 回调被 `with_app` 的重入保护**静默丢弃**，反选/切页都没跑 → 两条同时高亮。修前实测：启动后 AT-SPI `游戏列表 nSelected=1(鸣潮)` + `手柄列表 nSelected=1(手柄状态)`；诊断栈帧 `gtk_widget_show ← present ← app::launch`（`[SEL-BT]` 回溯，本轮已删）。**修法两处**：① 新增 `App::settle_first_frame`（`present()` 之后第一个 idle 执行）+ `App::sync_sidebar_selection`，按当前页把两个 ListBox 对齐回「同一时刻只有一条高亮」，并把键盘焦点交回当前页对应的行（否则焦点停在手柄行，回车/空格会直接切页；空列表时交给「添加游戏」按钮）；② `launch` 里给 present 期间的借用加注释，写明**不能**改成 clone window 后 present（否则回调会执行、首帧会误切手柄页），同时删掉调试期 `[SEL-BT]` 全栈回溯打印。**门禁真实输出**：`cargo fmt --check` → 无 diff、`cargo clippy --all-targets -- -D warnings` → `Finished dev profile`（0 警告）、`cargo test` → `31 passed; 0 failed; 4 ignored`、`cargo build --release` → `Finished release profile`。**修后实测（重启实例，AT-SPI + 像素双重取证）**：启动后 `游戏列表选中=['鸣潮'] 手柄列表选中=[]`、状态栏 `已选择: 鸣潮`；像素（窗口裁剪图 `shots/fix-nav-startup-config.png`）鸣潮行均值 `225` vs 手柄行 `234`（侧栏空白基线 `235`）→ 只有一行高亮；`xdotool key Tab` → `游戏列表选中=[] 手柄列表选中=['手柄状态']`、像素 `shots/fix-nav-gamepad-page.png` 手柄行 `214/217/221` vs 鸣潮行 `234`；`shift+Tab` 回到 `游戏列表选中=['鸣潮'] 手柄列表选中=[]`，往返两轮恒只有一条高亮。**注意**：本轮为取证关掉了你 10:25 启动的旧实例（pid 13195），当前运行的是修后构建（`target/debug/proton-launch`，Wayland）。