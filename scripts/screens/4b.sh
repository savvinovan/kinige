#!/usr/bin/env bash
# Shots of slice 4b (the Settings window) in the real window on the macOS runner: scripts/screens/4b.sh <built Kinige.app>.
# The data is synthetic (seed_demo) in a temporary directory; the real database is not touched. The result is $OUT (docs/screens/4b).
# Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. Since KINIG-160 the settings file is gone:
# the shots of its page (08, 09, 11, 12) and `settings-file.txt` stay in docs/screens/4b as history, and the large text is set in the window.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/4b}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"
INBOX="00000000-0000-4000-8000-000000000001"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
WORK="$(chat_id 'Работа')"
BOOKS="$(chat_id 'Книги')"

setting() { sql "select value from app_setting where key = '$1'"; }

# ---- 1. Launch, the main window ----
launch_window
wait_for_window
sleep 2
{
  echo "# The Settings window opens from the profile; a second press raises the same window, no second one appears."
  echo "Settings windows before the press: $(settings_windows)"
} > "$OUT/one-window.txt"
in_main
press "Profile"
press "Settings"
wait_for_settings
sleep 2
echo "after the first press: $(settings_windows)" >> "$OUT/one-window.txt"
press "Profile"
press "Settings"
sleep 2
echo "after the second press: $(settings_windows)" >> "$OUT/one-window.txt"
[ "$(settings_windows)" = "1" ] || fail "there is not exactly one settings window"
cat "$OUT/one-window.txt"

# ---- 2. The tree sections ----
in_settings
shoot_settings 01-appearance
press "Keyboard shortcuts"
shoot_settings 02-shortcuts
# Recording a new shortcut: the recording field in place of the keys, the row does not change the layout (no mouse and no keys: an AX press on "Change").
press "Change shortcut: Quick capture window"
shoot_settings 02b-shortcuts-recording
press "Copies"
shoot_settings 03-mirror
press "AI agent access"
shoot_settings 04-access-off
# Evidence about the second scroll "thumb" on the access page: the same view after switching to another section and back, and after
# a repaint (the window one pixel larger and back). If the thumb is a leftover (an old WebKit layer), it will disappear.
press "Appearance"
press "AI agent access"
shoot_settings 04b-access-after-return
{
  echo "# The size of the Settings window during the repaint for shot 04c (request 861 x 600, then 860 x 600)."
  echo "after +1 px: $(ax resize "$(app_pid)" 861 600)"
  sleep 2
  echo "after returning: $(ax resize "$(app_pid)" 860 600)"
} > "$OUT/repaint.txt"
cat "$OUT/repaint.txt"
sleep 2
shoot_settings 04c-access-after-resize

# ---- 3. Turning on: the warning over the window, the content under it does not shift ----
# While the warning is open (aria-modal), WebKit hides the rest of the page from accessibility, so the frames under it
# cannot be read. Here the frames are compared before the warning is shown and after it is closed; during the display the geometry (including with
# permanent scroll bars) is checked by tests/visual/scrollbars.spec.ts, and in shot 05 the page under the backdrop is visible.
{
  echo "# Frames (x y width height, screen pixels) before the warning is shown and after it is closed (\"Cancel\")."
  echo "access checkbox before: $(ax frame "$(app_pid)" "Allow agents to access the journal")"
  echo "tree item before:       $(ax frame "$(app_pid)" "Copies")"
} > "$OUT/no-shift.txt"
press "Allow agents to access the journal"
has "Turn on access" || has "Turn on" || fail "the warning is not shown"
shoot_settings 05-warning
press "Cancel"
{
  echo "access checkbox after: $(ax frame "$(app_pid)" "Allow agents to access the journal")"
  echo "tree item after:       $(ax frame "$(app_pid)" "Copies")"
} >> "$OUT/no-shift.txt"
cat "$OUT/no-shift.txt"
before_box="$(sed -n 's/^access checkbox before: *//p' "$OUT/no-shift.txt")"
after_box="$(sed -n 's/^access checkbox after: *//p' "$OUT/no-shift.txt")"
[ -n "$before_box" ] && [ "$before_box" = "$after_box" ] || fail "the checkbox shifted when the warning was shown ($before_box -> $after_box)"
press "Allow agents to access the journal"
press "Turn on"
press "Read and write"

# ---- 4. The "Visible chats" subscreen ----
press "Configure"
press "Inbox" CheckBox
shoot_settings 06-visible-chats

# ---- 5. An agent session through the bridge: a log of several pages ----
for round in 1 2 3; do
  limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$K/debug/session-$round.txt" --chat "$INBOX" --closed-chat "$BOOKS" || fail "agent session $round"
done
sleep 4
press "Access log"
scroll_to "Show more"
shoot_settings 07-access-log-long

# ---- 6. Large text, set the way a person sets it (KINIG-160, ADR-0022: there is no settings file) ----
# The helper opens the settings window with the screens driver and presses "Large"; no page of the window mentions a file any more.
set_text_size large
in_settings
press "AI agent access"
has "settings.json" && fail "the window mentions settings.json"
has "Changed via" && fail "a \"Changed via\" line is shown"
press "Access log"
shoot_settings 09b-access-log
shoot_main 10-main-large-text

# ---- 7. Large text in the settings window itself ----
press "AI agent access"
shoot_settings 13-large-text

# ---- 8. The smallest window size ----
{
  echo "# The window is shrunk to 100 x 100: the window has a minimum size (640 x 440), it does not get smaller."
  echo "size after the request: $(ax resize "$(app_pid)" 100 100)"
} > "$OUT/min-size.txt"
cat "$OUT/min-size.txt"
sleep 2
shoot_settings 14-min-size

# ---- 9. kill -9 after an edit in the window: "Large" survives ----
press "Appearance"
press "Normal"
sleep 3
press "Large"
sleep 3
[ "$(setting text_size)" = "large" ] || fail "the edit in the window was not stored"
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 2
in_main
press "Profile"
press "Settings"
wait_for_settings
sleep 2
in_settings
press "Appearance"
shoot_settings 15-after-kill9
[ "$(setting text_size)" = "large" ] || fail "after kill -9 text_size is not large"

finish
