#!/usr/bin/env bash
# Screens of slice 7a (voice notes) in the real window on a macOS runner: scripts/screens/7a.sh <built Kinige.app>.
# The app is built with KINIGE_FAKE_MIC=1 (screens.yml): an oscillator replaces getUserMedia, so the real MediaRecorder and the whole
# pipeline run with no device and no TCC prompt. The data are synthetic (seed_demo with KINIGE_SEED_VOICE=1: a tone made by `afconvert`,
# a Telegram-style .ogg, a note whose file is gone, an hour-long note, a note with text). Input is the screens driver inside the app
# (KINIG-92, ADR-0016; DRIVER=1): it finds elements by their accessible name in the app's own web view, so it answers while a recording
# runs (an accessibility call hung then) and a read of the window text takes ~100 ms (a tree dump took up to 3 minutes). Accessibility
# actions stay for what the web view cannot reach: resizing the window. No mouse, no keyboard, no global shortcuts. The first-recording
# permission prompt and a real microphone are NOT covered here: they stay with the person who accepts the slice.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7a}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=30

SETTINGS_FILE="$K/data/settings.json"
PROBE_FORMATS="$REPO/docs/screens/7a-probe/formats.txt"

# Keep the probe's facts if a probe run is committed; the scenario deletes only its own outputs.
rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"

# Positive control of the fake microphone guard: this build must carry it, or the guard has proved nothing.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --expect fake-mic || { echo "ERROR: the fake microphone is not in web/dist"; FAILS=$((FAILS + 1)); }
# The same for the screens driver, in web/dist and in the executable.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }

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
    L_PAUSE="Пауза"; L_CANT="Здесь не воспроизвести"
  else
    L_INBOX="Inbox"; L_MIC="Record voice note"; L_SEND_VOICE="Send the voice note"; L_TRASH_REC="Discard the recording"
    L_RECORDING="Recording"; L_DISCARD_Q="Discard the recording?"; L_KEEP="Keep recording"; L_PLAY="Play"; L_SPEED="Playback speed"
    L_SEARCH="Search"; L_SEARCH_RESULTS="Search results"; L_MISSING="File is missing"; L_NOTE="Voice note"; L_OPEN_DEFAULT="Open in default app"
    L_PAUSE="Pause"; L_CANT="Can't play here"
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

# Waits for a text in the window, up to `$2` seconds: the page polls every 50 ms and answers at once.
wait_for() { wait_name "$1" $(($2 * 1000)); }
# One press that is allowed to find nothing (no scenario error): the note may have ended already.
try_press() { drive_raw "$(drive_window)" "{\"kind\":\"press\",$(driver_query "$@")}" 5; }
voice_rows() { sqlw "select count(*) from attachment where filename like 'voice-%'"; }
# The first "m:ss of m:ss" (the waveform slider's value text) of a window dump, in seconds (the elapsed time of the playing note), or empty.
elapsed_from() { grep -oE '[0-9]+:[0-9]{2} (of|из) [0-9:]+' "$1" | head -1 | python3 -c 'import sys; t=sys.stdin.read().split(" ")[0].strip(); p=[int(x) for x in t.split(":")] if t else []; print(sum(v*60**i for i,v in enumerate(reversed(p))) if p else "")'; }
# The audio notes whose file is on disk (the seeded note whose file is gone never gets a waveform), oldest first; the Telegram-style stub is one.
AUDIO_LOCAL="from attachment a join attachment_local l on l.attachment_id = a.id where a.kind = 'audio' and l.state = 'local' order by a.rowid"
# The length in bytes of the saved waveform of each of them: 64 (peaks), 0 (the empty marker) or - (not known yet).
waveform_lengths() { sqlw "select coalesce(length(l.waveform), '-') ${AUDIO_LOCAL}" | tr '\n' ' '; }
# The same with the name, the length of the note and what the 0 means: the 20-minute cap (never decoded) is not "the decoder could not read".
waveform_report() {
  sqlw "select a.filename, coalesce(a.duration_ms, ''), coalesce(length(l.waveform), '-') ${AUDIO_LOCAL}" | while IFS='|' read -r name ms len; do
    case "$len" in
      64) meaning="peaks" ;;
      0) if [ -n "$ms" ] && [ "$ms" -gt 1200000 ]; then meaning="empty marker: over the 20-minute cap, never decoded"; else meaning="empty marker: the decoder could not read the file"; fi ;;
      *) meaning="not known yet" ;;
    esac
    printf '%s | %s ms | %s bytes | %s\n' "$name" "${ms:-unknown length}" "$len" "$meaning"
  done
}

