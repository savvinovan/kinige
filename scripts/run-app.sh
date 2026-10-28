#!/usr/bin/env bash
# Ставит зависимости (если нужно) и запускает приложение в режиме разработки: окно Kinige + Vite.
# Мост MCP (`kinige-mcp`) собирается и кладётся как sidecar, чтобы фрагмент подключения в окне «Доступ ИИ-агентов» вёл на
# настоящий файл (в dev он лежит рядом с исполняемым файлом приложения: target/debug/kinige-mcp).
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/node_modules ] || npm --prefix web ci
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
cargo build -p kinige-mcp
mkdir -p src-tauri/binaries
cp "${CARGO_TARGET_DIR:-target}/debug/kinige-mcp-bridge" "src-tauri/binaries/kinige-mcp-${TRIPLE}"
cd src-tauri && exec ../web/node_modules/.bin/tauri dev
