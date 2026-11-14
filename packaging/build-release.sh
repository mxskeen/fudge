#!/usr/bin/env bash
# Build release packages for Linux x86_64: a .deb and a .tar.gz in dist/.
# Usage: packaging/build-release.sh [version]   (defaults to the workspace version)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
DEB_VERSION="$(printf '%s' "$VERSION" | tr '-' '~')"   # 0.1.0-alpha.1 -> 0.1.0~alpha.1
ARCH="amd64"
BIN="target/release/fudge"
OUT="dist"

[ -x "$BIN" ] || cargo build --release -p fudge
rm -rf "$OUT" && mkdir -p "$OUT"

install_tree() {  # $1 = destination root
  install -Dm755 "$BIN" "$1/usr/bin/fudge"
  install -Dm644 packaging/fudge.desktop "$1/usr/share/applications/fudge.desktop"
  for dir in packaging/icons/hicolor/*/apps; do
    size="$(basename "$(dirname "$dir")")"
    install -Dm644 "$dir/fudge.png" "$1/usr/share/icons/hicolor/$size/apps/fudge.png"
  done
  install -Dm644 LICENSE-MIT "$1/usr/share/doc/fudge/LICENSE-MIT"
  install -Dm644 LICENSE-APACHE "$1/usr/share/doc/fudge/LICENSE-APACHE"
  install -Dm644 README.md "$1/usr/share/doc/fudge/README.md"
}

# ---- .deb ----
PKG="$OUT/pkgroot"
install_tree "$PKG"
mkdir -p "$PKG/DEBIAN"
SIZE_KB=$(du -sk "$PKG/usr" | cut -f1)
cat > "$PKG/DEBIAN/control" <<CTRL
Package: fudge
Version: $DEB_VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Installed-Size: $SIZE_KB
Depends: libgtk-4-1 (>= 4.10), libadwaita-1-0 (>= 1.5), libx11-6, libxcb1, libc6
Recommends: xdotool
Maintainer: mxskeen <https://github.com/mxskeen/fudge>
Homepage: https://github.com/mxskeen/fudge
Description: Fast, private keyboard launcher
 Press Alt+Space, type, get the answer. Apps, files, math, units, currency,
 time zones, emoji, colours, kill, window switching and web searches.
 No account and no telemetry. Pre-alpha: Linux X11 is the tested target.
CTRL
dpkg-deb --root-owner-group --build "$PKG" "$OUT/fudge_${DEB_VERSION}_${ARCH}.deb" >/dev/null
rm -rf "$PKG"

# ---- tar.gz ----
TAR="fudge-${VERSION}-linux-x86_64"
install_tree "$OUT/$TAR/root"
cp LICENSE-MIT LICENSE-APACHE README.md "$OUT/$TAR/"
cat > "$OUT/$TAR/install.sh" <<'INST'
#!/usr/bin/env sh
# Installs Fudge into ~/.local (no root needed). Run: ./install.sh
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
mkdir -p "$prefix"
cp -r "$here/root/usr/." "$prefix/"
echo "Installed to $prefix. Make sure $prefix/bin is on your PATH."
echo "Needs GTK 4 and libadwaita 1.5 (Ubuntu 24.04+: sudo apt install libgtk-4-1 libadwaita-1-0)."
echo "Start it with: fudge daemon     Autostart: fudge install-autostart"
INST
chmod +x "$OUT/$TAR/install.sh"
tar -C "$OUT" -czf "$OUT/$TAR.tar.gz" "$TAR"
rm -rf "$OUT/$TAR"

( cd "$OUT" && sha256sum ./*.deb ./*.tar.gz | sed 's#\./##' > SHA256SUMS )
ls -lh "$OUT"
