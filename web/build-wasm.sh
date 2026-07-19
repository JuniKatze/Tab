#!/bin/bash
# Build the tab-engine WASM module and generate JS bindings.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENGINE_DIR="$SCRIPT_DIR/../engine"
PKG_DIR="$SCRIPT_DIR/pkg"

echo "Building tab-engine for wasm32-unknown-unknown..."
cd "$ENGINE_DIR"
cargo build --target wasm32-unknown-unknown --features wasm --release

echo "Generating JS bindings..."
mkdir -p "$PKG_DIR"
wasm-bindgen \
  --target web \
  --out-dir "$PKG_DIR" \
  --out-name tab_engine \
  "$ENGINE_DIR/target/wasm32-unknown-unknown/release/tab_engine.wasm"

echo "Done! WASM package ready at $PKG_DIR"
ls -lh "$PKG_DIR"
