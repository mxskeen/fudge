#!/usr/bin/env bash
# Rebuild the in-browser demo engine (crates/fudge-compute -> WebAssembly).
# Needs: rustup, the wasm32 target, and wasm-bindgen-cli matching the
# wasm-bindgen version in Cargo.lock (currently 0.2.130).
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
cd "$root"
cargo build --locked -p fudge-compute --features wasm \
  --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir "$here/../public/wasm" \
  "${CARGO_TARGET_DIR:-$root/target}/wasm32-unknown-unknown/release/fudge_compute.wasm"
ls -la "$here/../public/wasm"
