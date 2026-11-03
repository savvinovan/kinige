#!/usr/bin/env bash
# Shots of slice 4b (the Settings window and the settings.json file) in the real window on the macOS runner: scripts/screens/4b.sh <built Kinige.app>.
# The data is synthetic (seed_demo) in a temporary directory; the real database is not touched. The result is $OUT (docs/screens/4b).
# Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. The settings file is edited
# by the scenario itself (as an "editor" or an agent): a temporary file next to it and mv, the way editors do.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/4b}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SETTINGS_FILE="$K/data/settings.json"
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

# Editing the file "from outside": a temporary file and mv (mode 0600). The argument is the content.
write_settings() {
  printf '%s' "$1" > "$K/data/.editor.tmp"
  chmod 600 "$K/data/.editor.tmp"
  mv "$K/data/.editor.tmp" "$SETTINGS_FILE"
  sleep 4
}

# Editing the file through python: the code changes the dictionary d.
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

# ---- 6. A settings file without errors; the file before and after an edit from the window ----
press "Settings file"
shoot_settings 08-file-ok
{
  echo "# settings.json: a synthetic file on the runner. The file permissions (ls -l) and the content."
  echo "== as the app wrote it =="
  ls -l "$SETTINGS_FILE"
  cat "$SETTINGS_FILE"
} > "$OUT/settings-file.txt"
# A foreign key in the file (an "agent" added a note): the app keeps it even after an edit from the window.
edit_settings 'd["x_my_note"] = "keep me"'
press "Appearance"
press "Large"
sleep 3
{
  echo
  echo "== a foreign key x_my_note was added to the file; \"Large\" was selected in the window =="
  ls -l "$SETTINGS_FILE"
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
grep -q '"x_my_note": "keep me"' "$SETTINGS_FILE" && grep -q '"text_size": "large"' "$SETTINGS_FILE" || fail "the edit from the window lost the foreign key or was not written"
[ "$(stat -f %Lp "$SETTINGS_FILE")" = "600" ] || fail "the file mode is not 0600"
press "Normal"
sleep 3
edit_settings 'd.pop("x_my_note", None)'

# ---- 7. An external edit: the text size and the chat "Работа" in the agent access ----
cp "$SETTINGS_FILE" "$K/debug/settings-good.json"
edit_settings 'd["text_size"] = "large"; d["agent_access"]["chats"].append({"title": "Работа"})'
{
  echo
  echo "== an external edit: text_size = large, the chat \"Работа\" was added to agent_access.chats (by title) =="
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
[ "$(setting text_size)" = "large" ] || fail "text_size from the file was not applied"
sql "select value from app_setting where key = 'mcp_chats'" | grep -q "$WORK" || fail "the chat from the file is not marked"
press "AI agent access"
has "Changed via settings.json" || fail "no \"Changed via settings.json\" line"
shoot_settings 09-after-external-edit
press "Access log"
has "access changed" || fail "the log has no \"settings.json · access changed\" line"
shoot_settings 09b-access-log-after-edit
shoot_main 10-main-large-text

# ---- 8. The file cannot be parsed ----
write_settings '{"text_size": '
press "Settings file"
has "could not be read" || fail "no \"file could not be read\" message"
shoot_settings 11-file-unreadable
[ "$(setting text_size)" = "large" ] || fail "the broken file changed the settings"

# ---- 9. Errors of several keys ----
write_settings '{"text_size":"huge","shortcuts":{"capture":"???"},"mirror":{"folder":"relative/path"},"agent_access":{"mode":"write","chats":[{"id":"no-such-chat"}]},"surprise":1}'
has "text_size: this value is not allowed" || fail "no text_size error"
has "shortcuts.capture: this shortcut cannot be used" || fail "no shortcut error"
has "mirror.folder: the folder path must be absolute" || fail "no mirror folder error"
has "agent_access.mode: this value is not allowed" || fail "no mode error"
has "agent_access.chats[0]: no such chat" || fail "no chat error"
has "surprise" || fail "no warning about the unknown key"
shoot_settings 12-file-key-errors
[ "$(setting text_size)" = "large" ] || fail "the key errors changed the settings"
write_settings "$(cat "$K/debug/settings-good.json" | python3 -c 'import json,sys; d=json.load(sys.stdin); d["text_size"]="large"; print(json.dumps(d))')"

# ---- 10. Large text ----
press "AI agent access"
shoot_settings 13-large-text

# ---- 11. The smallest window size ----
{
  echo "# The window is shrunk to 100 x 100: the window has a minimum size (640 x 440), it does not get smaller."
  echo "size after the request: $(ax resize "$(app_pid)" 100 100)"
} > "$OUT/min-size.txt"
cat "$OUT/min-size.txt"
sleep 2
shoot_settings 14-min-size

# ---- 12. kill -9 after an edit in the window and after an edit of the file ----
press "Appearance"
press "Normal"
sleep 3
edit_settings 'd["text_size"] = "large"'
kill -9 "$(app_pid)"
sleep 2
{
  echo
  echo "== kill -9 after an edit in the window (Normal) and an edit of the file (large) =="
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
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
{
  echo
  echo "== after the restart =="
  cat "$SETTINGS_FILE"
  echo "text_size in the database: $(setting text_size)"
  echo "agent access: $(setting mcp_enabled), mode: $(setting mcp_mode), chats: $(setting mcp_chats | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')"
} >> "$OUT/settings-file.txt"
[ "$(setting text_size)" = "large" ] || fail "after kill -9 text_size is not large"

finish
