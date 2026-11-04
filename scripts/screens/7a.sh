#!/usr/bin/env bash
# Screens of slice 7a (voice notes) in the real window on a macOS runner: scripts/screens/7a.sh <built Kinige.app>.
# The app is built with KINIGE_FAKE_MIC=1 (screens.yml): an oscillator replaces getUserMedia, so the real MediaRecorder and the whole
# pipeline run with no device and no TCC prompt. The data are synthetic (seed_demo with KINIGE_SEED_VOICE=1: a tone made by `afconvert`,
# a Telegram-style .ogg, a note whose file is gone, an hour-long note, a note with text). Input is accessibility actions only
# (ax.js): no mouse, no keyboard, no global shortcuts. The first-recording permission prompt and a real microphone are NOT covered here:
# they stay with the person who accepts the slice.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7a}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"
PROBE_FORMATS="$REPO/docs/screens/7a-probe/formats.txt"

# Keep the probe's facts if a probe run is committed; the scenario deletes only its own outputs.
rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"

# Positive control of the fake microphone guard: this build must carry it, or the guard has proved nothing.
"$REPO/scripts/check-fake-mic.sh" "$REPO/web/dist" --expect || { echo "ERROR: the fake microphone is not in web/dist"; FAILS=$((FAILS + 1)); }

KINIGE_SEED_VOICE=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
plutil -insert LSEnvironment.KINIGE_LOG -string debug "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
rm -f "$LOG_FILE"

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

labels() {
  if [ "$UI" = "ru" ]; then
    L_INBOX="Входящие"; L_MIC="Записать голосовое"; L_SEND_VOICE="Отправить голосовое сообщение"; L_TRASH_REC="Отменить запись"
    L_RECORDING="Идёт запись"; L_DISCARD_Q="Удалить запись?"; L_KEEP="Продолжить запись"; L_PLAY="Воспроизвести"; L_SPEED="Скорость воспроизведения"
    L_SEARCH="Поиск"; L_SEARCH_RESULTS="Результаты поиска"; L_MISSING="Файла нет"; L_NOTE="Голосовое сообщение"; L_OPEN_DEFAULT="Открыть в приложении по умолчанию"
  else
    L_INBOX="Inbox"; L_MIC="Record voice note"; L_SEND_VOICE="Send the voice note"; L_TRASH_REC="Discard the recording"
    L_RECORDING="Recording"; L_DISCARD_Q="Discard the recording?"; L_KEEP="Keep recording"; L_PLAY="Play"; L_SPEED="Playback speed"
    L_SEARCH="Search"; L_SEARCH_RESULTS="Search results"; L_MISSING="File is missing"; L_NOTE="Voice note"; L_OPEN_DEFAULT="Open in default app"
  fi
}

