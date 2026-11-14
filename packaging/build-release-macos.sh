#!/usr/bin/env bash
# Build the macOS (Apple Silicon) tarball in dist/. Run on macOS after `cargo build --release -p fudge`.
# Usage: packaging/build-release-macos.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
ARCH="$(uname -m)"; [ "$ARCH" = "arm64" ] || ARCH="x86_64"
NAME="fudge-${VERSION}-macos-${ARCH}"
OUT="dist"
BIN="target/release/fudge"

[ -x "$BIN" ] || cargo build --release -p fudge
mkdir -p "$OUT/$NAME"
install -m755 "$BIN" "$OUT/$NAME/fudge"
cp LICENSE-MIT LICENSE-APACHE README.md "$OUT/$NAME/"
cat > "$OUT/$NAME/install.sh" <<'INST'
#!/usr/bin/env sh
# Installs Fudge into ~/.local/bin. Run: ./install.sh
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
mkdir -p "$prefix/bin"
cp "$here/fudge" "$prefix/bin/fudge"
# The binary is not signed or notarized: remove the download quarantine flag.
xattr -d com.apple.quarantine "$prefix/bin/fudge" 2>/dev/null || true
echo "Installed to $prefix/bin/fudge. Make sure $prefix/bin is on your PATH."
echo "Fudge needs GTK 4 and libadwaita from Homebrew: brew install gtk4 libadwaita"
echo "Start it with: fudge daemon"
INST
chmod +x "$OUT/$NAME/install.sh"
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
rm -rf "$OUT/$NAME"
( cd "$OUT" && shasum -a 256 "$NAME.tar.gz" > "SHA256SUMS-macos-${ARCH}" )
ls -lh "$OUT"
