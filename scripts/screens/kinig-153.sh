#!/usr/bin/env bash
# Screens of KINIG-153 (voice notes from the quick capture window) in the real windows on a macOS runner: scripts/screens/kinig-153.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=kinig-153 (the slice id is lower case: the workflow wants ^[0-9a-z-]+$).
# The app is built with the fake microphone and the screens driver (screens.yml: a tone through the real MediaRecorder, no device, no TCC prompt;
# the same guard as slice 7a keeps both out of a build for people). The data are synthetic (seed_demo) in a temporary directory. Input is the
# screens driver in the capture and the main window (KINIG-92): it finds elements by their accessible name inside each web view. The capture window
# is shown by the driver's `front` step, which runs the app's own `show_capture` (global shortcuts are never simulated), so the hotkey path is
# NOT covered here (vitest and the Rust tests cover the page's and the window's side of it). The first-use permission prompt and a real
# microphone are NOT covered: they stay with the person who accepts it.
# The fake microphone ends a recording by itself after 14 seconds (scripts/screens/fake-mic.ts), so every recording below is planned to be
# finished or dropped within about ten.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/kinig-153}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=30

INBOX="00000000-0000-4000-8000-000000000001"
SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"

# Positive controls: this build must carry the fake microphone and the driver, or the guard proved nothing.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --expect fake-mic || { echo "ERROR: the fake microphone is not in web/dist"; FAILS=$((FAILS + 1)); }
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }

cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }

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

L_INBOX="Inbox"; L_NEW_NOTE="New note"; L_MIC="Record voice note"; L_SEND_VOICE="Send the voice note"; L_RECORDING="Recording"
L_TRASH_REC="Discard the recording"; L_DISCARD_Q="Discard the recording?"; L_DISCARD="Discard"; L_BUSY="The microphone is unavailable"
L_DISMISS="Dismiss"; L_NOTE="Voice note, "; L_STOPPED="Recording stopped"

voice_rows() { sqlw "select count(*) from attachment where filename like 'voice-%'"; }
voice_in_inbox() {
  sqlw "select count(*) from attachment a join message m on m.id = a.message_id where a.filename like 'voice-%' and m.chat_id = '${INBOX}' and m.deleted_at is null"
}

# Whether the capture window is on screen: a window above the normal layer, 500 to 700 px wide (the icon's menu is narrower). Read from the
# window list, so "absent" is an answer and not a timeout.
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

# The capture window through the driver's `front` for it: the app runs its own `show_capture`, as for the hotkey and the icon menu (the first
# run of this scenario tried the icon menu through System Events: its menu did not exist for accessibility, "Invalid index"). A hidden
# window's page answers the driver too, so a press that works is no proof the window is on screen: `capture_present` is that proof.
show_capture() {
  local attempt
  for attempt in 1 2 3; do
    drive_raw capture '{"kind":"front"}' > /dev/null
    for _ in $(seq 1 10); do
      if capture_present; then
        sleep 1
        in_capture
        # The field's microphone button, or the recording bar of a kept recording that stands in for the field.
        wait_name "${L_MIC}" 10000 || wait_name "${L_RECORDING}," 10000 || fail "the capture window has neither the microphone button nor a recording bar"
        return 0
      fi
      sleep 1
    done
  done
  fail "the capture window did not show (see tray-*.txt in the debug files)"
  return 1
}

# The capture window's own shot: the text of the window is read for the personal-path check, the picture is the window alone.
shoot_capture() {
  in_capture
  shoot "$1" capture
}

shoot_main() {
  in_main
  shoot "$1" main
}

# Whether the capture window's text (what a person reads, labels included) has these words; the driver's `has` matches accessible names only.
capture_says() { driver_text capture "$K/debug/says.txt" 5 > /dev/null 2>&1 && grep -q "$1" "$K/debug/says.txt"; }
wait_capture_says() {
  for _ in $(seq 1 "${2:-10}"); do
    capture_says "$1" && return 0
    sleep 1
  done
  return 1
}

# A press that is allowed to find nothing (no scenario error): the recording may have ended by itself already.
try_press() { drive_raw "$(drive_window)" "{\"kind\":\"press\",$(driver_query "$@")}" 5; }

# ---- 0. The app, in English ----
set_language en
launch_window
wait_for_window
in_main
wait_name "${L_INBOX}" 120000 || fail "the main window did not show the Inbox"
BEFORE_NOTES="$(voice_rows)"

# ---- 1. A text note from the capture window first: the window shows today's entries above the field afterwards ----
show_capture
drive "$(drive_window)" "{\"kind\":\"type\",\"name\":\"New note\",\"text\":\"Idea: a journal where the year looks like a forest.\"}"
press "Send" Button
wait_capture_hidden || fail "the capture window did not hide after a text note"
[ "$(sqlw "select count(*) from message where chat_id = '${INBOX}' and body_md like 'Idea: a journal%'")" = "1" ] || fail "the text note is not in the Inbox"

# ---- 2. The window with the microphone button ----
show_capture
has "${L_MIC}" || fail "no microphone button in the capture window"
shoot_capture 01-idle-with-microphone
grep -q "Idea: a journal" "$K/debug/shot-01-idle-with-microphone.txt" || fail "the capture window does not list the text note"

