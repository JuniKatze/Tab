#!/bin/bash
# Tab Desktop App Launcher

set -e
DIR="$(cd "$(dirname "$0")" && pwd)"

export NVM_DIR="${NVM_DIR:-$HOME/.config/nvm}"
if [ -s "$NVM_DIR/nvm.sh" ] && [ -f "$DIR/.nvmrc" ]; then
  . "$NVM_DIR/nvm.sh"
  nvm use >/dev/null
fi

# Kill any stale server on port 1420 left over from a previous run
echo "Checking for stale server on port 1420..."
PID_ON_1420=$(lsof -ti tcp:1420 2>/dev/null || true)
if [ -n "$PID_ON_1420" ]; then
  echo "Killing stale server (PID $PID_ON_1420)..."
  kill "$PID_ON_1420" 2>/dev/null || true
  sleep 0.5
fi

echo "Building frontend..."
cd "$DIR"
npx vite build --logLevel error

# Clean stale assets from previous builds (Vite hashes change every build)
echo "Cleaning stale build artifacts..."
CURRENT_JS=$(grep -oP 'src="[^"]*\.js"' "$DIR/dist/index.html" | head -1 | grep -oP '/assets/[^"]+')
CURRENT_CSS=$(grep -oP 'href="[^"]*\.css"' "$DIR/dist/index.html" | head -1 | grep -oP '/assets/[^"]+')
for f in "$DIR/dist/assets"/*; do
  [ -f "$f" ] || continue
  name="/assets/$(basename "$f")"
  if [ "$name" != "$CURRENT_JS" ] && [ "$name" != "$CURRENT_CSS" ]; then
    rm -f "$f"
  fi
done

echo "Starting local server..."
cd "$DIR/dist"
python3 -c "
import http.server
import sys

class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cache-Control', 'no-cache, no-store, must-revalidate')
        self.send_header('Pragma', 'no-cache')
        self.send_header('Expires', '0')
        super().end_headers()

http.server.test(HandlerClass=NoCacheHandler, port=1420, bind='127.0.0.1')
" &
SERVER_PID=$!
sleep 1

echo "Launching Tab..."
cd "$DIR/src-tauri"
cargo run --release

# Cleanup when Tauri exits
kill $SERVER_PID 2>/dev/null
