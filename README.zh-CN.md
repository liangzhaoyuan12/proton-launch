# Proton 启动管理器

基于 `umu-run` 的图形化 Proton 游戏启动管理器，参考 Lutris 的 Wine 配置界面设计。使用 Rust + GTK4 / libadwaita 构建（原生 GNOME 界面，中文 UI）。

本项目旨在打造高性能、高稳定性的 Proton 游戏管理器——Lutris 频繁崩溃，本项目基于 **umu-run** 提供轻量、可靠的替代方案。

> **English version**: [README.md](README.md)

## 功能特性

- **游戏管理** — 添加、删除、编辑多个游戏配置
- **内嵌 umu-run** — 编译时嵌入 `umu-run` 二进制，无需手动安装
- **Proton 配置** — 选择 Proton 版本/路径，支持浏览对话框
- **分类环境变量** — 15 个分类，覆盖所有常用 Proton/Wine 环境变量
  - 核心、DLL、图形渲染、Wayland、同步机制、内存/CPU、音频、Vulkan、Mesa/OpenGL、NVIDIA、DLSS/FSR/XeSS、性能监控、输入设备、字体、其他
- **一键开关** — 所有 0/1 布尔变量渲染为复选框
- **自定义环境变量** — 添加任意键值对
- **修改器/注入器** — 通过 `PROTON_REMOTE_DEBUG_CMD` 注入外部工具
- **渲染器选择** — 一键选择 `-dx11` / `-dx12` / `-opengl` / `-vulkan` 启动参数
- **多进程管理** — 每个游戏独立启动，各自 PID 和停止按钮
- **未保存修改保护** — 编辑内容缓存在内存中，切换条目不丢失；关闭窗口前弹出确认对话框
- **逐游戏运行日志** — 实时捕获 stdout/stderr，通过「日志」按钮查看，支持复制到剪贴板。日志仅存于内存，重启应用自动清空
- **原生 GNOME 界面** — GTK4 + libadwaita 控件，跟随系统明暗主题，窄窗口自动折叠为栈式导航
- **持久化配置** — JSON 格式，保存至 `~/.config/proton-launch/games.json`

## 依赖

**运行时：**
- Python 3（umu-run 必需）
- GTK 4 运行时库
- libadwaita 运行时库

**编译时（GTK4 + libadwaita）：**

| 发行版 | 命令 |
|--------|------|
| Debian / Ubuntu | `sudo apt-get install -y python3 build-essential pkg-config libgtk-4-dev libadwaita-1-dev` |
| Fedora / RHEL | `sudo dnf install python3 gtk4-devel libadwaita-devel pkg-config` |
| Arch Linux | `sudo pacman -S python gtk4 libadwaita pkgconf` |

## 构建

### 方式一：直接构建

```bash
git clone https://gitee.com/liangzhaoyuan12/proton-launch.git
cd proton-launch
cargo build --release
./target/release/proton-launch
```

### 方式二：构建脚本

自动构建并打包为 deb、rpm、pacman、tar.gz：

```bash
./build.sh
```

输出位于 `build/` 目录：

```
build/
├── proton-launch_*.*.*_amd64.deb
├── proton-launch_*.*.*_x86_64.rpm
├── proton-launch_*.*.*_x86_64.pkg.tar.zst
└── proton-launch_*.*.*_amd64.tar.gz
```

## 安装预构建包

### Debian / Ubuntu

```bash
sudo dpkg -i proton-launch_*.deb
```

### Fedora / RHEL

```bash
sudo dnf install ./proton-launch_*.rpm
```

### Arch Linux

```bash
sudo pacman -U proton-launch_*.pkg.tar.zst
```

### 通用（tar.gz）

解压后直接运行：

```bash
tar -xzf proton-launch_*.tar.gz
cd proton-launch-*-linux-*
./proton-launch
```

## 使用方法

1. 点击侧边栏「＋」按钮创建新游戏
2. 选择可执行文件并配置参数
3. 按需设置环境变量（打开开关并填写值）
4. 点击「保存配置」持久化（`Ctrl+S`）
5. 点击「运行」启动游戏，「停止」终止进程
6. 点击「日志」查看运行时日志（stdout/stderr），支持「复制」到剪贴板

## 仓库

- **Gitee**: <https://gitee.com/liangzhaoyuan12/proton-launch>
- **GitHub**: <https://github.com/liangzhaoyuan12/proton-launch>
- **umu-run**: <https://github.com/Open-Wine-Components/umu-launcher>

## 许可证

[MIT](LICENSE)
