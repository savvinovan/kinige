#!/usr/bin/env bash
# The microphone probe of slice 7a on a macOS runner: scripts/screens/7a-probe.sh <built Kinige.app> (screens.yml, slice 7a-probe).
# The app is built with KINIGE_FAKE_MIC=1 and KINIGE_PROBE=1 (screens.yml). At load, and again on the first trusted click, the page
# records five seconds from the fake microphone with the real MediaRecorder and writes the facts as an entry in the Inbox of the
# synthetic profile (scripts/screens/probe.ts); this script reads those entries from the database into formats.txt.
# The facts: isSecureContext and navigator.mediaDevices on tauri://localhost, MediaRecorder.isTypeSupported and canPlayType, the bytes
# per second, the container bytes, and whether the recording decodes. It never touches the microphone or TCC, and no device label is
# collected. Input is one accessibility press (ax.js): no mouse, no keys.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7a-probe}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"

# Positive control: this build must carry the fake microphone (the guard in build-app.sh only proves anything if it can fire).
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --expect fake-mic || fail "the fake microphone is not in web/dist"

prepare_app "$SOURCE_APP"
launch_window
wait_for_window
# The first run (no gesture) records five seconds and decodes them; give it time.
sleep 25

probe_entries() { sqlite3 "$K/data/kinige.db" "select body_md || char(10) || '----' from message where body_md like 'KINIGE PROBE 7a%' order by created_at"; }

COUNT_BEFORE="$(sqlite3 "$K/data/kinige.db" "select count(*) from message where body_md like 'KINIGE PROBE 7a%'")"
echo "probe entries before the press: $COUNT_BEFORE"
# One trusted click for the second run. A press on a sidebar chat is as good as any; the result is part of the report.
PRESS_RESULT="$(ax press "$(app_pid)" "Inbox" 2>&1 || true)"
echo "press Inbox: $PRESS_RESULT"
sleep 25

{
  echo "# Facts about audio recording and playback in the webview of the built app (slice 7a probe, macOS runner)."
  echo "# AX press on the Inbox chat before the second run: $PRESS_RESULT"
  echo
  probe_entries
} > "$OUT/formats.txt"
cat "$OUT/formats.txt"

COUNT_AFTER="$(sqlite3 "$K/data/kinige.db" "select count(*) from message where body_md like 'KINIGE PROBE 7a%'")"
[ "$COUNT_BEFORE" -ge 1 ] || fail "the first probe run left no entry"
[ "$COUNT_AFTER" -ge 2 ] || fail "the second probe run (after the press) left no entry"
grep -q 'navigator.mediaDevices' "$OUT/formats.txt" || fail "the probe report has no facts"

shoot 01-window
finish
