#!/usr/bin/env bash
# The guard against shipping the screens runners' test hooks (slice 7a, ADR-0015; the screens driver, KINIG-92, ADR-0016).
#   scripts/check-test-hooks.sh <dist> [--binary <executable>]
#       fails if a hook's marker is in the web build (and, with --binary, in the app executable), unless that hook's allow
#       variable is 1 (only screens.yml sets them)
#   scripts/check-test-hooks.sh <dist> [--binary <executable>] --expect <hook>
#       fails if the marker of that hook is NOT there (the positive control of a screens scenario: a guard that never fired
#       proves nothing)
# Hooks and their allow variables:
#   fake-mic         kinige-fake-microphone-7a  in web/dist (scripts/screens/fake-mic.ts)   KINIGE_ALLOW_FAKE_MIC=1
#   screens-driver   kinige-screens-driver-92   in web/dist (scripts/screens/driver.ts) and in the Rust binary
#                    (src-tauri/src/screens_driver)                                         KINIGE_ALLOW_SCREENS_DRIVER=1
# The web target is `web/dist`, not the .app: Tauri embeds the assets compressed, so searching the bundle would find nothing.
# The Rust binary is searched as it is: string literals are stored as they are, also with `strip = true`.
set -euo pipefail

FAKE_MIC_MARKER="kinige-fake-microphone-7a"
DRIVER_MARKER="kinige-screens-driver-92"

usage() { echo "usage: $0 <dist directory> [--binary <executable>] [--expect fake-mic|screens-driver]" >&2; exit 2; }

DIST="${1:-}"
[ -n "$DIST" ] || usage
shift
BINARY=""
EXPECT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; [ -n "$BINARY" ] || usage; shift 2 ;;
    --expect) EXPECT="${2:-}"; [ -n "$EXPECT" ] || usage; shift 2 ;;
    *) usage ;;
  esac
done
[ -d "$DIST" ] || { echo "no such directory: $DIST" >&2; exit 2; }
[ -z "$BINARY" ] || [ -f "$BINARY" ] || { echo "no such file: $BINARY" >&2; exit 2; }

in_dist() { grep -rqF "$1" "$DIST"; }
in_binary() { [ -z "$BINARY" ] || grep -aqF "$1" "$BINARY"; }

if [ -n "$EXPECT" ]; then
  case "$EXPECT" in
    fake-mic)
      in_dist "$FAKE_MIC_MARKER" || { echo "the fake microphone is not in $DIST, but this build must have it" >&2; exit 1; }
      ;;
    screens-driver)
      in_dist "$DRIVER_MARKER" || { echo "the screens driver is not in $DIST, but this build must have it" >&2; exit 1; }
      if [ -n "$BINARY" ]; then
        in_binary "$DRIVER_MARKER" || { echo "the screens driver is not in $BINARY, but this build must have it" >&2; exit 1; }
      fi
      ;;
    *) usage ;;
  esac
  exit 0
fi

PROBLEMS=0
if in_dist "$FAKE_MIC_MARKER" && [ "${KINIGE_ALLOW_FAKE_MIC:-}" != "1" ]; then
  echo "the fake microphone is in $DIST (KINIGE_FAKE_MIC=1 was set for this build?); refusing to build a bundle with it." >&2
  echo "Only the screens workflow may do this, with KINIGE_ALLOW_FAKE_MIC=1." >&2
  PROBLEMS=$((PROBLEMS + 1))
fi
if [ "${KINIGE_ALLOW_SCREENS_DRIVER:-}" != "1" ]; then
  if in_dist "$DRIVER_MARKER"; then
    echo "the screens driver is in $DIST (KINIGE_SCREENS_DRIVER=1 was set for this build?); refusing to build a bundle with it." >&2
    PROBLEMS=$((PROBLEMS + 1))
  fi
  if [ -n "$BINARY" ] && grep -aqF "$DRIVER_MARKER" "$BINARY"; then
    echo "the screens driver is in $BINARY (the screens-driver cargo feature was on?); refusing to build a bundle with it." >&2
    PROBLEMS=$((PROBLEMS + 1))
  fi
  if [ "$PROBLEMS" -gt 0 ]; then echo "Only the screens workflow may do this, with KINIGE_ALLOW_SCREENS_DRIVER=1." >&2; fi
fi
[ "$PROBLEMS" -eq 0 ]
