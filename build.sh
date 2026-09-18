#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
PROJECT_DIR="$(pwd)"
BUILD_DIR="$PROJECT_DIR/build"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
ARCH="amd64"
RPM_ARCH="x86_64"
PKG_NAME="proton-launch"
TARGET_DIR="$PROJECT_DIR/target/release"
BINARY="$TARGET_DIR/$PKG_NAME"
ICON="$PROJECT_DIR/icon.png"
DESKTOP_FILE="$BUILD_DIR/$PKG_NAME.desktop"

echo "=== Proton Launch Manager Build Script ==="
echo "Version: $VERSION"
echo "Arch: $ARCH"

# create build dir
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# build release binary
echo ""
echo ">>> Building release binary ..."
cargo build --release

if [ ! -f "$BINARY" ]; then
    echo "ERROR: binary not found at $BINARY"
    exit 1
fi

# create .desktop file
cat > "$DESKTOP_FILE" << EOF
[Desktop Entry]
Name=Proton Launch Manager
Name[zh]=Proton 启动管理器
Comment=Proton game launcher GUI based on umu-run
Comment[zh]=基于 umu-run 的 Proton 游戏启动 GUI
Exec=$PKG_NAME
Icon=$PKG_NAME
Terminal=false
Type=Application
StartupWMClass=com.protonlaunch.Manager
Categories=Game;Utility;
EOF

# ──────────────────────────────────────────────────
# deb
# ──────────────────────────────────────────────────
echo ""
echo ">>> Building .deb ..."
DEB_DIR="$BUILD_DIR/deb"
mkdir -p "$DEB_DIR/DEBIAN"
mkdir -p "$DEB_DIR/usr/bin"
mkdir -p "$DEB_DIR/usr/share/applications"
mkdir -p "$DEB_DIR/usr/share/icons/hicolor/512x512/apps"

cat > "$DEB_DIR/DEBIAN/control" << EOF
Package: $PKG_NAME
Version: $VERSION
Section: games
Priority: optional
Architecture: $ARCH
Maintainer: liangzhaoyuan12
Description: Proton game launcher GUI based on umu-run
 A graphical Proton launch manager using umu-run,
 with environment variable configuration similar to Lutris.
Homepage: https://github.com/liangzhaoyuan12/proton-launch
Depends: python3, libgtk-4-1 (>= 4.18), libadwaita-1-0 (>= 1.4)
Recommends: umu-run | python3-umu-run
EOF

cp "$BINARY" "$DEB_DIR/usr/bin/$PKG_NAME"
cp "$DESKTOP_FILE" "$DEB_DIR/usr/share/applications/$PKG_NAME.desktop"
cp "$ICON" "$DEB_DIR/usr/share/icons/hicolor/512x512/apps/$PKG_NAME.png"

dpkg-deb --root-owner-group --build "$DEB_DIR" "$BUILD_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
echo "    -> $BUILD_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"

# ──────────────────────────────────────────────────
# rpm
# ──────────────────────────────────────────────────
echo ""
echo ">>> Building .rpm ..."
RPM_DIR="$BUILD_DIR/rpm"
mkdir -p "$RPM_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

SPEC_FILE="$RPM_DIR/SPECS/$PKG_NAME.spec"

cat > "$SPEC_FILE" << EOF
Name: $PKG_NAME
Version: $VERSION
Release: 1%{?dist}
Summary: Proton game launcher GUI based on umu-run
License: MIT
URL: https://github.com/liangzhaoyuan12/proton-launch
BuildArch: $RPM_ARCH
Requires: python3
Requires: gtk4
Requires: libadwaita
Recommends: umu-run

%description
A graphical Proton launch manager using umu-run,
with environment variable configuration similar to Lutris.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/512x512/apps
install -m 755 $BINARY %{buildroot}/usr/bin/$PKG_NAME
install -m 644 $DESKTOP_FILE %{buildroot}/usr/share/applications/$PKG_NAME.desktop
install -m 644 $ICON %{buildroot}/usr/share/icons/hicolor/512x512/apps/$PKG_NAME.png

%files
/usr/bin/$PKG_NAME
/usr/share/applications/$PKG_NAME.desktop
/usr/share/icons/hicolor/512x512/apps/$PKG_NAME.png

%changelog
* $(LANG=C date '+%a %b %d %Y') liangzhaoyuan12 <liangzhaoyuan12> - $VERSION-1
- Initial release
EOF