detect_language() {
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

restart_app() {
  kill -9 "$(app_pid)"
  sleep 2
  launch_window
  wait_for_window
  sleep 5
}

try_press() { [ "$(ax press "$(app_pid)" "$@" 2>&1 | cut -c1-7)" = "pressed" ]; }
voice_rows() { sqlw "select count(*) from attachment where filename like 'voice-%'"; }
# The first "m:ss / m:ss" of a window dump, in seconds (the elapsed time of the playing note), or empty.
elapsed_from() { grep -oE '[0-9]+:[0-9]{2} / [0-9:]+' "$1" | head -1 | python3 -c 'import sys; t=sys.stdin.read().split(" /")[0].strip(); p=[int(x) for x in t.split(":")] if t else []; print(sum(v*60**i for i,v in enumerate(reversed(p))) if p else "")'; }

# ---- 1. The feed: players, the missing file, the Telegram-style voice ----
set_language ru
launch_window
wait_for_window
sleep 8
detect_language
has "${L_NOTE}, 40" || fail "no voice player of 40 seconds in the feed"
has "${L_MISSING}" || fail "the voice note whose file is gone is not marked"
[ "$(sqlw "select count(*) from attachment_local where state = 'missing'")" = "1" ] || fail "the sweep did not mark exactly one file as missing"
has "3723" || fail "no player of the hour-long note (3723 seconds)"
shoot 01-feed
BEFORE_NOTES="$(voice_rows)"

# ---- 2. Recording: the bar, the timer, the meter ----
press "${L_MIC}"
sleep 4
has "${L_RECORDING}" || fail "the recording bar did not appear after the microphone button"
shoot 02-recording
grep -E '[0-9]:[0-9]{2}' "$K/debug/shot-02-recording.txt" | grep -iE "${L_RECORDING}|timer|0:0[2-9]" | head -5 > "$OUT/recording.txt" || true
cat "$OUT/recording.txt"
sleep 3

# ---- 3. Cancel: past 5 seconds the bar asks; Keep goes on ----
press "${L_TRASH_REC}"
sleep 2
has "${L_DISCARD_Q}" || fail "the discard confirmation did not open"
shoot 03-discard-confirm
press "${L_KEEP}"
sleep 2
has "${L_RECORDING}" || fail "Keep recording did not return to the recording bar"

# ---- 4. Send: the note is in the feed at once, with the player and no text ----
press "${L_SEND_VOICE}"
sleep 5
AFTER_NOTES="$(voice_rows)"
[ "${AFTER_NOTES}" -eq $((BEFORE_NOTES + 1)) ] || fail "the recording was not saved (${BEFORE_NOTES} -> ${AFTER_NOTES})"
NEWEST="$(sqlw "select id || '|' || mime || '|' || duration_ms from attachment where filename like 'voice-%' order by rowid desc limit 1")"
echo "newest note: ${NEWEST}"
case "$NEWEST" in *"|audio/mp4|"* | *"|audio/webm|"*) ;; *) fail "the saved note has an unexpected MIME: ${NEWEST}" ;; esac
[ "$(echo "$NEWEST" | cut -d'|' -f3)" -ge 5000 ] || fail "the saved note is shorter than the recording (${NEWEST})"
[ -z "$(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null)" ] || fail "a .part file is left after sending"
shoot 04-sent

# ---- 5. Playback: the elapsed time moves; one reading before, two after ----
press "${L_PLAY}"
sleep 2
dump playing-a
sleep 3
shoot 05-playing
A="$(elapsed_from "$K/debug/playing-a.txt")"
B="$(elapsed_from "$K/debug/shot-05-playing.txt")"
{
  echo "elapsed after the press (AX value, seconds): ${A:-none}"
  echo "elapsed 3 seconds and one dump later: ${B:-none}"
} > "$OUT/playback.txt"
cat "$OUT/playback.txt"
if [ -z "$A" ] || [ -z "$B" ] || [ "$B" -le "$A" ]; then
  fail "the elapsed time did not advance (a finding if the web view refused the autoplay of an AX press)"
fi

# ---- 6. Speed: 1x -> 1.5x -> 2x ----
press "${L_SPEED} 1×"
sleep 1
press "${L_SPEED} 1.5×"
sleep 2
has "2×" || fail "the speed chip does not say 2×"
shoot 06-speed-2x

# ---- 7. kill -9: the sent note is back and plays; a recording that was running is gone ----
press "${L_MIC}"
sleep 3
has "${L_RECORDING}" || fail "no second recording to lose"
restart_app
detect_language
[ "$(voice_rows)" = "${AFTER_NOTES}" ] || fail "a note appeared or disappeared across kill -9 ($(voice_rows) instead of ${AFTER_NOTES})"
has "${L_RECORDING}" && fail "the recording bar survived kill -9"
[ -z "$(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null | head -1)" ] || echo "note: a .part file is left (the sweep removes it on the next start)"
shoot 07-after-kill9

# ---- 8. Search: the word "voice" finds the notes (no transcription yet); the date in the name does not ----
osascript -e 'set the clipboard to "voice"' > "$K/debug/clipboard-search.txt" 2>&1
SEARCH_FOCUS="$(ax focus "$(app_pid)" "${L_SEARCH}" 2>&1)"
sleep 1
SEARCH_MENU="$(ax menu "$(app_pid)" "Edit|Правка" "Paste|Вставить" 2>&1)"
sleep 5
{
  echo "focus: ${SEARCH_FOCUS}"
  echo "menu: ${SEARCH_MENU}"
  if has "${L_SEARCH_RESULTS}"; then echo "RESULT: the search results are open for the word voice"; else echo "RESULT: the search results did not open (a finding)"; fi
} > "$OUT/search.txt"
cat "$OUT/search.txt"
shoot 08-search

