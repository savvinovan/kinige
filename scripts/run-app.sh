#!/usr/bin/env bash
# Installs dependencies (if needed) and runs the app in development mode: the Kinige window + Vite.
# The MCP bridge (`kinige-mcp`) is built and placed as a sidecar, so that the connection snippet in the "AI agents access" window
# points to a real file (in dev it sits next to the app executable: target/debug/kinige-mcp).
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/node_modules ] || npm --prefix web ci
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
cargo build -p kinige-mcp
mkdir -p src-tauri/binaries
install -m 755 "${CARGO_TARGET_DIR:-target}/debug/kinige-mcp-bridge" "src-tauri/binaries/kinige-mcp-${TRIPLE}"
cd src-tauri && exec ../web/node_modules/.bin/tauri dev