# ---- 1. The feed: players, the missing file, the Telegram-style voice ----
set_language ru
launch_window
wait_for_window
# The first step waits for the page to listen; then the feed may still be drawing on a loaded VM: wait for the players instead of assuming.
wait_for "Входящие" 120 || fail "the window did not show the Inbox"
detect_language
wait_for "${L_NOTE}, 40" 120 || fail "no voice player of 40 seconds in the feed"
wait_for "${L_MISSING}" 60 || fail "the voice note whose file is gone is not marked"
[ "$(sqlw "select count(*) from attachment_local where state = 'missing'")" = "1" ] || fail "the sweep did not mark exactly one file as missing"
wait_for "3723" 30 || fail "no player of the hour-long note (3723 seconds)"
shoot 01-feed
BEFORE_NOTES="$(voice_rows)"

# ---- 5. Playback: the picture while it plays, then the log of the audio route ----
ROUTE_BEFORE="$(grep -c 'audio route: ' "$LOG_FILE" 2> /dev/null || true)"
press "${L_PLAY}"
sleep 14
# 14 s in, the played part of the waveform stands about a third of the way along: the shot shows the fill stopping inside the bars. The
# window text is read through the driver right after the picture (about 100 ms), so the elapsed time and the pause button are the
# playing note's, not those of a note that has ended by the time a tree dump finishes.
shoot 05-playing
cp "$K/debug/shot-05-playing.txt" "$K/debug/playing-a.txt"
A="$(elapsed_from "$K/debug/playing-a.txt")"
{
  echo "elapsed right after the picture (the slider's value text through the driver, seconds): ${A:-none}"
  echo "pause button in the window: $(has "${L_PAUSE}" && echo yes || echo no)"
  echo "'Can't play here' in the window: $(has "${L_CANT}" && echo yes || echo no)"
  echo "audio route requests before / after the press (app log): ${ROUTE_BEFORE:-0} / $(grep -c 'audio route: ' "$LOG_FILE" 2> /dev/null || true)"
  grep 'audio route: ' "$LOG_FILE" 2> /dev/null | sed -E 's/^\[[^]]*\]\[[^]]*\]//' | head -6
} > "$OUT/playback.txt"
cat "$OUT/playback.txt"
# The proof of playback is the web view's requests of the audio route (80 of them in the committed run), the pause button and a moving time.
ROUTE_AFTER="$(grep -c 'audio route: ' "$LOG_FILE" 2> /dev/null || true)"
echo "playback proof: ${ROUTE_AFTER:-0} audio route requests after ${ROUTE_BEFORE:-0} before the press; elapsed (seconds): ${A:-none}" >> "$OUT/playback.txt"
if [ "${ROUTE_AFTER:-0}" -le "${ROUTE_BEFORE:-0}" ]; then
  fail "the voice note did not play: the web view made no request of the audio route"
fi
[ -n "${A:-}" ] && [ "${A:-0}" -ge 5 ] || fail "the player does not show the note moving (elapsed: ${A:-none})"

# ---- 6. Speed: 1x -> 1.5x -> 2x ----
press "${L_SPEED} 1×"
sleep 1
press "${L_SPEED} 1.5×"
sleep 2
has "2×" || fail "the speed chip does not say 2×"
shoot 06-speed-2x

# The note may still be playing: pause it before the recording starts (one press; no answer is fine when it has ended).
try_press "${L_PAUSE}" || true
sleep 2

# ---- 2. Recording: the bar and the timer, then the recording ends by itself ----
# The driver answers while a recording runs (an accessibility call hung then, so the old scenario pressed once and waited). The press is
# one driver step; the bar is checked and the shot is taken while the recording runs. The fake microphone's recording ends by itself
# after 14 seconds (scripts/screens/fake-mic.ts). The discard question and Keep are covered by the Storybook stories and vitest.
press "${L_MIC}"
wait_for "${L_RECORDING}" 10 || fail "the recording bar did not show after the press"
sleep 5
shoot 02-recording
echo "the shot was taken after the press, a 5 s sleep and the shot's own wait, so the timer in it (about 0:08) is the evidence, not the 5 s; the recording ends by itself at 14 s" > "$OUT/recording.txt"

# ---- 3. The recording sent itself: the note is in the feed, with the player and no text ----
for _ in $(seq 1 30); do
  [ "$(voice_rows)" -gt "${BEFORE_NOTES}" ] && break
  sleep 3
