#!/usr/bin/env bash
# A check of the lib.sh helpers without macOS: `shoot` must fail on an empty or cut-off window dump (KINIG-45), otherwise the personal
# path check silently checks nothing; and the screens driver's shell side (KINIG-92) against a fake app that answers request files.
# Run: bash scripts/screens/lib.test.sh (in CI, the web job).
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

# ---- The screens driver (KINIG-92): `drive`, `press`, `has`, `wait_name`, `shoot` against a fake app ----
unset -f sleep
DRIVER=1
DRIVE_DIR="$WORK/driver"
DRIVE_TIMEOUT=2
PAUSE=0
mkdir -p "$DRIVE_DIR/requests" "$DRIVE_DIR/replies" "$K/debug" "$OUT"
printf 'The feed says hello' > "$WORK/text-answer"

# The fake app: answers every request file by what the step says; "Silent" gets no answer, "Missing" an error.
fake_app() {
  local f id body
  while :; do
    for f in "$DRIVE_DIR"/requests/*.json; do
      [ -f "$f" ] || continue
      id="$(basename "$f" .json)"
      body="$(cat "$f")"
      rm -f "$f"
      printf '%s\n' "$body" >> "$WORK/seen"
      case "$body" in
        *Silent*) continue ;;
        *Missing*) printf '{"id":"%s","ok":false,"error":"not found: Missing"}' "$id" ;;
        *'"kind":"text"'*) printf '{"id":"%s","ok":true,"value":"%s"}' "$id" "$(cat "$WORK/text-answer")" ;;
        *'"kind":"has"'*'"name":"Here"'*) printf '{"id":"%s","ok":true,"value":"yes"}' "$id" ;;
        *'"kind":"has"'*) printf '{"id":"%s","ok":true,"value":"no"}' "$id" ;;
        *) printf '{"id":"%s","ok":true}' "$id" ;;
      esac > "$DRIVE_DIR/replies/$id.json"
    done
    sleep 0.02
  done
}
fake_app &
FAKE_APP=$!
disown
trap 'kill "$FAKE_APP" 2> /dev/null; rm -rf "$WORK"' EXIT

expect() {
  local name="$1" want="$2"
  shift 2
  FAILS=0
  "$@" > "$WORK/last-output" 2>&1
  if [ "$want" = "fail" ] && [ "$FAILS" -eq 0 ]; then
    echo "DOES NOT FAIL: $name"
    PROBLEMS=$((PROBLEMS + 1))
  elif [ "$want" = "pass" ] && [ "$FAILS" -ne 0 ]; then
    echo "FAILS WRONGLY: $name: $(cat "$WORK/last-output")"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}
expect_seen() {
  if ! grep -qF -- "$2" "$WORK/seen"; then
    echo "NOT SENT: $1 (wanted $2)"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

AX_WINDOW=Kinige expect "a press that is found" pass press "Play"
expect_seen "press goes to the main window" '"window":"main","step":{"kind":"press","name":"Play"}'
AX_WINDOW=Settings expect "a press in the settings window" pass press "Dark" RadioButton
expect_seen "press in settings, with the role mapped" '"window":"settings","step":{"kind":"press","name":"Dark","role":"radio"}'
expect "a press that finds nothing is a scenario error" fail press "Missing"
expect "a step nobody answers is a scenario error" fail press "Silent"
expect "show_menu" pass show_menu "Inbox"
expect_seen "show_menu is a menu step" '"kind":"menu","name":"Inbox"'
expect "scroll_to" pass scroll_to "Row 5"
expect_seen "scroll_to is a scroll step" '"kind":"scroll","name":"Row 5"'
FAILS=0
has "Here" || { echo "has: yes was not seen"; PROBLEMS=$((PROBLEMS + 1)); }
has "Elsewhere" && { echo "has: no was not seen"; PROBLEMS=$((PROBLEMS + 1)); }
wait_name "Here" 3000 || { echo "wait_name: not answered"; PROBLEMS=$((PROBLEMS + 1)); }
expect_seen "wait is sent with its time" '"kind":"wait","name":"Here","timeoutMs":3000'
wait_name "Gone" 3000 gone Button || { echo "wait_name gone: not answered"; PROBLEMS=$((PROBLEMS + 1)); }
expect_seen "wait gone with a role" '"kind":"wait","name":"Gone","role":"button","gone":true,"timeoutMs":3000'

printf 'The feed says hello' > "$WORK/text-answer"
expect "a shot of clean text" pass shoot "driver-clean"
printf '/Users/someone/Documents/journal' > "$WORK/text-answer"
expect "a shot of text with a foreign path" fail shoot "driver-private"
printf '' > "$WORK/text-answer"
expect "a shot of an empty text" fail shoot "driver-empty"
printf 'The feed says hello' > "$WORK/text-answer"
[ "$(json_escape 'a "b" \c')" = 'a \"b\" \\c' ] || { echo "json_escape is wrong"; PROBLEMS=$((PROBLEMS + 1)); }
FAKE_ESCAPED="$(json_escape 'Say "hi"')"
FAILS=0
press 'Say "hi"' > /dev/null 2>&1
expect_seen "a name with quotes is escaped" "\"name\":\"$FAKE_ESCAPED\""

[ "$PROBLEMS" -eq 0 ] && echo "lib.sh: checks passed"
exit "$PROBLEMS"
