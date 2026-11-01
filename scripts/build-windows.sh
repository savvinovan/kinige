#!/usr/bin/env bash
# Release build of the unsigned NSIS installer for Windows (slice 10a, ADR-0012). Run under Git Bash on a Windows runner
# (the release and screens workflows) or any shell with node, cargo and the MSVC toolchain on Windows.
# Authenticode signing and the updater come in slice 10 (ADR-0003): SmartScreen warns until then.
# The MCP bridge is not built or bundled on Windows: `src-tauri/tauri.windows.conf.json` empties `bundle.externalBin`.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d web/node_modules ] || npm --prefix web ci

(cd src-tauri && ../web/node_modules/.bin/tauri build --bundles nsis)

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
EXE="$TARGET_DIR/release/kinige.exe"
[ -f "$EXE" ] || { echo "not found: $EXE" >&2; exit 1; }
[ ! -e "$TARGET_DIR/release/kinige-mcp.exe" ] || { echo "the MCP bridge must not be part of the Windows build" >&2; exit 1; }

# The version of the build = the version in Cargo.toml (ADR-0012): tauri-build writes it into the exe resources and the
# bundler into the installer name. `cargo pkgid` prints "…#kinige@0.5.0"; no network needed.
PKGID="$(cargo pkgid -p kinige --offline)"
VERSION="${PKGID##*[#@]}"

shopt -s nullglob
INSTALLERS=("$TARGET_DIR"/release/bundle/nsis/*-setup.exe)
[ "${#INSTALLERS[@]}" -eq 1 ] || { echo "expected one installer, found ${#INSTALLERS[@]}" >&2; exit 1; }
case "$(basename "${INSTALLERS[0]}")" in
  *"_${VERSION}_"*) ;;
  *) echo "installer name ${INSTALLERS[0]} does not carry version $VERSION" >&2; exit 1 ;;
esac

GOT="$(powershell.exe -NoProfile -Command "(Get-Item -LiteralPath '$EXE').VersionInfo.ProductVersion" | tr -d '\r')"
[ "$GOT" = "$VERSION" ] || { echo "ProductVersion in kinige.exe = $GOT, Cargo.toml says $VERSION" >&2; exit 1; }
echo "OK: ${INSTALLERS[0]} ($VERSION)"
