#!/usr/bin/env bash
# Release build of the unsigned NSIS installer for Windows (slice 10a, ADR-0012). Run under Git Bash on a Windows runner
# (the release and screens workflows) or any shell with node, cargo and the MSVC toolchain on Windows.
# Authenticode signing and the updater come in slice 10 (ADR-0003): SmartScreen warns until then.
# The MCP bridge is not built or bundled on Windows: `src-tauri/tauri.windows.conf.json` empties `bundle.externalBin`.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d web/node_modules ] || npm --prefix web ci

# The screens driver (KINIG-92, ADR-0016): the cargo feature goes together with the web half (`KINIGE_SCREENS_DRIVER=1`). Off by default.
FEATURE_ARGS=()
if [ "${KINIGE_SCREENS_DRIVER:-}" = "1" ]; then
  [ "${KINIGE_ALLOW_SCREENS_DRIVER:-}" = "1" ] || { echo "KINIGE_SCREENS_DRIVER=1 builds the screens driver into the app; refusing without KINIGE_ALLOW_SCREENS_DRIVER=1 (only screens.yml sets it)." >&2; exit 1; }
  FEATURE_ARGS=(--features screens-driver)
fi
(cd src-tauri && ../web/node_modules/.bin/tauri build ${FEATURE_ARGS[@]+"${FEATURE_ARGS[@]}"} --bundles nsis)

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
EXE="$TARGET_DIR/release/kinige.exe"
[ -f "$EXE" ] || { echo "not found: $EXE" >&2; exit 1; }

# The screens runners' test hooks (the fake microphone, ADR-0015; the screens driver, ADR-0016) must never reach a bundle for people:
# refuse unless screens.yml opted in.
scripts/check-test-hooks.sh web/dist --binary "$EXE"

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

# The MCP bridge (`kinige-mcp-bridge` as built by cargo, `kinige-mcp` as bundled on macOS) must not be in the installer.
# NSIS installers are archives 7-Zip can list (7-Zip is on the GitHub Windows runners).
if command -v 7z >/dev/null 2>&1; then
  if 7z l "${INSTALLERS[0]}" | grep -qi 'kinige-mcp'; then
    echo "the MCP bridge is inside the installer" >&2
    exit 1
  fi
else
  echo "7z not found: the installer contents were not checked for the MCP bridge"
fi

# The file name of a release (ADR-0012): Kinige-X.Y.Z-windows-x64-setup.exe instead of the bundler's Kinige_X.Y.Z_x64-setup.exe.
RELEASE_NAME="Kinige-${VERSION}-windows-x64-setup.exe"
mv "${INSTALLERS[0]}" "$(dirname "${INSTALLERS[0]}")/$RELEASE_NAME"
echo "OK: $(dirname "${INSTALLERS[0]}")/$RELEASE_NAME ($VERSION)"
