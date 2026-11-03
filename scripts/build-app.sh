#!/usr/bin/env bash
# Release build of Kinige.app with an ad-hoc signature and a signature check (macOS).
# Developer ID, notarization and the updater are slice 10 (see ADR-0003).
# The MCP bridge (`kinige-mcp`, ADR-0009) is built separately and placed as a sidecar: bundle.externalBin → Contents/MacOS/kinige-mcp.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d web/node_modules ] || npm --prefix web ci

# Target triple and the bridge: tauri-build expects `binaries/kinige-mcp-<triple>` (the name without the triple is in the bundle).
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
cargo build --release -p kinige-mcp
mkdir -p src-tauri/binaries
# `install -m 755`, not `cp`: `cp` over the empty stub from build.rs keeps its 0644 mode, and the bridge in the bundle does not start.
install -m 755 "${CARGO_TARGET_DIR:-target}/release/kinige-mcp-bridge" "src-tauri/binaries/kinige-mcp-${TRIPLE}"

# signingIdentity "-" in tauri.conf.json = ad-hoc signature. `KINIGE_IDENTIFIER` is a build with another identifier for acceptance
# (separate data and a separate single-instance, the user's real app is not affected).
CONFIG_ARGS=()
if [ -n "${KINIGE_IDENTIFIER:-}" ]; then
  CONFIG_ARGS=(--config "{\"identifier\":\"${KINIGE_IDENTIFIER}\"}")
fi
# `KINIGE_BUNDLES` is what to build: `app` by default; the release (.github/workflows/release.yml) asks for `app,dmg`.
(cd src-tauri && ../web/node_modules/.bin/tauri build --bundles "${KINIGE_BUNDLES:-app}" ${CONFIG_ARGS[@]+"${CONFIG_ARGS[@]}"})

APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Kinige.app"
[ -d "$APP" ] || { echo "not found: $APP" >&2; exit 1; }
[ -x "$APP/Contents/MacOS/kinige-mcp" ] || { echo "the bundle has no kinige-mcp bridge" >&2; exit 1; }

# Bundle version = the version from Cargo.toml (ADR-0012): tauri.conf.json has no version of its own, Tauri takes it from the crate.
# `cargo pkgid` prints "...#kinige@0.5.0"; no network needed.
PKGID="$(cargo pkgid -p kinige --offline)"
VERSION="${PKGID##*[#@]}"
for KEY in CFBundleShortVersionString CFBundleVersion; do
  GOT="$(plutil -extract "$KEY" raw "$APP/Contents/Info.plist")"
  [ "$GOT" = "$VERSION" ] || { echo "$KEY in the bundle = $GOT, but Cargo.toml has $VERSION" >&2; exit 1; }
done

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature|Format"
echo "OK: $APP ($VERSION)"