# ---- 9. The forest card says how many voice notes the day has ----
kill -9 "$(app_pid)"
sleep 2
sqlw "insert or replace into app_setting(key, value) values ('forest_view', 'expanded')"
launch_window
wait_for_window
sleep 8
DUMP_TIMEOUT=420 shoot 09-forest-card
{
  echo "# Lines of the window tree of the expanded forest that hold the day card summary"
  grep -E '(записей|записи|запись|notes?) · .*(голосов|voice)' "$K/debug/shot-09-forest-card.txt" | sed -E 's/^ +//' | head -3
} > "$OUT/forest-card.txt"
cat "$OUT/forest-card.txt"
grep -qE 'голосов|voice' "$OUT/forest-card.txt" || fail "the forest card does not mention the voice notes of the day"
kill -9 "$(app_pid)"
sleep 2
sqlw "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"

# ---- 10. English, large text, 640 px ----
set_language en
launch_window
wait_for_window
sleep 6
detect_language
shoot 10-en
edit_settings 'd["text_size"] = "large"'
sleep 2
SHOOT_DUMP=0 shoot 11-large-text
edit_settings 'd["text_size"] = "normal"'
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
SHOOT_DUMP=0 shoot 12-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- Texts: the Markdown copy, the files, the formats ----
{
  echo "# Link lines of the voice notes in the Markdown copy"
  grep -rhE '^\[voice-[^]]+\]\(<' "$K/mirror" --include='*.md' | sort | uniq | head -10
  echo "# Files of the copy that are voice notes"
  (cd "$K/mirror" && find . -type f -name 'voice-*' | sort | while IFS= read -r f; do printf '%s %s\n' "$f" "$(stat -f %z "$f")"; done)
} > "$OUT/mirror.txt"
cat "$OUT/mirror.txt"
grep -qE '^\[voice-[0-9-]+\.m4a\]\(<[^>]+>\) \(0:40\)$' "$OUT/mirror.txt" || fail "no voice link with the length (0:40) in the Markdown copy"
grep -qE '\(1:02:03\)' "$OUT/mirror.txt" || fail "the hour-long note has no 1:02:03 in the Markdown copy"
: > "$OUT/files.txt"
{
  echo "rows: $(sqlw "select count(*) from attachment")  voice notes: $(voice_rows)  marked missing: $(sqlw "select count(*) from attachment_local where state = 'missing'")"
  echo "files in attachments/: $(find "$K/data/attachments" -maxdepth 1 -type f | wc -l | tr -d ' ')  leftover .part files: $(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null | wc -l | tr -d ' ')"
  echo "recorded notes by container: $(sqlw "select mime || ' x' || count(*) from attachment where filename like 'voice-%' group by mime" | tr '\n' ' ')"
} >> "$OUT/files.txt"
cat "$OUT/files.txt"
if [ -f "$PROBE_FORMATS" ]; then cp "$PROBE_FORMATS" "$OUT/formats.txt"; else echo "no probe output was committed" > "$OUT/formats.txt"; fi

# ---- Privacy of the log: no file names, no device names ----
if sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "voice-[0-9]|audio_1|attachments/|seed-voice|AirPods|MacBook Microphone" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds file names, device names or paths: $(head -c 300 "$K/debug/log-private.txt")"
fi
# The fake microphone's marker is in this build and only this build (the positive control ran at the top).

# ---- Measurements: send_voice, opening a chat of 200 notes, the cold start ----
kill -9 "$(app_pid)"
sleep 2
cargo build --release -q -p kinige-store --example voice_bench || fail "build of voice_bench"
BENCH="$REPO/target/release/examples/voice_bench"
[ -x "$BENCH" ] || BENCH="${CARGO_TARGET_DIR:-$REPO/target}/release/examples/voice_bench"
"$BENCH" "$K/bench" > "$OUT/bench.txt" 2>&1 || fail "voice_bench"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
cat "$OUT/bench.txt" "$OUT/cold-start.txt"
rm -rf "$K/bench"
python3 - "$OUT/bench.txt" <<'PY' || fail "a send_voice or chat-open p95 is over its limit"
import re, sys
bad = 0
for line in open(sys.argv[1], encoding="utf-8"):
    m = re.search(r"p95 ([0-9.]+) ms.*limit p95 ([0-9.]+) ms", line)
    if m and float(m.group(1)) >= float(m.group(2)):
        print("OVER:", line.strip())
        bad += 1
sys.exit(1 if bad else 0)
PY

finish
