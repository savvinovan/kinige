#!/usr/bin/env bash
# Screens of KINIG-145 ("Jump to latest" in the feed) in the real window on a macOS runner: scripts/screens/kinig-145.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=kinig-145 (the slice id is lower case: the workflow wants ^[0-9a-z-]+$).
# The data are synthetic (seed_demo with KINIGE_SEED_FOREST=1: a lived-in year, the last ten days in the Inbox) in a temporary
# directory; the real database is not touched. The forest is hidden, so that the window tree stays small. Input is accessibility
# actions only (ax.js): no mouse, no keyboard, no global shortcuts. The feed is scrolled up with AXScrollToVisible on one of the oldest
# loaded entries, the button is pressed with AXPress. The interface language is Russian, then English; large text; a 640 px window.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/kinig-145}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

INBOX="00000000-0000-4000-8000-000000000001"
SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_FOREST=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
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

labels() {
  if [ "$UI" = "ru" ]; then L_JUMP="К последним записям"; else L_JUMP="Jump to latest"; fi
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
  detect_language
}

# One of the oldest entries on the first page of the Inbox (the feed loads the last 200): the first entry of a day whose text ends with
# the date, so that the search finds it near the start of the tree. Scrolling to it puts the feed more than a screen above its end.
EARLY="$(sqlw "select journal_date from (select * from message where chat_id = '$INBOX' and deleted_at is null order by journal_date desc, created_at desc, id desc limit 200) where body_md like '% · ' || journal_date order by journal_date limit 1")"
[ -n "${EARLY}" ] || fail "no dated entry on the first page of the Inbox"
EARLY_MARK="· ${EARLY}"
echo "the oldest loaded dated entry: ${EARLY}"

# ---- 1. The feed at its end: no button ----
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
has "Входящие" || fail "the window did not show the Inbox"
has "${L_JUMP}" && fail "the button is on screen although the feed is at its end"
shoot 01-at-the-end-no-button

# ---- 2. Scrolled up by more than a screen: the button is there, in the bottom right corner, above the composer ----
scroll_to "${EARLY_MARK}"
sleep 3
has "${L_JUMP}" || fail "no \"${L_JUMP}\" button after scrolling up"
shoot 02-scrolled-up-with-button

# ---- 3. A click: the end of the feed at once, the button is gone ----
press "${L_JUMP}"
sleep 3
has "${L_JUMP}" && fail "the button stayed after the click"
shoot 03-after-click

# ---- 4. Large text: the button grows with the text and stays above the composer ----
edit_settings 'd["text_size"] = "large"'
sleep 2
scroll_to "${EARLY_MARK}"
sleep 3
has "${L_JUMP}" || fail "no button with large text"
shoot 04-large-text-with-button
press "${L_JUMP}"
edit_settings 'd["text_size"] = "normal"'

# ---- 5. A 640 px window: the button overlays the right edge of the column ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
scroll_to "${EARLY_MARK}"
sleep 3
has "${L_JUMP}" || fail "no button in the narrow window"
shoot 05-narrow-640-with-button
press "${L_JUMP}"
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- 6. English: the label is "Jump to latest" ----
set_language en
restart_app
scroll_to "${EARLY_MARK}"
sleep 3
has "${L_JUMP}" || fail "no \"${L_JUMP}\" button in English"
shoot 06-en-with-button

finish
