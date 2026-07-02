# Proton Launch Manager

基于 `umu-run` 的 Proton 启动 GUI 管理器，仿照 Lutris 的 Wine 配置界面设计，使用 Rust + egui/eframe 构建。

本项目立项是为了创建一个**高性能、高稳定性**的 Proton 游戏管理器——Lutris 经常动不动就崩溃，本项目基于 **umu-run** 致力于提供一款轻量可靠的替代方案。

## 功能

- **游戏管理** — 添加/删除/编辑多个游戏配置
- **umu-run 侧载** — `umu-run` 二进制编译时嵌入，无需额外安装
- **Proton 配置** — 选择 Proton 版本/目录，支持浏览选择
- **环境变量分类配置** — 15 个分类，涵盖所有常用 Proton/Wine 环境变量
  - 核心、DLL、图形渲染、Wayland、同步机制、内存/CPU、音频、Vulkan、Mesa/OpenGL、NVIDIA、DLSS/FSR/XeSS、性能监控、输入、字体、其他
- **布尔开关一键勾选** — 所有 0/1 变量显示为复选框
- **自定义环境变量** — 支持任意 key-value 对
- **修改器/注入器** — 通过 `PROTON_REMOTE_DEBUG_CMD` 注入外部工具
- **渲染器选择** — 一键添加 `-dx11` / `-dx12` / `-opengl` / `-vulkan` 启动参数
- **多进程管理** — 每个游戏独立启动、独立显示 PID 和停止按钮
- **中英文界面** — 右上角一键切换，语言选择自动记忆
- **配置持久化** — JSON 格式保存至 `~/.config/proton-launch/games.json`

## 依赖

**运行时依赖：**
- Python 3（umu-run 需要）
- glibc

**编译依赖：**

| 发行版 | 安装命令 |
|--------|----------|
| Debian / Ubuntu | `sudo apt-get install -y python3 libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev` |
| Fedora / RHEL | `sudo dnf install python3 clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel` |
| Arch Linux | `sudo pacman -S python clang gtk3 libxcb libxkbcommon openssl pkg-config` |

## 编译

### 方式一：直接编译

```bash
git clone https://gitee.com/liangzhaoyuan12/proton-launch.git
cd proton-launch
cargo build --release
./target/release/proton-launch
```

### 方式二：打包脚本

自动编译并打包为 deb、rpm、pacman、tar.gz：

```bash
./build.sh
```

产物在 `build/` 目录下：

```
build/
├── proton-launch_0.1.0_amd64.deb
├── proton-launch_0.1.0_x86_64.rpm
├── proton-launch_0.1.0_x86_64.pkg.tar.zst
└── proton-launch_0.1.0_amd64.tar.gz
```

## 安装预编译包

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

### 通用 (tar.gz)

解压后直接运行：

```bash
tar -xzf proton-launch_*.tar.gz
cd proton-launch-*-linux-*
./proton-launch
```

## 使用

1. 点击左侧「＋ 添加」创建新游戏
2. 选择可执行文件，配置参数
3. 按需设置环境变量（勾选并填写）
4. 点击「💾 保存配置」保存
5. 点击「▶ 运行」启动游戏

## 仓库

- **GitHub**: <https://github.com/liangzhaoyuan12/proton-launch>
- **Gitee**: <https://gitee.com/liangzhaoyuan12/proton-launch>
- **umu-run**: <https://github.com/Open-Wine-Components/umu-launcher>

## 开源协议

[MIT](LICENSE)