done
sleep 4
AFTER_NOTES="$(voice_rows)"
[ "${AFTER_NOTES}" -eq $((BEFORE_NOTES + 1)) ] || fail "the recording was not saved (${BEFORE_NOTES} -> ${AFTER_NOTES})"
NEWEST="$(sqlw "select id || '|' || mime || '|' || duration_ms from attachment where filename like 'voice-%' order by rowid desc limit 1")"
RECORDED_ID="$(echo "$NEWEST" | cut -d'|' -f1)"
echo "newest note: ${NEWEST}"
case "$NEWEST" in *"|audio/mp4|"* | *"|audio/webm|"*) ;; *) fail "the saved note has an unexpected MIME: ${NEWEST}" ;; esac
[ "$(echo "$NEWEST" | cut -d'|' -f3)" -ge 5000 ] || fail "the saved note is shorter than the recording (${NEWEST})"
[ -z "$(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null)" ] || fail "a .part file is left after sending"
# The recorded note has a waveform: the recorder's peaks went with it (the fake microphone's tone swings through silence, so the shape varies).
RECORDED_WAVE="$(sqlw "select coalesce(length(waveform), '-') from attachment_local where attachment_id = '${RECORDED_ID}'")"
echo "waveform of the recorded note (bytes): ${RECORDED_WAVE}"
[ "${RECORDED_WAVE}" = "64" ] || fail "the recorded note has no 64-peak waveform from the recorder (${RECORDED_WAVE})"
shoot 04-sent
# The seeded note that has no peaks in its row is decoded by the player when it shows on screen (the lazy path): wait for it.
for _ in $(seq 1 12); do
  case "$(waveform_lengths)" in *"-"*) sleep 5 ;; *) break ;; esac
done
{
  echo "# the saved waveform of every audio note whose file is on disk, oldest first: name | length of the note | bytes saved | what it means"
  echo "# (64 bytes are peaks; 0 is the empty marker, from the 20-minute cap or from a file the decoder could not read; the Telegram-style stub is one of them)"
  waveform_report
  echo "recorded note: ${RECORDED_WAVE} bytes"
} > "$OUT/waveform.txt"
cat "$OUT/waveform.txt"
case "$(waveform_lengths)" in *"-"*) echo "finding: a note is still without a waveform (the player never decoded it)" ;; esac

# ---- 7. kill -9: the sent note is back and plays; a recording that was running is gone ----
# A second recording is started and the process is killed 6 seconds later, while it runs.
press "${L_MIC}"
wait_for "${L_RECORDING}" 10 || fail "the second recording did not start"
sleep 6
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
wait_for "${L_INBOX}" 60 || fail "the window did not show the Inbox after kill -9"
detect_language
[ "$(voice_rows)" = "${AFTER_NOTES}" ] || fail "a note appeared or disappeared across kill -9 ($(voice_rows) instead of ${AFTER_NOTES})"
has "${L_RECORDING}" && fail "the recording bar survived kill -9"
[ -z "$(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null | head -1)" ] || echo "note: a .part file is left (the sweep removes it on the next start)"
shoot 07-after-kill9

# ---- 8. Search: the word "voice" finds the notes (no transcription yet); the date in the name does not ----
# The word goes into the search field as typing (the driver's native value setter plus an input event): no clipboard and no Edit menu.
if drive "$(drive_window)" "{\"kind\":\"type\",\"name\":\"$(json_escape "${L_SEARCH}")\",\"text\":\"voice\"}"; then SEARCH_TYPED=typed; else SEARCH_TYPED="not typed: ${DRIVE_ERROR}"; fi
wait_for "${L_SEARCH_RESULTS}" 10
{
  echo "type into '${L_SEARCH}': ${SEARCH_TYPED}"
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
wait_for "${L_INBOX}" 60 || fail "the window did not show the Inbox with the forest open"
sleep 8
shoot 09-forest-card
{
  echo "# Lines of the window text of the expanded forest that hold the day card summary"
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
wait_for "Inbox" 60 || fail "the window did not show the Inbox in English"
sleep 3
detect_language
shoot 10-en
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot 11-large-text
edit_settings 'd["text_size"] = "normal"'
# Resizing the window is native: the one accessibility call left in this scenario.
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot 12-narrow-640
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
  echo "seeded voice notes: ${BEFORE_NOTES} (made by seed_demo, not recorded); recorded in this run: $(($(voice_rows) - BEFORE_NOTES)), container: $(sqlw "select mime from attachment where id = '${RECORDED_ID}'")"
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
# The cold start of the chat with 200 voice notes between two controls on an empty database with the same binary and the same VM: the VM
# drifts (slice 7 saw a median of 1.27 s and of 1.82 s on the same code), so the number is read against the controls taken right around it.
{
  KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start-control-before.txt" 2>&1 || fail "cold start control (before)"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
{
  KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start-control-after.txt" 2>&1 || fail "cold start control (after)"
cat "$OUT/bench.txt" "$OUT/cold-start-control-before.txt" "$OUT/cold-start.txt" "$OUT/cold-start-control-after.txt"
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
