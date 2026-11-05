#!/usr/bin/env bash
# The screens driver probe on a macOS runner (KINIG-92, ADR-0016): scripts/screens/screens-driver-probe.sh <built Kinige.app>
# (screens.yml, slice screens-driver-probe). The app is built with the fake microphone and the screens driver. It answers four questions
# that decide how far the driver can replace accessibility actions, and writes the facts to facts.txt:
#   1. Does `press` on a voice note's Play start playback although its click is not trusted (no user activation)?
#   2. Does `press` on the microphone start a recording from the fake microphone?
#   3. Does the driver answer while a recording runs, where an AX call hangs on a macOS runner?
#   4. How long does one step take, request file to reply file?
# Playback is read from the window itself (the Pause button, the slider's "m:ss of m:ss") and from the app log (the audio route).
# Synthetic data only. No mouse and no keyboard: the driver, plus one AX call during the recording as the comparison for question 3.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/screens-driver-probe}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"

"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --expect fake-mic || fail "the fake microphone is not in web/dist"
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || fail "the screens driver is not in web/dist and in the executable"

KINIGE_SEED_VOICE=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
plutil -insert LSEnvironment.KINIGE_LOG -string debug "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"
defaults write "$BUNDLE_ID" AppleLanguages -array en
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
rm -f "$LOG_FILE"

FACTS="$OUT/facts.txt"
fact() { echo "$*" | tee -a "$FACTS"; }
now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time() * 1000'; }
yes_no() { if [ "$1" -eq 0 ]; then echo yes; else echo no; fi; }
elapsed() { grep -oE '[0-9]+:[0-9]{2} of [0-9:]+' | head -1; }
route_requests() { grep -c 'audio route: ' "$LOG_FILE" 2> /dev/null || true; }
voice_rows() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "select count(*) from attachment where filename like 'voice-%'"; }

: > "$FACTS"
fact "# Facts about the screens driver on a macOS runner (KINIG-92 probe). $(sw_vers -productName) $(sw_vers -productVersion)"

launch_window
wait_for_window
# The first step waits for the page to say it listens (up to DRIVE_TIMEOUT); the first answer's time is a fact too.
START="$(now_ms)"
drive_raw main '{"kind":"has","name":"Inbox"}' 60
RC=$?
fact "first step (waits for the page to listen): ok=$(yes_no $RC), $(( $(now_ms) - START )) ms"
wait_name "Voice note, 40" 60000 || fail "the seeded voice note did not show"

# ---- 4. Round trip of one step: 20 'has' steps one after another ----
TIMES=""
for _ in $(seq 1 20); do
  START="$(now_ms)"
  drive_raw main '{"kind":"has","name":"Inbox"}' 10
  TIMES="$TIMES $(( $(now_ms) - START ))"
done
SORTED="$(echo "$TIMES" | tr ' ' '\n' | grep . | sort -n)"
fact "4. round trip of one step (20 'has' steps, ms): median $(echo "$SORTED" | sed -n 10p), min $(echo "$SORTED" | head -1), max $(echo "$SORTED" | tail -1)"
START="$(now_ms)"
driver_text main "$K/debug/text-idle.txt"
fact "4. one 'text' step of the idle window: $(( $(now_ms) - START )) ms, $(wc -c < "$K/debug/text-idle.txt" | tr -d ' ') bytes"

# ---- 1. Play without user activation ----
ROUTE_BEFORE="$(route_requests)"
drive_raw main '{"kind":"press","name":"Play"}' 10
RC=$?
fact "1. press Play: ok=$(yes_no $RC) ${DRIVE_ERROR}"
sleep 3
PLAYING=no; has "Pause" && PLAYING=yes
CANT=no; has "Can't play here" && CANT=yes
driver_text main "$K/debug/text-playing-a.txt"
A="$(elapsed < "$K/debug/text-playing-a.txt")"
sleep 3
driver_text main "$K/debug/text-playing-b.txt"
B="$(elapsed < "$K/debug/text-playing-b.txt")"
fact "1. 3 s after the press: Pause button shown: $PLAYING, 'Can't play here': $CANT"
fact "1. slider position after 3 s / 6 s: ${A:-none} / ${B:-none}"
fact "1. audio route requests in the app log before / after: ${ROUTE_BEFORE:-0} / $(route_requests)"
shoot 01-playing
drive_raw main '{"kind":"press","name":"Pause"}' 10

# ---- 2 and 3. Recording from the fake microphone, and the driver while it runs ----
BEFORE="$(voice_rows)"
drive_raw main '{"kind":"press","name":"Record voice note"}' 20
RC=$?
fact "2. press Record voice note: ok=$(yes_no $RC) ${DRIVE_ERROR}"
RECORDING=no; wait_name "Recording" 5000 && RECORDING=yes
fact "2. the recording bar shows 'Recording' after the press: $RECORDING"
sleep 2
shoot 02-recording
# Six steps one second apart, inside the 14 s of the recording: how many answered, and how many of them saw the recording bar.
OK=0; BAD=0; SEEN=0; MAX=0
for _ in $(seq 1 6); do
  START="$(now_ms)"
  if drive_raw main '{"kind":"has","name":"Recording"}' 10; then
    OK=$((OK + 1))
    [ "$(drive_value)" = "yes" ] && SEEN=$((SEEN + 1))
  else
    BAD=$((BAD + 1))
  fi
  TOOK=$(( $(now_ms) - START ))
  [ "$TOOK" -gt "$MAX" ] && MAX="$TOOK"
  sleep 0.5
done
fact "3. driver steps during the recording: $OK answered, $BAD not, $SEEN of them saw the recording bar, slowest $MAX ms"
START="$(now_ms)"
AX_RESULT="$(AX_TIMEOUT=10 ax has "$(app_pid)" "Recording" 2>&1 | head -c 80)"
fact "3. for comparison, one AX call right after (the recording may have ended by then): $(( $(now_ms) - START )) ms -> ${AX_RESULT:-<no answer>}"
# The fake microphone's recording ends by itself at 14 s and sends the note.
for _ in $(seq 1 30); do
  [ "$(voice_rows)" -gt "$BEFORE" ] && break
  sleep 2
done
fact "2. voice notes before / after: $BEFORE / $(voice_rows); newest: $(sqlite3 "$K/data/kinige.db" "select mime || ' ' || duration_ms || ' ms' from attachment where filename like 'voice-%' order by rowid desc limit 1")"
[ "$(voice_rows)" -gt "$BEFORE" ] || fail "no recording was saved after the press"
shoot 03-sent

fact "driver errors in this run: $FAILS"
finish
