#!/usr/bin/env bash
# Ставит зависимости (если нужно) и запускает приложение в режиме разработки: окно Kinige + Vite.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/node_modules ] || npm --prefix web ci
cd src-tauri && exec ../web/node_modules/.bin/tauri dev
