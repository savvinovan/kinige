#!/usr/bin/env bash
# A check of the lib.sh helpers without macOS: `shoot` must fail on an empty or cut-off window dump (KINIG-45), otherwise the personal
# path check silently checks nothing. Run: bash scripts/screens/lib.test.sh (in CI, the web job).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
K="$WORK/k"
OUT="$WORK/out"
APP="$WORK/Kinige.app"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# Stubs: the app "exists", the shot is "taken", the tree traversal answers with what DUMP_ANSWER holds.
app_pid() { echo 1; }
limit() { return 0; }
sleep() { :; }
ax() {
  case "$1" in
    dump) printf '%b' "$DUMP_ANSWER" ;;
    *) echo ok ;;
  esac
}

PROBLEMS=0
expect_fails() {
  local name="$1" want="$2"
  FAILS=0
  shoot "case" > /dev/null 2>&1
  if [ "$want" = "fail" ] && [ "$FAILS" -eq 0 ]; then
    echo "DOES NOT FAIL: $name"
    PROBLEMS=$((PROBLEMS + 1))
  elif [ "$want" = "pass" ] && [ "$FAILS" -ne 0 ]; then
    echo "FAILS WRONGLY: $name"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

DUMP_ANSWER='' expect_fails "empty dump" fail
DUMP_ANSWER='Alarm clock\n' expect_fails "cut off by the time limit" fail
DUMP_ANSWER='no process' expect_fails "no process" fail
DUMP_ANSWER='AXGroup Лента\n AXButton Отправить\n' expect_fails "real dump without personal paths" pass
DUMP_ANSWER='AXGroup /Users/someone/Documents\n' expect_fails "real dump with a foreign path" fail

[ "$PROBLEMS" -eq 0 ] && echo "lib.sh: checks passed"
exit "$PROBLEMS"