# ---- 3. A recording keeps the window when the main window takes the focus ----
# The press starts the fake microphone; the bar shows; then the main window is brought to the front (the capture window loses the key,
# which used to hide it). The window list says whether it is still there, and the capture window's own text says it is still recording.
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the recording bar did not show in the capture window"
sleep 1
drive_raw main '{"kind":"front"}' > /dev/null
sleep 3
{
  if capture_present; then echo "capture window on screen after the main window took the focus: yes"; else echo "capture window on screen after the main window took the focus: NO"; fi
  in_capture
  if has "${L_RECORDING}"; then echo "recording bar still in the capture window: yes"; else echo "recording bar still in the capture window: NO"; fi
} > "$OUT/focus.txt"
cat "$OUT/focus.txt"
grep -q "focus: yes" "$OUT/focus.txt" || fail "the capture window hid while it was recording"
grep -q "window: yes" "$OUT/focus.txt" || fail "the recording bar is gone after the focus moved"
shoot_capture 02-recording-main-window-in-front
# Send; no scenario error if the recording ended by itself in the meantime (the note is saved either way, and the count below decides).
in_capture
try_press "${L_SEND_VOICE}" || true
wait_capture_hidden || fail "the capture window did not hide after sending a recording"
sleep 2
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 1))" ] || fail "the recording is not in the Inbox (voice notes in the Inbox: $(voice_in_inbox))"

# ---- 4. One microphone: the main window cannot record while the capture window does ----
show_capture
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the second recording did not start in the capture window"
in_main
press "${L_MIC}"
wait_name "${L_BUSY}" 8000 || fail "the main window did not say the microphone is unavailable"
shoot_main 03-main-window-microphone-busy
press "${L_DISMISS}" Button
in_capture
try_press "${L_SEND_VOICE}" || true
wait_capture_hidden || fail "the capture window did not hide after the second recording"
sleep 2
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 2))" ] || fail "the main window recorded too, or a note is missing (voice notes in the Inbox: $(voice_in_inbox), wanted $((BEFORE_NOTES + 2)))"
{
  echo "voice notes in the Inbox before / after: ${BEFORE_NOTES} / $(voice_in_inbox)"
  echo "voice notes in other chats: $(sqlw "select count(*) from attachment a join message m on m.id = a.message_id where a.filename like 'voice-%' and m.chat_id != '${INBOX}'")"
} > "$OUT/inbox.txt"
cat "$OUT/inbox.txt"

# ---- 5. The new notes are in the capture window's list and in the main window's chat ----
show_capture
shoot_capture 04-list-with-voice-notes
grep -q "Voice note" "$K/debug/shot-04-list-with-voice-notes.txt" || fail "the capture window's list has no voice note"
shoot_main 05-main-chat-with-voice-notes
in_main
wait_name "${L_NOTE}" 20000 || fail "the main window's Inbox shows no voice note player"
# The capture window loses the key when the main window is shown and hides (nothing is recording).
wait_capture_hidden || fail "the capture window stayed on screen with nothing recording"

# ---- 6. Large text: the bar and the question must fit the small window ----
edit_settings 'd["text_size"] = "large"'
show_capture
shoot_capture 06-idle-large
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the recording bar did not show in large text"
sleep 1
shoot_capture 07-recording-large
# The trash button drops a recording under five seconds at once and asks over five; the shot took a few seconds, so either may come.
press "${L_TRASH_REC}"
if has "${L_DISCARD_Q}"; then press "${L_DISCARD}" Button; fi
wait_name "${L_RECORDING}" 5000 gone || fail "the recording was not dropped"
# Over five seconds it asks first: Esc is the bar's (the window stays), and the question fits.
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the recording bar did not show for the question"
sleep 5
drive "$(drive_window)" '{"kind":"key","key":"Escape"}'
wait_name "${L_DISCARD_Q}" 5000 || fail "the question 'Discard the recording?' did not show on Esc"
capture_present || fail "Esc hid the capture window while it was recording"
shoot_capture 08-discard-question-large
press "${L_DISCARD}" Button
wait_name "${L_DISCARD_Q}" 5000 gone || fail "Discard did not close the question"
capture_present || fail "the capture window hid after Discard"
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 2))" ] || fail "a discarded recording was saved"
# A recording is hidden mid-way (the driver's `hide` is the app's own request, the one the hotkey and closing the window make): the window
# goes, nothing is saved, and the recording is kept. Reopened, it says so; Esc hides the window and keeps it; the trash button (under five
# seconds here, so no question) is what drops it.
press "${L_MIC}"
wait_name "${L_RECORDING}" 10000 || fail "the recording bar did not show for the hide"
sleep 3
drive_raw capture '{"kind":"hide"}' > /dev/null
wait_capture_hidden || fail "the capture window did not hide on the app's hide request"
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 2))" ] || fail "hiding the window saved the recording"
show_capture
wait_capture_says "${L_STOPPED}" 10 || fail "the kept recording is not there when the window comes back"
shoot_capture 09-recording-kept-large
drive "$(drive_window)" '{"kind":"key","key":"Escape"}'
wait_capture_hidden || fail "Esc did not hide the window with a kept recording"
show_capture
wait_capture_says "${L_STOPPED}" 10 || fail "Esc discarded the kept recording"
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 2))" ] || fail "a kept recording was saved by Esc"
press "${L_TRASH_REC}"
for _ in $(seq 1 10); do capture_says "${L_STOPPED}" || break; sleep 1; done
capture_says "${L_STOPPED}" && fail "the trash button did not drop the kept recording"
[ "$(voice_in_inbox)" = "$((BEFORE_NOTES + 2))" ] || fail "a discarded kept recording was saved"
drive "$(drive_window)" '{"kind":"key","key":"Escape"}'
wait_capture_hidden || fail "Esc did not hide the idle capture window"
edit_settings 'd["text_size"] = "normal"'

# ---- Privacy: no file names or device names in the log ----
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
if [ -f "$LOG_FILE" ] && sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "voice-[0-9]|attachments/|AirPods|MacBook Microphone" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds file names or device names: $(head -c 300 "$K/debug/log-private.txt")"
fi

finish
