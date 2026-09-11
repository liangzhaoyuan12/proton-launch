# Proton Launch Manager

A graphical Proton launch GUI manager using `umu-run`, inspired by Lutris's Wine configuration interface. Built with Rust + GTK4 / libadwaita (native GNOME interface, Chinese UI).

This project aims to create a high-performance, high-stability Proton game manager — Lutris tends to crash frequently, and this project seeks to provide a lightweight, reliable alternative built on **umu-run**.

> **中文版本**: [README.zh-CN.md](README.zh-CN.md)

## Features

- **Game Management** — Add, delete, and edit multiple game configurations
- **Side-loaded umu-run** — The `umu-run` binary is embedded at compile time, no manual installation needed
- **Proton Configuration** — Select Proton version/path with a browse dialog
- **Categorized Environment Variables** — 15 categories covering all common Proton/Wine env vars
  - Core, DLL, Graphics, Wayland, Sync, Memory/CPU, Audio, Vulkan, Mesa/OpenGL, NVIDIA, DLSS/FSR/XeSS, Performance, Input, Font, Other
- **One-click Toggle** — All 0/1 boolean variables render as checkboxes
- **Custom Env Vars** — Add arbitrary key-value pairs
- **Modder/Injector** — Inject external tools via `PROTON_REMOTE_DEBUG_CMD`
- **Renderer Selection** — One-click `-dx11` / `-dx12` / `-opengl` / `-vulkan` launch args
- **Multi-process Management** — Each game starts independently with its own PID and stop button
- **Unsaved Changes Protection** — Edits are cached in memory; switching items preserves your work. Close confirmation dialog warns before discarding unsaved changes
- **Per-game Runtime Logs** — stdout/stderr captured in real-time, viewable via the 日志 button with copy-to-clipboard support. Logs are in-memory only and cleared on app restart
- **Native GNOME UI** — GTK4 + libadwaita widgets, follows the system light/dark theme, collapses to stack navigation on narrow windows
- **Persistent Config** — JSON format, saved to `~/.config/proton-launch/games.json`

## Dependencies

**Runtime:**
- Python 3 (required by umu-run)
- GTK 4 runtime libraries
- libadwaita runtime libraries

**Build-time (GTK4 + libadwaita):**

| Distro | Command |
|--------|---------|
| Debian / Ubuntu | `sudo apt-get install -y python3 build-essential pkg-config libgtk-4-dev libadwaita-1-dev` |
| Fedora / RHEL | `sudo dnf install python3 gtk4-devel libadwaita-devel pkg-config` |
| Arch Linux | `sudo pacman -S python gtk4 libadwaita pkgconf` |

## Build

### Option 1: Direct Build

```bash
git clone https://github.com/liangzhaoyuan12/proton-launch.git
cd proton-launch
cargo build --release
./target/release/proton-launch
```

### Option 2: Build Script

Automatically builds and packages into deb, rpm, pacman, tar.gz:

```bash
./build.sh
```

Output is placed in the `build/` directory:

```
build/
├── proton-launch_*.*.*_amd64.deb
├── proton-launch_*.*.*_x86_64.rpm
├── proton-launch_*.*.*_x86_64.pkg.tar.zst
└── proton-launch_*.*.*_amd64.tar.gz
```

## Install Pre-built Packages

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

### Generic (tar.gz)

Extract and run directly:

```bash
tar -xzf proton-launch_*.tar.gz
cd proton-launch-*-linux-*
./proton-launch
```

## Usage

1. Click the 「＋」 button in the sidebar to create a new game
2. Select the executable and configure arguments
3. Set environment variables as needed (turn the switch on and fill in the value)
4. Click 「保存配置」 to persist (`Ctrl+S`)
5. Click 「运行」 to launch the game, 「停止」 to terminate its process
6. Click 「日志」 to view real-time runtime logs (stdout/stderr), with a 「复制」 button to copy to clipboard

## Repositories

- **GitHub**: <https://github.com/liangzhaoyuan12/proton-launch>
- **Gitee**: <https://gitee.com/liangzhaoyuan12/proton-launch>
- **umu-run**: <https://github.com/Open-Wine-Components/umu-launcher>

## License

[MIT](LICENSE)
