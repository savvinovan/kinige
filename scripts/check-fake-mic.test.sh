#!/usr/bin/env bash
# Controls of scripts/check-fake-mic.sh: it must fail on a build that holds the marker and pass on one that does not.
# Run: bash scripts/check-fake-mic.test.sh (in CI, the web job).
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
GUARD="$REPO/scripts/check-fake-mic.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/clean" "$WORK/fake"
echo 'console.log("a normal build")' > "$WORK/clean/index.js"
echo 'document.dataset.x="kinige-fake-microphone-7a"' > "$WORK/fake/index.js"

PROBLEMS=0
check() {
  local name="$1" want="$2"
  shift 2
  if "$@" > /dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name (wanted $want, got $got)"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

unset KINIGE_ALLOW_FAKE_MIC
check "a clean build passes" pass "$GUARD" "$WORK/clean"
check "a build with the marker fails without the opt-in" fail "$GUARD" "$WORK/fake"
check "the opt-in lets the marker through" pass env KINIGE_ALLOW_FAKE_MIC=1 "$GUARD" "$WORK/fake"
check "an opt-in other than 1 does not" fail env KINIGE_ALLOW_FAKE_MIC=yes "$GUARD" "$WORK/fake"
check "the positive control passes on a build with the marker" pass "$GUARD" "$WORK/fake" --expect
check "the positive control fails on a clean build" fail "$GUARD" "$WORK/clean" --expect
check "a missing directory fails" fail "$GUARD" "$WORK/nope"

echo "problems: $PROBLEMS"
[ "$PROBLEMS" -eq 0 ]
