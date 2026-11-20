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
AX_WINDOW=Capture expect "a press in the capture window" pass press "Record voice note"
expect_seen "press in the quick capture window" '"window":"capture","step":{"kind":"press","name":"Record voice note"}'
# KINIG-153: the shot of the capture window never sends `front` (it would show the panel as an ordinary window and take the key from the
# main one); the text for the personal path check still comes from the capture window.
: > "$WORK/seen"
printf 'A note in the capture window' > "$WORK/text-answer"
AX_WINDOW=Capture expect "a shot of the capture window" pass shoot "capture-shot" capture
expect_seen "the capture shot reads the capture window's text" '"window":"capture","step":{"kind":"text"}'
if grep -qF '"kind":"front"' "$WORK/seen"; then
  echo "THE CAPTURE SHOT SENT front"
  PROBLEMS=$((PROBLEMS + 1))
fi
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
# Subshells (KINIG-92 review): `$(has ...)` runs in a child shell, where `$$` is the parent's and a counter's increment is lost. Each step
# must still get its own id, hence its own answer, and no reply file may be left for a later step to find.
FIRST="$(has "Here" && echo yes || echo no)"
SECOND="$(has "Elsewhere" && echo yes || echo no)"
THIRD="$(has "Here" && echo yes || echo no)"
[ "$FIRST $SECOND $THIRD" = "yes no yes" ] || { echo "subshell ids: got '$FIRST $SECOND $THIRD', wanted 'yes no yes'"; PROBLEMS=$((PROBLEMS + 1)); }
[ "$(find "$DRIVE_DIR/ids" -type f | wc -l | tr -d ' ')" = "0" ] || { echo "reserved ids are left behind"; PROBLEMS=$((PROBLEMS + 1)); }
AX_WINDOW=Kinige expect "a press after the subshell steps does not read an old reply" fail press "Missing"
expect "a press after that finds its own reply" pass press "Play"
[ -z "$(ls "$DRIVE_DIR/replies")" ] || { echo "reply files are left behind: $(ls "$DRIVE_DIR/replies")"; PROBLEMS=$((PROBLEMS + 1)); }

[ "$(json_escape 'a "b" \c')" = 'a \"b\" \\c' ] || { echo "json_escape is wrong"; PROBLEMS=$((PROBLEMS + 1)); }
FAKE_ESCAPED="$(json_escape 'Say "hi"')"
FAILS=0
press 'Say "hi"' > /dev/null 2>&1
expect_seen "a name with quotes is escaped" "\"name\":\"$FAKE_ESCAPED\""

# ---- set_text_size (KINIG-160): the text size through the settings window, in a scenario whose own input is AX (DRIVER unset) ----
sql() { printf '%s' "$STORED"; }
: > "$WORK/seen"
(DRIVER="" STORED=large; FAILS=0; set_text_size large > "$WORK/last-output" 2>&1; exit "$FAILS") || { echo "set_text_size large FAILS WRONGLY: $(cat "$WORK/last-output")"; PROBLEMS=$((PROBLEMS + 1)); }
expect_seen "the profile menu is opened in the main window" '"window":"main","step":{"kind":"press","name":"Profile"}'
expect_seen "Settings is pressed in the main window" '"window":"main","step":{"kind":"press","name":"Settings"}'
expect_seen "the settings window is waited for" '"window":"settings","step":{"kind":"wait","name":"Appearance","timeoutMs":2000}'
expect_seen "the Appearance page is opened (the window may be on another page)" '"window":"settings","step":{"kind":"press","name":"Appearance","role":"button"}'
expect_seen "the radio button is waited for on that page" '"window":"settings","step":{"kind":"wait","name":"Large","role":"radio","timeoutMs":4000}'
expect_seen "the radio button is pressed in the settings window" '"window":"settings","step":{"kind":"press","name":"Large","role":"radio"}'
[ "$(grep -c '"kind"' "$WORK/seen")" = "7" ] || { echo "set_text_size sent other than seven steps: $(cat "$WORK/seen")"; PROBLEMS=$((PROBLEMS + 1)); }
# The last step puts the main window in front (an AX call without AX_WINDOW acts on the frontmost window), on a failure too.
expect_last_step() {
  if [ "$(tail -n 1 "$WORK/seen" | sed -e 's/^{"id":"[A-Za-z0-9]*",//')" != '"window":"main","step":{"kind":"front"}}' ]; then
    echo "NOT LAST: $1 (last step: $(tail -n 1 "$WORK/seen"))"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}
expect_last_step "the main window is put in front at the end"
: > "$WORK/seen"
(STORED="normal"; FAILS=0; set_text_size large > /dev/null 2>&1; exit "$FAILS") > /dev/null
expect_last_step "the main window is put in front after a failed check"
: > "$WORK/seen"
(STORED="large"; FAILS=0; set_text_size huge > /dev/null 2>&1; exit "$FAILS") > /dev/null
expect_last_step "the main window is put in front after a refused size"
: > "$WORK/seen"
(DRIVER="" STORED=large; FAILS=0; set_text_size large > /dev/null 2>&1)
if grep -E '"kind":"(type|key|hide)"' "$WORK/seen" > /dev/null; then
  echo "set_text_size sent a step that is not a click"
  PROBLEMS=$((PROBLEMS + 1))
fi
if grep -iE 'agent|access|доступ|Allow' "$WORK/seen" > /dev/null; then
  echo "set_text_size touched agent access"
  PROBLEMS=$((PROBLEMS + 1))
fi
# "Normal" is the default: a database that never held a size passes; any other value for either size is a scenario error.
(STORED=""; FAILS=0; set_text_size normal > /dev/null 2>&1; exit "$FAILS") || { echo "set_text_size normal FAILS WRONGLY on an empty database"; PROBLEMS=$((PROBLEMS + 1)); }
(STORED="normal"; FAILS=0; set_text_size large > /dev/null 2>&1; exit "$FAILS") && { echo "DOES NOT FAIL: the database stayed normal after 'large'"; PROBLEMS=$((PROBLEMS + 1)); }
(STORED=""; FAILS=0; set_text_size large > /dev/null 2>&1; exit "$FAILS") && { echo "DOES NOT FAIL: an empty database after 'large'"; PROBLEMS=$((PROBLEMS + 1)); }
(STORED="large"; FAILS=0; set_text_size huge > /dev/null 2>&1; exit "$FAILS") && { echo "DOES NOT FAIL: an unknown size"; PROBLEMS=$((PROBLEMS + 1)); }

[ "$PROBLEMS" -eq 0 ] && echo "lib.sh: checks passed"
exit "$PROBLEMS"
