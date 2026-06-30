### umu-cli核心环境变量

| 变量名 | 是否必需 | 说明 | 默认值 |
|--------|----------|------|--------|
| `WINEPREFIX` | 可选 | Wine 前缀目录路径 | `$HOME/Games/umu/$GAMEID`（若 `GAMEID` 未设置则为 `$HOME/Games/umu/umu-default`） |
| `PROTONPATH` | 可选 | Proton 目录的完整路径、版本名称（如 `GE-Proton9-5`）或代号（如 `GE-Proton`） | `UMU-Proton`（最新稳定版） |
| `GAMEID` | 可选 | 任意值，或 `umu-database` 中的有效 ID | `umu-default`（不自动应用任何修复） |
| `STORE` | 可选 | 任意值，或 `umu-database` 中的有效商店 ID | `"none"` |


以下是将b站原文内容整理成的表格，按类别分组，方便查阅：

## 常用兼容层

| 兼容层名称 | 说明 |
|---|---|
| GE-Proton | 最推荐使用 |
| Proton-CachyOS-SLR | CachyOS 默认，新特性更多些 |
| Proton-EM | 有些小众功能 |
| Kron4ek Wine-Builds Staging-Tkg | 对于某些小众游戏可能有奇效 |

> 目前建议使用 [ProtonPlus](https://github.com/Vysp3r/protonplus) 进行版本管理

---

## 核心配置变量

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINEPREFIX` | 指定 Wine 前缀（虚拟 Windows 环境）目录，默认 `~/.wine`，可用于隔离不同程序环境 | `~/.wine-game` |
| `STEAM_COMPAT_DATA_PATH` | Proton 的自定义参数，类似 `WINEPREFIX` | — |
| `COPYPREFIX` | 复制前缀，常用于同步桌面端与 Steam Deck 的前缀文件（GE-Proton 引入） | `0`=禁用（默认），`1`=启用 |
| `SteamDeck` | 禁用 SteamDeck 模式 | `0`=禁用，`1`=启用（SteamDeck 默认参数） |
| `PROTON_ADD_CONFIG` | 快捷设置参数（Proton-EM 引入） | `fsr4,wayland,hdr` 等 |

---

## DLL 和 Windows 环境变量管理

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINEDLLOVERRIDES` | 覆盖 DLL 加载顺序 | `n`=程序原生附带，`b`=Wine 内置，如 `n,b`=优先原生；空值=禁用；支持通配符如 `*d3d*=n`；分号分隔多个 DLL |
| `WINEDLLPATH` | 指定额外 DLL 搜索路径 | 冒号分隔多个路径，如 `/opt/dlls:/usr/local/wine/dlls` |
| `WINEPATH` | 为 Windows 程序添加 PATH 环境变量 | Windows 路径格式，分号分隔，如 `C:\\tools;Z:\\usr\\local\\bin`（Wine 默认将 `Z:` 映射到 Linux 根目录） |

---

## 图形渲染配置

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINE_D3D_CONFIG` | 配置 Direct3D 行为 | `renderer=vulkan`（推荐）/ `gl`；`csmt=0/1`（命令流多线程）；逗号分隔多选项；`nod3d12` 禁用 DX12（等效 `PROTON_NO_D3D12`），类似有 `nod3d9`、`nod3d10`、`nod3d11` |
| `WINE_DO_NOT_CREATE_DXGI_DEVICE_MANAGER` | 修复过场动画色块错误 | `0`=禁用（默认），`1`=启用 |
| `PROTON_NO_WM_DECORATION` | 禁用窗口管理器装饰，用于修复无边框全屏的鼠标输入问题 | `0`=禁用（默认），`1`=启用 |
| `DXVK_HUD` | DXVK 性能 HUD 显示 | `fps`=仅 FPS，`full`=完整信息，`fps,gpuload,devinfo`=组合显示，`1`=基础信息，`0`=关闭 |
| `DXVK_CONFIG_FILE` | 指定 DXVK 配置文件路径 | 如 `$HOME/.config/dxvk.conf`（可设置帧率限制、各向异性过滤等） |
| `DXVK_STATE_CACHE_PATH` | DXVK 着色器缓存路径 | 如 `$HOME/.cache/dxvk`（提升二次启动速度），可设为 `/tmp` 用于测试 |
| `VKD3D_CONFIG` | VKD3D-Proton 配置选项 | `dxr`=DXR 光追，`dxr11`=DX11 光追，`no_upload_hvv`=性能优化；逗号分隔多选项 |
| `VKD3D_SHADER_CACHE_PATH` | VKD3D 着色器缓存路径 | 如 `$HOME/.cache/vkd3d`（同 DXVK 缓存作用） |
| `WINE_FULLSCREEN_FSR` | 启用 AMD FSR 1 | `0`=禁用（默认），`1`=启用（需 Wine 8.6+，仅适用于 Vulkan 游戏） |
| `WINE_FULLSCREEN_FSR_MODE` | FSR 质量模式 | `0`=Ultra Quality，`1`=Quality（推荐），`2`=Balanced，`3`=Performance，`4`=Ultra Performance |
| `WINE_FULLSCREEN_FSR_STRENGTH` | FSR 锐化强度 | `0-5`，默认 `2`，`5` 为最高（可能过锐化） |
| `WINE_FULLSCREEN_FSR_CUSTOM_MODE` | 设置屏幕的虚拟分辨率 | 如 `1920x1080` |
| `WINE_FULLSCREEN_INTEGER_SCALING` | 启用整数缩放模式 | `0`=禁用（默认），`1`=启用 |

---

## Wayland 相关

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `PROTON_ENABLE_WAYLAND` | 启用 Wine-Wayland | `0`=禁用（默认），`1`=启用 |
| `PROTON_ENABLE_HDR` | Wine-Wayland 中使用 HDR，需同时启用 Wayland | `0`=禁用（默认），`1`=启用 |

> 注：Wine-Wayland 驱动支持较新，Bug 较多

---

## 同步机制

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINEESYNC` | 启用 esync（eventfd 同步） | `0`=禁用（默认），`1`=启用（需 `ulimit -Hn >= 524288`） |
| `WINEFSYNC` | 启用 fsync（futex2 同步） | `0`=禁用（默认），`1`=启用（需 Linux 5.16+，比 esync 更快） |
| `NTSYNC` | 启用 ntsync（原生 NT 同步） | `0`=禁用（默认），`1`=启用（Wine 9.0+，需 Linux 6.10+，性能最优） |

> 优先级：ntsync > fsync > esync，只启用一个

---

## 内存和进程管理

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINE_LARGE_ADDRESS_AWARE` | 32 位程序可使用大于 2GB 内存 | `1`=启用（最大约 3-4GB，解决内存不足崩溃） |
| `WINE_HEAP_DELAY_FREE` | 延迟堆内存释放 | 可能提升性能，但会增加内存占用 |
| `WINE_CPU_TOPOLOGY` | 模拟 CPU 拓扑 | 格式：`核心数:逻辑处理器数`，如 `4:8`（4核8线程）、`6:12`（6核12线程）、`8:8`（8核无HT） |

---

## 音频配置

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `PULSE_LATENCY_MSEC` | PulseAudio 延迟（毫秒） | `60`=默认稳定，`30`=低延迟，`15`=更低（可能爆音），`120`=高稳定性 |
| `SDL_AUDIODRIVER` | SDL 音频后端 | `pulse`=PulseAudio（常用），`alsa`=ALSA 直接，`pipewire`=PipeWire（推荐），`dummy`=静音 |
| `PIPEWIRE_LATENCY` | PipeWire 延迟设置 | 格式：`采样数/采样率`，`256/48000`=平衡（5.3ms），`128/48000`=低延迟（2.7ms），`512/48000`=高稳定（10.7ms） |

---

## Vulkan 驱动配置

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `VK_ICD_FILENAMES` | 指定 Vulkan ICD 文件 | 多 GPU 系统指定驱动，如 `radeon_icd.x86_64.json`、`nvidia_icd.json`=NVIDIA、`intel`=Intel |
| `AMD_VULKAN_ICD` | AMD Vulkan 驱动选择 | `RADV`=开源驱动（推荐），`AMDVLK`=官方闭源驱动 |

---

## Mesa / OpenGL 配置

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `MESA_GL_VERSION_OVERRIDE` | 强制 OpenGL 版本 | `4.6`=最新，`4.5COMPAT`=兼容模式，`3.3`=旧版兼容（需与 GLSL 版本对应） |
| `MESA_GLSL_VERSION_OVERRIDE` | 强制 GLSL 版本 | `460`=GLSL 4.60，`450`=GLSL 4.50，`330`=GLSL 3.30 |
| `MESA_NO_ERROR` | 禁用 GL 错误检查 | `1`=禁用检查（提升 5-15% 性能但隐藏错误），`0`=启用检查（默认） |
| `mesa_glthread` | Mesa 多线程优化 | `true`=启用（显著提升 OpenGL 性能），`false`=禁用（默认） |
| `MESA_LOADER_DRIVER_OVERRIDE` | 强制 Mesa 驱动 | `radeonsi`=AMD GCN/RDNA，`iris`=Intel Xe，`zink`=OpenGL over Vulkan，`nouveau`=NVIDIA 开源 |

---

## NVIDIA 驱动专用

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `__GL_THREADED_OPTIMIZATIONS` | NVIDIA OpenGL 多线程优化 | `1`=启用（可提升 10-30% 性能），`0`=禁用 |
| `__GL_SHADER_DISK_CACHE` | 着色器磁盘缓存 | `1`=启用（加快二次启动），`0`=禁用 |
| `__GL_SHADER_DISK_CACHE_PATH` | 缓存路径 | 自定义缓存位置，如 `$HOME/.nv/GLCache`（默认 `~/.nv/ComputeCache`），可设为 `/tmp` 加速 |
| `WINE_HIDE_NVIDIA_GPU` | 隐藏 NVIDIA GPU | 多 GPU 系统避免错误识别 |
| `__NV_PRIME_RENDER_OFFLOAD` | NVIDIA Prime 渲染卸载 | Optimus 笔记本必需 |
| `__GLX_VENDOR_LIBRARY_NAME` | GLX 提供方 | `nvidia`=强制 NVIDIA，`mesa`=强制 Mesa 集显 |

---

## DLSS / FSR / XeSS 更新相关

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `PROTON_DLSS_UPGRADE` | 自动将 DLSS 更新到最新版本（Proton-CachyOS 引入） | `0`=禁用（默认），`1`=启用 |
| `PROTON_DLSS_INDICATOR` | 在游戏中显示 DLSS 状态指示器（Proton-CachyOS 引入） | `0`=禁用（默认），`1`=启用 |
| `PROTON_FSR4_UPGRADE` | 自动将 FSR 更新到最新版本（Proton-CachyOS 引入） | `0`=禁用（默认），`1`=启用 |
| `FSR4_UPGRADE` | 自动将 FSR 更新到最新版本（GE-Proton 引入） | `0`=禁用（默认），`1`=启用 |
| `PROTON_FSR4_RDNA3_UPGRADE` | 使用 RDNA3 优化的 FSR4 DLL（Proton-CachyOS 引入） | `0`=禁用（默认），`1`=启用 |
| `PROTON_XESS_UPGRADE` | 自动将 XeSS 更新到最新版本（Proton-CachyOS 引入） | `0`=禁用（默认），`1`=启用 |

---

## 性能监控和调试

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `GALLIUM_HUD` | Mesa Gallium3D 内置性能 HUD | `fps`=仅 FPS，`"fps,cpu,gpu-load"`=多项（引号内），`"fps+cpu"`=叠加显示（加号），`help`=显示可用选项 |
| `MANGOHUD` | 启用 MangoHud 性能 HUD（第三方） | `0`=禁用（默认），`1`=启用（需安装 mangohud 包） |
| `MANGOHUD_CONFIG` | MangoHud 配置 | `fps,frametime`=常用组合，`full`=完整信息，`position=top-left/top-right/bottom-left/bottom-right` 指定位置 |
| `MANGOHUD_CONFIGFILE` | MangoHud 配置文件路径 | 如 `$HOME/.config/MangoHud/custom.conf`（复杂配置推荐使用配置文件） |

---

## 字体渲染

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `FREETYPE_PROPERTIES` | FreeType 字体渲染 | `truetype:interpreter-version=40`（`35`=v35 经典效果，`40`=v40 更清晰，推荐 CJK） |

---

## 输入设备

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `PROTON_PREFER_SDL` | 输入时优先使用 SDL（GE-Proton 引入） | `0`=禁用（默认），`1`=启用（通常用于修复非 Xbox 手柄时的控制输入问题） |
| `SDL_GAMECONTROLLERCONFIG` | 自定义手柄按键映射 | 完整映射字符串从 SDL GameController DB 获取 |
| `SDL_JOYSTICK_DEVICE` | 指定输入设备 | `js0`=手柄1，`js1`=手柄2，`/dev/input/event*`=evdev 设备 |

---

## 多 Wine 版本管理

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINEVERPATH` | 指定 Wine 版本路径 | 多版本共存管理，指向特定安装目录 |
| `WINE` | 直接指定 wine 可执行文件 | 用于 GE-Proton 等定制版本 |

---

## 其他

| 变量 | 说明 | 取值/示例 |
|---|---|---|
| `WINE_DISABLE_FAST_SYNC` | 禁用快速同步 | 兼容性调试用，会降低性能 |
| `WINE_DISABLE_WRITE_WATCH` | 禁用写入检测 | 某些反作弊系统需要，影响性能 |
| `STAGING_SHARED_MEMORY` | 启用共享内存 | Wine Staging 特有功能，提升性能 |

> **Steam 中填写方式**：非游戏自带启动项参数需以 `%command%` 结尾，例如 `WINEDLLOVERRIDES="version=n,b;wimmm=n,b" %command%`