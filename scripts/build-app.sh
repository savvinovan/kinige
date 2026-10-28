#!/usr/bin/env bash
# Release-сборка Kinige.app с ad-hoc подписью и проверкой подписи (macOS).
# Developer ID, нотаризация и updater — слайс 10 (см. ADR-0003).
# Мост MCP (`kinige-mcp`, ADR-0009) собирается отдельно и кладётся как sidecar: bundle.externalBin → Contents/MacOS/kinige-mcp.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d web/node_modules ] || npm --prefix web ci

# Целевая тройка и мост: `binaries/kinige-mcp-<triple>` ждёт tauri-build (имя без тройки — в бандле).
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
cargo build --release -p kinige-mcp
mkdir -p src-tauri/binaries
cp "${CARGO_TARGET_DIR:-target}/release/kinige-mcp-bridge" "src-tauri/binaries/kinige-mcp-${TRIPLE}"

# signingIdentity "-" в tauri.conf.json = ad-hoc подпись. `KINIGE_IDENTIFIER` — сборка с другим identifier для приёмки
# (отдельные данные и отдельный single-instance, настоящее приложение пользователя не затрагивается).
CONFIG_ARGS=()
if [ -n "${KINIGE_IDENTIFIER:-}" ]; then
  CONFIG_ARGS=(--config "{\"identifier\":\"${KINIGE_IDENTIFIER}\"}")
fi
(cd src-tauri && ../web/node_modules/.bin/tauri build --bundles app ${CONFIG_ARGS[@]+"${CONFIG_ARGS[@]}"})

APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Kinige.app"
[ -d "$APP" ] || { echo "не найден $APP" >&2; exit 1; }
[ -x "$APP/Contents/MacOS/kinige-mcp" ] || { echo "в бандле нет моста kinige-mcp" >&2; exit 1; }

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature|Format"
echo "OK: $APP"
