#!/usr/bin/env bash
# The guard against shipping the screens runners' fake microphone (slice 7a, ADR-0015).
#   scripts/check-fake-mic.sh <dist directory>             fails if the fake microphone's marker is in the build, unless
#                                                          KINIGE_ALLOW_FAKE_MIC=1 (only screens.yml sets it)
#   scripts/check-fake-mic.sh <dist directory> --expect    fails if the marker is NOT in the build (the positive control of the
#                                                          screens scenario: a guard that never fired proves nothing)
# The marker is the string literal in scripts/screens/fake-mic.ts. The target is `web/dist`, not the .app: Tauri embeds the assets
# compressed, so searching the bundle would find nothing.
set -euo pipefail

MARKER="kinige-fake-microphone-7a"
DIST="${1:?usage: $0 <dist directory> [--expect]}"
[ -d "$DIST" ] || { echo "no such directory: $DIST" >&2; exit 2; }

if grep -rqF "$MARKER" "$DIST"; then
  FOUND=1
else
  FOUND=0
fi

if [ "${2:-}" = "--expect" ]; then
  [ "$FOUND" -eq 1 ] || { echo "the fake microphone is not in $DIST, but this build must have it" >&2; exit 1; }
  exit 0
fi

if [ "$FOUND" -eq 1 ] && [ "${KINIGE_ALLOW_FAKE_MIC:-}" != "1" ]; then
  echo "the fake microphone is in $DIST (KINIGE_FAKE_MIC=1 was set for this build?); refusing to build a bundle with it." >&2
  echo "Only the screens workflow may do this, with KINIGE_ALLOW_FAKE_MIC=1." >&2
  exit 1
fi