rpmbuild --define "_topdir $RPM_DIR" -bb "$SPEC_FILE"
RPM_FILE=$(find "$RPM_DIR/RPMS" -name "${PKG_NAME}-${VERSION}*.rpm" -print -quit 2>/dev/null || true)
if [ -n "$RPM_FILE" ]; then
    mv "$RPM_FILE" "$BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.rpm"
fi
echo "    -> $BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.rpm"

# ──────────────────────────────────────────────────
# pacman (Arch Linux)
# ──────────────────────────────────────────────────
echo ""
echo ">>> Building .pkg.tar.zst (pacman) ..."
PACMAN_DIR="$BUILD_DIR/pacman"
mkdir -p "$PACMAN_DIR"
mkdir -p "$PACMAN_DIR/usr/bin"
mkdir -p "$PACMAN_DIR/usr/share/applications"
mkdir -p "$PACMAN_DIR/usr/share/icons/hicolor/512x512/apps"

cp "$BINARY" "$PACMAN_DIR/usr/bin/$PKG_NAME"
cp "$DESKTOP_FILE" "$PACMAN_DIR/usr/share/applications/$PKG_NAME.desktop"
cp "$ICON" "$PACMAN_DIR/usr/share/icons/hicolor/512x512/apps/$PKG_NAME.png"

cat > "$PACMAN_DIR/.PKGINFO" << EOF
pkgname = $PKG_NAME
pkgver = $VERSION-1
pkgdesc = Proton game launcher GUI based on umu-run
url = https://github.com/liangzhaoyuan12/proton-launch
builddate = $(date +%s)
packager = liangzhaoyuan12
size = $(stat -c%s "$BINARY")
arch = $RPM_ARCH
license = MIT
depend = python3
depend = gtk4
depend = libadwaita
makepkgopt = !mtree
EOF

cd "$PACMAN_DIR"
tar -cf "$BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.pkg.tar" \
    .PKGINFO usr/
cd "$PROJECT_DIR"
zstd -q -c "$BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.pkg.tar" \
    > "$BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.pkg.tar.zst"
rm "$BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.pkg.tar"
echo "    -> $BUILD_DIR/${PKG_NAME}_${VERSION}_${RPM_ARCH}.pkg.tar.zst"

# ──────────────────────────────────────────────────
# tar.gz + README
# ──────────────────────────────────────────────────
echo ""
echo ">>> Building .tar.gz ..."
TARGZ_DIR="$BUILD_DIR/$PKG_NAME-$VERSION-linux-$ARCH"
mkdir -p "$TARGZ_DIR"

cp "$BINARY" "$TARGZ_DIR/$PKG_NAME"

cat > "$TARGZ_DIR/README.md" << EOF
# Proton Launch Manager v$VERSION

A graphical Proton launch manager using umu-run,
with environment variable configuration similar to Lutris.

## Dependencies (required)

- **python3** — required to run umu-run
- **GTK 4** runtime libraries
- **libadwaita** runtime libraries

## Runtime Recommends

- \`umu-run\` — will be auto-extracted from the binary if not installed

## Usage
\`\`\`
./$PKG_NAME
\`\`\`

1. Click 「＋」 to create a new game
2. Select the executable and configure arguments
3. Set environment variables as needed
4. Click 「保存配置」 to persist (Ctrl+S)
5. Click 「运行」 to launch, 「停止」 to terminate
6. Click 「日志」 to view runtime logs, with copy-to-clipboard support

## Build from Source

Runtime/UI toolkit: GTK 4 + libadwaita（界面为 GNOME 原生控件，字体使用系统字体）.

### Debian / Ubuntu
\`\`\`
sudo apt-get install -y python3 build-essential pkg-config libgtk-4-dev libadwaita-1-dev
\`\`\`

### Fedora / RHEL
\`\`\`
sudo dnf install python3 gtk4-devel libadwaita-devel pkg-config
\`\`\`

### Arch Linux
\`\`\`
sudo pacman -S python gtk4 libadwaita pkgconf
\`\`\`

Then:
\`\`\`
git clone https://github.com/liangzhaoyuan12/proton-launch.git
cd proton-launch
cargo build --release
\`\`\`

## License
MIT
EOF

cd "$BUILD_DIR"
tar -czf "${PKG_NAME}_${VERSION}_${ARCH}.tar.gz" "$PKG_NAME-$VERSION-linux-$ARCH/"
cd "$PROJECT_DIR"
echo "    -> $BUILD_DIR/${PKG_NAME}_${VERSION}_${ARCH}.tar.gz"

echo ""
echo "=== All packages built successfully ==="
ls -lh "$BUILD_DIR"/*.{deb,rpm,pkg.tar.zst,tar.gz} 2>/dev/null || echo "(some packages may not have been generated)"
