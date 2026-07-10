#!/bin/bash
# Tab Desktop App Launcher

set -e
DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Building frontend..."
cd "$DIR"
npx vite build --logLevel error

echo "Starting local server..."
cd "$DIR/dist"
python3 -m http.server 1420 -b 127.0.0.1 &
SERVER_PID=$!
sleep 1

echo "Launching Tab..."
cd "$DIR/src-tauri"
cargo run --release

# Cleanup when Tauri exits
kill $SERVER_PID 2>/dev/null
