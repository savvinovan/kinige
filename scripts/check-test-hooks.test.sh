#!/usr/bin/env bash
# Controls of scripts/check-test-hooks.sh: it must fail on a build that holds a hook's marker (in web/dist or in the binary) and pass
# on one that does not. Run: bash scripts/check-test-hooks.test.sh (in CI, the web job).
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
GUARD="$REPO/scripts/check-test-hooks.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/clean" "$WORK/fake" "$WORK/driver"
echo 'console.log("a normal build")' > "$WORK/clean/index.js"
echo 'document.dataset.x="kinige-fake-microphone-7a"' > "$WORK/fake/index.js"
echo 'document.dataset.y="kinige-screens-driver-92"' > "$WORK/driver/index.js"
printf 'ELF\0\0 a normal binary \0 log::info\0' > "$WORK/clean.bin"
printf 'ELF\0\0 screens driver on \0kinige-screens-driver-92\0' > "$WORK/driver.bin"

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

unset KINIGE_ALLOW_FAKE_MIC KINIGE_ALLOW_SCREENS_DRIVER
# The fake microphone (slice 7a).
check "a clean build passes" pass "$GUARD" "$WORK/clean"
check "a clean build with a clean binary passes" pass "$GUARD" "$WORK/clean" --binary "$WORK/clean.bin"
check "a build with the fake microphone fails without the opt-in" fail "$GUARD" "$WORK/fake"
check "the opt-in lets the fake microphone through" pass env KINIGE_ALLOW_FAKE_MIC=1 "$GUARD" "$WORK/fake"
check "an opt-in other than 1 does not" fail env KINIGE_ALLOW_FAKE_MIC=yes "$GUARD" "$WORK/fake"
check "the driver's opt-in does not let the fake microphone through" fail env KINIGE_ALLOW_SCREENS_DRIVER=1 "$GUARD" "$WORK/fake"
check "the positive control passes on a build with the fake microphone" pass "$GUARD" "$WORK/fake" --expect fake-mic
check "the positive control fails on a clean build" fail "$GUARD" "$WORK/clean" --expect fake-mic
# The screens driver (KINIG-92): the web half, the Rust half, each by itself.
check "a build with the driver in web/dist fails without the opt-in" fail "$GUARD" "$WORK/driver"
check "the driver opt-in lets the web half through" pass env KINIGE_ALLOW_SCREENS_DRIVER=1 "$GUARD" "$WORK/driver"
check "the fake microphone's opt-in does not let the driver through" fail env KINIGE_ALLOW_FAKE_MIC=1 "$GUARD" "$WORK/driver"
check "an opt-in other than 1 does not (driver)" fail env KINIGE_ALLOW_SCREENS_DRIVER=yes "$GUARD" "$WORK/driver"
check "a driver in the binary alone fails without the opt-in" fail "$GUARD" "$WORK/clean" --binary "$WORK/driver.bin"
check "the opt-in lets the binary through" pass env KINIGE_ALLOW_SCREENS_DRIVER=1 "$GUARD" "$WORK/clean" --binary "$WORK/driver.bin"
check "the driver positive control passes with both halves" pass "$GUARD" "$WORK/driver" --binary "$WORK/driver.bin" --expect screens-driver
check "the driver positive control fails when the binary lacks it" fail "$GUARD" "$WORK/driver" --binary "$WORK/clean.bin" --expect screens-driver
check "the driver positive control fails when web/dist lacks it" fail "$GUARD" "$WORK/clean" --binary "$WORK/driver.bin" --expect screens-driver
check "the driver positive control fails on a clean build" fail "$GUARD" "$WORK/clean" --expect screens-driver
# Bad usage.
check "a missing directory fails" fail "$GUARD" "$WORK/nope"
check "a missing binary fails" fail "$GUARD" "$WORK/clean" --binary "$WORK/nope.bin"
check "an unknown hook fails" fail "$GUARD" "$WORK/clean" --expect nothing
check "no arguments fail" fail "$GUARD"

echo "problems: $PROBLEMS"
[ "$PROBLEMS" -eq 0 ]
