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
# `install -m 755`, а не `cp`: `cp` поверх пустой заглушки из build.rs оставляет её права 0644, и мост в бандле не запускается.
install -m 755 "${CARGO_TARGET_DIR:-target}/release/kinige-mcp-bridge" "src-tauri/binaries/kinige-mcp-${TRIPLE}"

# signingIdentity "-" в tauri.conf.json = ad-hoc подпись. `KINIGE_IDENTIFIER` — сборка с другим identifier для приёмки
# (отдельные данные и отдельный single-instance, настоящее приложение пользователя не затрагивается).
CONFIG_ARGS=()
if [ -n "${KINIGE_IDENTIFIER:-}" ]; then
  CONFIG_ARGS=(--config "{\"identifier\":\"${KINIGE_IDENTIFIER}\"}")
fi
# `KINIGE_BUNDLES` — что собирать: по умолчанию `app`; релиз (.github/workflows/release.yml) просит `app,dmg`.
(cd src-tauri && ../web/node_modules/.bin/tauri build --bundles "${KINIGE_BUNDLES:-app}" ${CONFIG_ARGS[@]+"${CONFIG_ARGS[@]}"})

APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Kinige.app"
[ -d "$APP" ] || { echo "не найден $APP" >&2; exit 1; }
[ -x "$APP/Contents/MacOS/kinige-mcp" ] || { echo "в бандле нет моста kinige-mcp" >&2; exit 1; }

# Версия бандла = версия из Cargo.toml (ADR-0012): tauri.conf.json своей версии не имеет, Tauri берёт её из крейта.
# `cargo pkgid` выдаёт «…#kinige@0.5.0»; сеть не нужна.
PKGID="$(cargo pkgid -p kinige --offline)"
VERSION="${PKGID##*[#@]}"
for KEY in CFBundleShortVersionString CFBundleVersion; do
  GOT="$(plutil -extract "$KEY" raw "$APP/Contents/Info.plist")"
  [ "$GOT" = "$VERSION" ] || { echo "$KEY в бандле = $GOT, а в Cargo.toml $VERSION" >&2; exit 1; }
done

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature|Format"
echo "OK: $APP ($VERSION)"
