#!/usr/bin/env bash
# Build a Fudge AppImage (x86_64) into dist/. Bundles GTK 4 and libadwaita via linuxdeploy.
# Usage: packaging/build-appimage.sh [version]
# Build on the oldest distro you want to support: an AppImage runs on anything with the same
# or newer glibc than the build machine.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
BIN="target/release/fudge"
OUT="dist"
TOOLS="${FUDGE_TOOLS_DIR:-$PWD/target/appimage-tools}"
APPDIR="$PWD/target/Fudge.AppDir"

[ -x "$BIN" ] || cargo build --release -p fudge
mkdir -p "$OUT" "$TOOLS"

fetch() {  # $1 = file name, $2 = url
  [ -x "$TOOLS/$1" ] && return
  curl -fsSL "$2" -o "$TOOLS/$1" && chmod +x "$TOOLS/$1"
}
fetch linuxdeploy https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
fetch linuxdeploy-plugin-gtk.sh https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh
fetch linuxdeploy-plugin-appimage https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage

rm -rf "$APPDIR"
export PATH="$TOOLS:$PATH"
export DEPLOY_GTK_VERSION=4
export VERSION
export APPIMAGE_EXTRACT_AND_RUN=1   # lets linuxdeploy run where FUSE is missing (CI, containers)
export LDAI_OUTPUT="$OUT/Fudge-${VERSION}-x86_64.AppImage"

linuxdeploy \
  --appdir "$APPDIR" \
  --executable "$BIN" \
  --desktop-file packaging/fudge.desktop \
  --icon-file packaging/icons/hicolor/256x256/apps/fudge.png \
  --plugin gtk \
  --output appimage

# linuxdeploy names the file after the desktop entry; normalise it.
produced="$(ls -t Fudge*.AppImage 2>/dev/null | head -1 || true)"
if [ -n "$produced" ] && [ "$produced" != "$LDAI_OUTPUT" ]; then mv "$produced" "$LDAI_OUTPUT"; fi

( cd "$OUT" && sha256sum ./*.AppImage | sed 's#\./##' > SHA256SUMS-appimage )
ls -lh "$OUT"/*.AppImage
echo "Run with: chmod +x $LDAI_OUTPUT && $LDAI_OUTPUT daemon"
