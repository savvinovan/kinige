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
# The screens driver (KINIG-92, ADR-0016): the cargo feature goes together with the web half (`KINIGE_SCREENS_DRIVER=1`, read by
# `web/vite.config.ts` in the `beforeBuildCommand`). Off by default; `src-tauri/build.rs` refuses it without KINIGE_ALLOW_SCREENS_DRIVER=1.
FEATURE_ARGS=()
if [ "${KINIGE_SCREENS_DRIVER:-}" = "1" ]; then
  [ "${KINIGE_ALLOW_SCREENS_DRIVER:-}" = "1" ] || { echo "KINIGE_SCREENS_DRIVER=1 builds the screens driver into the app; refusing without KINIGE_ALLOW_SCREENS_DRIVER=1 (only screens.yml sets it)." >&2; exit 1; }
  FEATURE_ARGS=(--features screens-driver)
fi
# `KINIGE_BUNDLES` is what to build: `app` by default; the release (.github/workflows/release.yml) asks for `app,dmg`.
(cd src-tauri && ../web/node_modules/.bin/tauri build ${FEATURE_ARGS[@]+"${FEATURE_ARGS[@]}"} --bundles "${KINIGE_BUNDLES:-app}" ${CONFIG_ARGS[@]+"${CONFIG_ARGS[@]}"})

APP="${CARGO_TARGET_DIR:-target}/release/bundle/macos/Kinige.app"
# The screens runners' test hooks (the fake microphone, ADR-0015; the screens driver, ADR-0016) must never reach a bundle for people:
# refuse unless screens.yml opted in. The Rust half of the driver is searched in the executable.
scripts/check-test-hooks.sh web/dist --binary "$APP/Contents/MacOS/kinige"

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

# The microphone (slice 7a, ADR-0015): without the usage description macOS ends the process at the first recording, and without the
# entitlement the hardened runtime refuses the microphone. Nothing in CI reaches TCC, so a missing key is asserted here instead of
# being found by the person who presses the button.
MIC_TEXT="$(plutil -extract NSMicrophoneUsageDescription raw "$APP/Contents/Info.plist" 2>/dev/null || true)"
[ -n "$MIC_TEXT" ] || { echo "the bundle has no NSMicrophoneUsageDescription" >&2; exit 1; }

codesign --verify --deep --strict --verbose=2 "$APP"
codesign -d --entitlements :- "$APP" 2>&1 | grep -q "com.apple.security.device.audio-input" \
  || { echo "the signature does not carry com.apple.security.device.audio-input" >&2; codesign -d --entitlements :- "$APP" >&2 || true; exit 1; }
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature|Format"
echo "OK: $APP ($VERSION)"
