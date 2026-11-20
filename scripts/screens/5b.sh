#!/usr/bin/env bash
# Screens of the small fixes batch (slice 5b: KINIG-150 the chat name in the top bar and the forest that comes back as it was, KINIG-156 closing the
# main window stops a recording, KINIG-76 no page scroll bar) in the real windows on a macOS runner: scripts/screens/5b.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=5b
# The app is built with the fake microphone and the screens driver (screens.yml; the same guard as slice 7a keeps both out of a build for people).
# The data are synthetic (seed_demo with KINIGE_SEED_FOREST=1) in a temporary directory. Scroll bars are always shown (a mouse, "Always show scroll
# bars"), so a page-level bar would be on the picture. Input is the screens driver; the window is resized with accessibility (ax.js). The main window
# is closed by the driver's `close` step: the window's own close request, which the red button and ⌘W make (the first idea, a click on the red
# button through System Events, would hang while a recording runs). So the native button itself is NOT covered here. A real microphone and the system's
# microphone indicator are NOT covered either: the fake microphone runs the real MediaRecorder, so what is proved is that the recorder finishes in a
# window that has just been hidden and that the capture window can then record. The fake microphone ends a recording by itself after 14 seconds, so
# each recording below is planned to be over within about ten.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5b}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=30

SETTINGS_FILE="$K/data/settings.json"
L_INBOX="Inbox"; L_TOGGLE="Forest"; L_EXPAND="Expand forest"; L_COLLAPSE="Collapse forest"; L_PROJECTS="Проекты"; L_LONG="Очень длинное"
L_MIC="Record voice note"; L_RECORDING="Recording,"; L_STOPPED="Recording stopped"; L_SEND_VOICE="Send the voice note"; L_TRASH_REC="Discard the recording"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"

# Positive controls: this build must carry the fake microphone and the driver, or the guard proved nothing.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --expect fake-mic || { echo "ERROR: the fake microphone is not in web/dist"; FAILS=$((FAILS + 1)); }
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }

KINIGE_SEED_FOREST=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
# The longest name a chat may have (128 characters, `chat_title_too_long` above it): one chat is renamed in the database before the app starts.
LONG_TITLE="$(python3 -c 'print(("Очень длинное название чата, которое не помещается в строку. " * 4)[:128])')"
sqlite3 "$K/data/kinige.db" "update chat set title = '${LONG_TITLE}' where title = 'Мама'"
# Scroll bars are always shown on this disposable runner; the app starts after this.
defaults write NSGlobalDomain AppleShowScrollBars -string Always
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }
setting() { sqlw "select coalesce(value, '') from app_setting where key = '$1'"; }
voice_in_chat() {
  sqlw "select count(*) from attachment a join message m on m.id = a.message_id join chat c on c.id = m.chat_id where a.filename like 'voice-%' and c.title = '$1' and m.deleted_at is null"
}

edit_settings() {
  python3 - "$SETTINGS_FILE" "$1" <<'PY'
import json, os, sys
path, code = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as f:
    d = json.load(f)
exec(code)
tmp = os.path.join(os.path.dirname(path), ".editor.tmp")
with open(tmp, "w", encoding="utf-8") as f:
    json.dump(d, f, ensure_ascii=False, indent=2)
    f.write("\n")
os.chmod(tmp, 0o600)
os.replace(tmp, path)
PY
  sleep 4
}

# Whether the capture window is on screen: a window above the normal layer, 500 to 700 px wide (the icon's menu is narrower).
capture_present() {
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2> /dev/null |
    awk -F'\t' '{ split($2, l, "="); split($3, s, "x"); if (l[2] + 0 > 0 && s[1] + 0 >= 500 && s[1] + 0 <= 700) found = 1 } END { exit found ? 0 : 1 }'
}
wait_capture_hidden() {
  for _ in $(seq 1 15); do
    capture_present || return 0
    sleep 1
  done
  return 1
}
# Whether the main window is on screen: a window on the normal layer (the capture window is above it, the status item's menu is not a window).
main_present() { KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2> /dev/null | grep -q 'layer=0'; }
wait_main_hidden() {
  for _ in $(seq 1 15); do
    main_present || return 0
    sleep 1
  done
  return 1
}

