#!/usr/bin/env bash
# Diagnostic of slice 7a on a macOS runner: how long does an accessibility call take while a (fake-microphone) recording runs, what are
# the processes doing, and where is the web content process stuck. scripts/screens/7a-load.sh <built Kinige.app> (screens.yml, slice 7a-load).
# The first 7a runs found every accessibility call timing out for as long as a recording ran; this scenario measures instead of guessing.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7a-load}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=30
rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
defaults write "$BUNDLE_ID" AppleLanguages -array en
launch_window
wait_for_window
sleep 8

timed() { # label, command...
  local label="$1" start end
  shift
  start="$(date +%s)"
  local result
  result="$("$@" 2>&1 | head -c 120)"
  end="$(date +%s)"
  echo "$label: $((end - start)) s -> ${result:-<empty>}" | tee -a "$OUT/load.txt"
}
snapshot() {
  {
    echo "# processes by CPU ($1)"
    ps -Ao pid,pcpu,pmem,comm | sort -k2 -nr | head -6
  } | tee -a "$OUT/load.txt"
}

: > "$OUT/load.txt"
snapshot "idle, before the press"
timed "has Inbox (idle)" ax has "$(app_pid)" "Inbox"
timed "has Inbox (idle, again)" ax has "$(app_pid)" "Inbox"
timed "press the microphone" ax press "$(app_pid)" "Record voice note"
for i in 1 2 3 4 5 6; do
  snapshot "recording, check $i"
  timed "has Recording (check $i)" ax has "$(app_pid)" "Recording"
  sleep 3
done
WEB="$(pgrep -f 'com.apple.WebKit.WebContent' | head -1)"
echo "web content process: ${WEB:-none}" | tee -a "$OUT/load.txt"
if [ -n "$WEB" ]; then
  limit 40 sample "$WEB" 3 -file "$OUT/web-content-sample.txt" > /dev/null 2>&1 || echo "sample failed" | tee -a "$OUT/load.txt"
fi
timed "press Send" ax press "$(app_pid)" "Send the voice note"
sleep 6
timed "has Voice note after Send" ax has "$(app_pid)" "Voice note, "
snapshot "after Send"
SHOOT_DUMP=0 shoot 01-after
finish
