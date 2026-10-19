#!/usr/bin/env bash
# Release-сборка Kinige.app с ad-hoc подписью и проверкой подписи (macOS).
# Developer ID, нотаризация и updater — слайс 10 (см. ADR-0003).
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d web/node_modules ] || npm --prefix web ci

# signingIdentity "-" в tauri.conf.json = ad-hoc подпись.
(cd src-tauri && ../web/node_modules/.bin/tauri build --bundles app)

APP="target/release/bundle/macos/Kinige.app"
[ -d "$APP" ] || { echo "не найден $APP" >&2; exit 1; }

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature|Format"
echo "OK: $APP"