show_capture() {
  local attempt
  for attempt in 1 2 3; do
    drive_raw capture '{"kind":"front"}' > /dev/null
    for _ in $(seq 1 10); do
      if capture_present; then
        sleep 1
        in_capture
        wait_name "${L_MIC}" 10000 || wait_name "${L_RECORDING}" 10000 || fail "the capture window has neither the microphone button nor a recording bar"
        return 0
      fi
      sleep 1
    done
  done
  fail "the capture window did not show"
  return 1
}

# The document's sizes (the driver's `metrics` step), appended to scroll.txt with the stage's name; the page must not scroll in either direction.
check_page_scroll() {
  local stage="$1" values name
  in_main
  if ! drive_raw main '{"kind":"metrics"}' 15; then
    fail "no page metrics at ${stage}: ${DRIVE_ERROR}"
    return 0
  fi
  values="$(drive_value)"
  { echo "== ${stage}"; printf '%s\n' "$values"; } >> "$OUT/scroll.txt"
  val() { printf '%s\n' "$values" | sed -n "s/^$1=//p"; }
  [ -n "$(val document.scrollHeight)" ] || fail "the metrics at ${stage} have no document.scrollHeight"
  [ "$(val document.scrollHeight)" -le "$(val document.clientHeight)" ] || fail "the page scrolls vertically at ${stage} ($(val document.scrollHeight) > $(val document.clientHeight))"
  [ "$(val document.scrollWidth)" -le "$(val document.clientWidth)" ] || fail "the page scrolls sideways at ${stage} ($(val document.scrollWidth) > $(val document.clientWidth))"
}

restart_app() {
  kill -9 "$(app_pid)"
  sleep 2
  launch_window
  wait_for_window
  in_main
  wait_name "${L_INBOX}" 60000 > /dev/null 2>&1 || true
  sleep 3
}

resize() {
  echo "window: $(ax resize "$(app_pid)" "$1" "$2")" | tee -a "$OUT/scroll.txt"
  sleep 3
}

toggle() { in_main; press "${L_TOGGLE}" Button; sleep 1; }
forest_gone() { ! has "${L_EXPAND}" && ! has "${L_COLLAPSE}"; }

# ---- 0. The app, in English, the Inbox with its name in the bar and the strip below ----
set_language en
launch_window
wait_for_window
in_main
wait_name "${L_INBOX}" 120000 || fail "the main window did not show the Inbox"
sleep 3
has "${L_EXPAND}" || fail "no strip at the start"
drive_raw main '{"kind":"text"}' 15 && drive_value > "$K/debug/inbox-text.txt"
shoot 01-inbox-title-strip
check_page_scroll "1280x800 normal text, the strip"

# ---- 1. A 128-character chat name in the smallest window: one line, the switch does not move ----
press "${L_LONG}"
sleep 2
resize 640 400
shoot 02-long-title-640
check_page_scroll "640x400 normal text, the long name, the strip"
resize 1280 800
press "${L_INBOX}"
sleep 2

# ---- 2. Large text at 1280 x 800 ----
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot 03-large-text-1280
check_page_scroll "1280x800 large text, the strip"
edit_settings 'd["text_size"] = "normal"'

# ---- 3. The board is hidden and comes back as the board, also after kill -9 ----
press "${L_EXPAND}"
has "${L_COLLAPSE}" || fail "the board did not open"
[ "$(setting forest_view)" = "expanded" ] || fail "forest_view is not 'expanded' after the board opened"
toggle
forest_gone || fail "the forest is still on screen after the switch"
[ "$(setting forest_view)" = "hidden" ] || fail "forest_view is not 'hidden' after the switch"
[ "$(setting forest_shown_view)" = "expanded" ] || fail "the remembered view is not 'expanded' after hiding the board"
shoot 04-board-hidden
restart_app
forest_gone || fail "the forest came back after kill -9 although it was hidden"
[ "$(setting forest_shown_view)" = "expanded" ] || fail "the remembered view did not survive kill -9"
toggle
has "${L_COLLAPSE}" || fail "one press after kill -9 did not bring the board back"
shoot 05-after-kill9-board-back
# The strip comes back as the strip.
press "${L_COLLAPSE}"
has "${L_EXPAND}" || fail "the board did not collapse to the strip"
toggle
toggle
has "${L_EXPAND}" || fail "hide and show of the strip did not bring the strip back"
[ "$(setting forest_shown_view)" = "strip" ] || fail "the remembered view is not 'strip'"
# The board again, then hidden: the rest of the scenario runs without the forest in the way.
press "${L_EXPAND}"
toggle
forest_gone || fail "the forest did not hide"

