#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
output="$PWD/dist"
stage="$output/stage"
mkdir -p "$output"
rm -rf "$stage"
install -Dm755 target/release/mango-layout-tray "$stage/usr/bin/mango-layout-tray"
install -Dm644 data/mango-layout-tray.desktop "$stage/usr/share/applications/mango-layout-tray.desktop"
install -Dm644 data/mango-layout-tray.svg "$stage/usr/share/icons/hicolor/scalable/apps/mango-layout-tray.svg"
install -Dm644 LICENSE "$stage/usr/share/licenses/mango-layout-tray/LICENSE"
install -Dm644 README.md "$stage/usr/share/doc/mango-layout-tray/README.md"
layer_lib=$(pkg-config --variable=libdir gtk4-layer-shell-0)/libgtk4-layer-shell.so.0
install -Dm755 "$(readlink -f "$layer_lib")" "$stage/usr/lib/mango-layout-tray/libgtk4-layer-shell.so.0"
install -Dm644 packaging/gtk4-layer-shell.LICENSE "$stage/usr/share/licenses/mango-layout-tray/gtk4-layer-shell.LICENSE"
patchelf --set-rpath "\$ORIGIN/../lib/mango-layout-tray" "$stage/usr/bin/mango-layout-tray"
tar -C "$stage/usr" -czf "$output/mango-layout-tray-$version-linux-$arch.tar.gz" .

if command -v dpkg-deb >/dev/null; then
    deb_arch="$arch"
    case "$arch" in x86_64) deb_arch=amd64;; aarch64) deb_arch=arm64;; esac
    mkdir -p "$stage/DEBIAN"
    cat > "$stage/DEBIAN/control" <<CONTROL
Package: mango-layout-tray
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: Mikkel Rask <mikkelrask@users.noreply.github.com>
Depends: libgtk-4-1 (>= 4.8), libc6 (>= 2.39), libgcc-s1, libwayland-client0
Description: Visual layout picker for MangoWM
 Standard Linux tray integration with live layout indicators and wireframe previews.
CONTROL
    dpkg-deb --build --root-owner-group "$stage" "$output/mango-layout-tray-${version}_${deb_arch}.deb"
    rm -rf "$stage/DEBIAN"
fi

if command -v rpmbuild >/dev/null; then
    rpmroot="$output/rpm"
    mkdir -p "$rpmroot"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
    cat > "$rpmroot/SPECS/mango-layout-tray.spec" <<SPEC
Name: mango-layout-tray
Version: $version
Release: 1
Summary: Visual layout picker for MangoWM
License: MIT
URL: https://github.com/mikkelrask/mango-layout-tray
Requires: gtk4 >= 4.8, glibc >= 2.39, libgcc, wayland-libs
AutoReqProv: no
%description
A system tray utility with live layout indicators and wireframe layout previews.
%install
mkdir -p %{buildroot}
cp -a "$stage/usr" %{buildroot}/
%files
/usr/bin/mango-layout-tray
/usr/lib/mango-layout-tray/
/usr/share/applications/mango-layout-tray.desktop
/usr/share/icons/hicolor/scalable/apps/mango-layout-tray.svg
/usr/share/licenses/mango-layout-tray/
/usr/share/doc/mango-layout-tray/
SPEC
    rpmbuild -bb --define "_topdir $rpmroot" --define '_build_id_links none' "$rpmroot/SPECS/mango-layout-tray.spec"
    find "$rpmroot/RPMS" -name '*.rpm' -exec cp {} "$output/" \;
fi
(cd "$output" && sha256sum ./*.tar.gz ./*.deb ./*.rpm 2>/dev/null > SHA256SUMS) || test -s "$output/SHA256SUMS"
