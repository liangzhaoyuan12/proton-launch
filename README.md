# Proton Launch Manager

A graphical Proton launch GUI manager using `umu-run`, inspired by Lutris's Wine configuration interface. Built with Rust + egui/eframe.

This project aims to create a high-performance, high-stability Proton game manager — Lutris tends to crash frequently, and this project seeks to provide a lightweight, reliable alternative built on **umu-run**.

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
- **Bilingual UI** — Switch between Chinese and English instantly; language preference is remembered
- **Persistent Config** — JSON format, saved to `~/.config/proton-launch/games.json`

## Dependencies

**Runtime:**
- Python 3 (required by umu-run)
- glibc

**Build-time:**

| Distro | Command |
|--------|---------|
| Debian / Ubuntu | `sudo apt-get install -y python3 libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev` |
| Fedora / RHEL | `sudo dnf install python3 clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel` |
| Arch Linux | `sudo pacman -S python clang gtk3 libxcb libxkbcommon openssl pkg-config` |

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
├── proton-launch_0.1.0_amd64.deb
├── proton-launch_0.1.0_x86_64.rpm
├── proton-launch_0.1.0_x86_64.pkg.tar.zst
└── proton-launch_0.1.0_amd64.tar.gz
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

1. Click 「＋ Add」 on the left to create a new game
2. Select the executable and configure arguments
3. Set environment variables as needed (check the box and fill in the value)
4. Click 「💾 Save Config」 to persist
5. Click 「▶ Run」 to launch the game

## Repositories

- **GitHub**: <https://github.com/liangzhaoyuan12/proton-launch>
- **Gitee**: <https://gitee.com/liangzhaoyuan12/proton-launch>
- **umu-run**: <https://github.com/Open-Wine-Components/umu-launcher>

## License

[MIT](LICENSE)