# ---- 4. Recording in a chat, closing the main window: the recorder stops, the capture window can record ----
press "${L_PROJECTS}"
sleep 2
BEFORE_NOTES="$(voice_in_chat "${L_PROJECTS}")"
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the recording bar did not show in the main window"
sleep 2
shoot 06-recording-in-main-window
drive main '{"kind":"close"}'
wait_main_hidden || fail "the main window did not hide on the close request"
# The microphone is let go: the capture window records at once (before the fix: "The microphone is unavailable").
show_capture
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the capture window cannot record after the main window was closed mid-recording"
sleep 1
in_capture
shoot 07-capture-recording-main-closed capture
press "${L_TRASH_REC}"
# Under five seconds the trash button drops a recording at once; the shot took a few, so the question may come first.
if has "Discard the recording?"; then press "Discard" Button; fi
wait_name "${L_RECORDING}" 5000 gone || fail "the capture recording was not dropped"
drive_raw capture '{"kind":"hide"}' > /dev/null
wait_capture_hidden || fail "the capture window did not hide"
[ "$(voice_in_chat "${L_PROJECTS}")" = "$BEFORE_NOTES" ] || fail "closing the window saved the recording"

# ---- 5. The main window comes back: "Recording stopped", then Send saves the note in its chat ----
drive_raw main '{"kind":"front"}' > /dev/null
wait_for_window
in_main
wait_name "${L_STOPPED}" 20000 || fail "the main window does not show the kept recording"
sleep 1
shoot 08-main-reopened-recording-stopped
grep -qE "Recording stopped · 0:(0[1-9]|1[0-3])" "$K/debug/shot-08-main-reopened-recording-stopped.txt" || fail "the kept recording has no time between one and thirteen seconds"
press "${L_SEND_VOICE}"
for _ in $(seq 1 20); do
  [ "$(voice_in_chat "${L_PROJECTS}")" = "$((BEFORE_NOTES + 1))" ] && break
  sleep 1
done
[ "$(voice_in_chat "${L_PROJECTS}")" = "$((BEFORE_NOTES + 1))" ] || fail "Send did not save the note in ${L_PROJECTS}"
sleep 2
shoot 09-note-sent

# ---- 6. The smallest window with large text: the page never scrolls, the field is whole ----
edit_settings 'd["text_size"] = "large"'
resize 640 400
check_page_scroll "640x400 large text, the forest hidden"
shoot 10-min-large-hidden
toggle
has "${L_COLLAPSE}" || fail "the switch did not bring the board back at 640x400"
press "${L_COLLAPSE}"
has "${L_EXPAND}" || fail "the board did not collapse at 640x400"
check_page_scroll "640x400 large text, the strip"
shoot 11-min-large-strip
press "${L_EXPAND}"
has "${L_COLLAPSE}" || fail "the board did not open at 640x400"
check_page_scroll "640x400 large text, the board"
shoot 12-min-large-board
edit_settings 'd["text_size"] = "normal"'
check_page_scroll "640x400 normal text, the board"
shoot 13-min-normal-board
resize 1280 800

# ---- Privacy: no file names or device names in the log ----
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
if [ -f "$LOG_FILE" ] && sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "voice-[0-9]|attachments/|AirPods|MacBook Microphone" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds file names or device names: $(head -c 300 "$K/debug/log-private.txt")"
fi

finish
