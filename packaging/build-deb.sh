#!/usr/bin/env bash
# Build the thallium-files .deb into dist/. Heavy: run it in CI (the
# `package` job), not on a laptop.
set -euo pipefail

cd "$(dirname "$0")/.."

# The workspace version plus the commit it was built from, so every build
# sorts after the last and a rebuild of new code is never mistaken for the
# version already installed.
base="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
stamp="$(git log -1 --format=%cd --date=format:%Y%m%d%H%M%S 2>/dev/null || date +%Y%m%d%H%M%S)"
commit="$(git rev-parse --short HEAD 2>/dev/null || echo local)"
version="${THALLIUM_FILES_VERSION:-${base}+${stamp}.g${commit}}"
arch="$(dpkg --print-architecture)"
pkg="thallium-files"
pkg_dir="$PWD/target/deb-root/${pkg}_${version}_${arch}"

cargo build --release -p thallium-backend --features qt --bin thallium-file-manager

rm -rf "$pkg_dir"
mkdir -p "$pkg_dir/DEBIAN" "$pkg_dir/usr/bin" "$pkg_dir/usr/share/applications" \
	"$pkg_dir/usr/share/dbus-1/services" "$pkg_dir/usr/share/doc/$pkg" \
	"$pkg_dir/usr/share/xdg-desktop-portal/portals"

install -m 0755 target/release/thallium-file-manager "$pkg_dir/usr/bin/thallium-file-manager"
install -m 0644 data/io.thallium.Files.desktop "$pkg_dir/usr/share/applications/io.thallium.Files.desktop"
install -m 0644 data/io.thallium.Files.FileManager1.service \
	"$pkg_dir/usr/share/dbus-1/services/io.thallium.Files.FileManager1.service"
install -m 0644 data/org.freedesktop.impl.portal.desktop.thallium.service \
	"$pkg_dir/usr/share/dbus-1/services/org.freedesktop.impl.portal.desktop.thallium.service"
install -m 0644 data/thallium.portal "$pkg_dir/usr/share/xdg-desktop-portal/portals/thallium.portal"
install -m 0644 README.md "$pkg_dir/usr/share/doc/$pkg/README.md"
install -m 0644 LICENSE "$pkg_dir/usr/share/doc/$pkg/copyright"

cat > "$pkg_dir/DEBIAN/control" <<CONTROL
Package: $pkg
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: Thallium OS <maintainers@thallium.local>
Depends: libc6, libgcc-s1, libstdc++6, libqt6core6t64, libqt6gui6, libqt6qml6, libqt6dbus6, libqt6network6, qt6-wayland, qml6-module-qtquick, qml6-module-qtquick-controls, qml6-module-qtquick-templates, qml6-module-qtquick-shapes, qml6-module-qtquick-effects, qml6-module-qtquick-window, qml6-module-qtqml-workerscript, tar, util-linux, xdg-utils
Recommends: udisks2, libarchive-tools, ffmpegthumbnailer
Suggests: rclone
Description: File manager for Thallium OS
 Thallium Files is a Qt file manager on a Rust backend: tabs, live
 folders, background copy and move with conflict handling, trash,
 bookmarks shared with GTK, drives, archives, custom actions, bulk
 rename and undo. It answers org.freedesktop.FileManager1, so other
 applications' "Show in folder" opens it.
CONTROL

cat > "$pkg_dir/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
	update-desktop-database -q /usr/share/applications || true
fi
exit 0
POSTINST
chmod 0755 "$pkg_dir/DEBIAN/postinst"

mkdir -p dist
dpkg-deb --root-owner-group --build "$pkg_dir" "dist/${pkg}_${version}_${arch}.deb"
echo "dist/${pkg}_${version}_${arch}.deb"
