#!/usr/bin/env bash
# Shots of slice 5d (KINIG-160, ADR-0022: settings only in SQLite, read and partly changed by agents through MCP) in the real window on the
# macOS runner: scripts/screens/5d.sh <built Kinige.app>. Run: gh workflow run screens.yml --ref <branch> -f slice=5d.
# The data are synthetic (seed_demo) in a temporary directory; the real database is not touched. Input is accessibility actions (ax.js) for the
# settings window, no mouse, no keyboard, no global shortcuts. This is the only scenario that uses the MCP tool: an agent (our own client,
# scripts/mcp-settings.py, through the bridge of the bundle) reads the settings, changes the text size and meets every refusal.
#
# Agent access is switched on with sqlite3 on the throwaway database, as a person would in the settings window: nothing inside the app can
# switch it on. The three access keys are put back when the scenario ends, also when it fails (the trap below).
#
# What it proves in the real window: after the agent's change the main and the settings window are in large text without a restart (03, 07), a
# refused shortcut change leaves the shortcuts as they were (02), the access log has the new rows (04), the settings page has the new line (05),
# the large text survives `kill -9` (06), and Kinige never touches a `settings.json` of the user while it removes only its own old temporary
# files (`untouched-file.txt`).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5d}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"
INBOX="00000000-0000-4000-8000-000000000001"
DB="$K/data/kinige.db"
ACCESS_KEYS="'mcp_enabled', 'mcp_mode', 'mcp_chats'"
HEX="0123456789abcdef0123456789abcdef"

sqlw() { sqlite3 -cmd '.timeout 5000' "$DB" "$@"; }
setting() { sqlw "select value from app_setting where key = '$1'"; }

# The access keys as they were before the scenario, as SQL to run again; the trap puts them back (also on a failure and on a kill).
save_access() {
  sqlw "select 'insert into app_setting(key, value) values (' || quote(key) || ', ' || quote(value) || ');' from app_setting where key in ($ACCESS_KEYS)" > "$K/debug/access-before.sql"
}
restore_access() {
  [ -f "$K/debug/access-before.sql" ] || return 0
  { echo "delete from app_setting where key in ($ACCESS_KEYS);"; cat "$K/debug/access-before.sql"; } | sqlite3 -cmd '.timeout 5000' "$DB" 2> /dev/null
}
access_on() {
  sqlw "insert or replace into app_setting(key, value) values ('mcp_enabled', '1'), ('mcp_mode', 'read_write'), ('mcp_chats', json_array('$INBOX'))"
}

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror" "$K/debug"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
trap 'restore_access; cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
save_access
prepare_app "$SOURCE_APP"

# A synthetic settings.json of "the user", an old temporary file of Kinige's own, and a look-alike that is not: planted before the first launch.
SETTINGS_JSON="$K/data/settings.json"
printf '{\n  "text_size": "normal",\n  "agent_access": {"enabled": true, "mode": "read_write"},\n  "note": "the user wrote this"\n}\n' > "$SETTINGS_JSON"
chmod 600 "$SETTINGS_JSON"
touch -t 202601020304 "$SETTINGS_JSON"
printf 'old' > "$K/data/.settings.json.$HEX.tmp"
printf 'keep' > "$K/data/.settings.json.01234567-89ab-cdef-0123-456789abcdef.tmp"
file_facts() { printf 'sha256 %s, mtime %s, bytes %s' "$(shasum -a 256 "$1" | cut -d' ' -f1)" "$(stat -f %m "$1")" "$(stat -f %z "$1")"; }
FILE_BEFORE="$(file_facts "$SETTINGS_JSON")"

access_on

# ---- 1. Launch: the settings window, Appearance with the tree ----
launch_window
wait_for_window
sleep 2
in_main
press "Profile"
press "Settings"
wait_for_settings
sleep 2
in_settings
shoot_settings 01-appearance
has "Settings file" && fail "the tree still has a Settings file item"
has "settings.json" && fail "the window mentions settings.json"
[ "$(setting text_size)" = "" ] || [ "$(setting text_size)" = "normal" ] || fail "the old file changed the text size at launch ($(setting text_size))"

# ---- 2. An agent session through the bridge: read, refuse, change the text size ----
limit 90 python3 "$REPO/scripts/mcp-settings.py" "$BRIDGE" "$SOCKET" "$OUT/mcp-session.txt" "$DB" || fail "the agent session"
sleep 4
[ "$(setting text_size)" = "large" ] || fail "the agent's change was not stored"
[ -z "$(setting shortcut_capture)" ] && [ -z "$(setting shortcut_clipboard)" ] || fail "a shortcut was stored after refused changes"
[ "$(grep -c 'setting_not_writable' "$OUT/mcp-session.txt")" -ge 6 ] || fail "the transcript does not show the refusals of the six names"
grep -q 'mcp_read_only' "$OUT/mcp-session.txt" || fail "the transcript has no read-only refusal"
grep -q 'mcp_disabled' "$OUT/mcp-session.txt" || fail "the transcript has no access-off refusal"

# ---- 3. The windows after the agent's change: no restart, no window came forward ----
in_settings
press "Keyboard shortcuts"
shoot_settings 02-shortcuts-unchanged
shoot_main 03-main-large-text
echo "Settings windows: $(settings_windows)" > "$OUT/windows.txt"

# ---- 4. The access log and the settings page ----
press "AI agent access"
shoot_settings 05-access-page
press "Access log"
has "changed a setting" || fail "the log has no \"changed a setting\" row"
has "read the settings" || fail "the log has no \"read the settings\" row"
has "denied: settings window only" || fail "the log has no \"denied: settings window only\" row"
shoot_settings 04-access-log

# ---- 5. The settings window in large text ----
press "Appearance"
shoot_settings 07-settings-large-text

# ---- 6. kill -9: the large text is in place ----
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 3
in_main
shoot_main 06-main-after-kill-9
[ "$(setting text_size)" = "large" ] || fail "after kill -9 the text size is not large"

# ---- 7. The user's settings.json was never touched; only the old temporary file went ----
FILE_AFTER="$(file_facts "$SETTINGS_JSON")"
{
  echo "# A synthetic settings.json planted before the first launch, an old temporary file of Kinige's own and a look-alike."
  echo "settings.json before: $FILE_BEFORE"
  echo "settings.json after:  $FILE_AFTER"
  if [ -e "$K/data/.settings.json.$HEX.tmp" ]; then echo ".settings.json.<32 hex>.tmp: still there"; else echo ".settings.json.<32 hex>.tmp: removed"; fi
  if [ -e "$K/data/.settings.json.01234567-89ab-cdef-0123-456789abcdef.tmp" ]; then echo "hyphenated look-alike: kept"; else echo "hyphenated look-alike: removed"; fi
  echo "text_size in the database: $(setting text_size)"
} > "$OUT/untouched-file.txt"
cat "$OUT/untouched-file.txt"
[ "$FILE_BEFORE" = "$FILE_AFTER" ] || fail "settings.json changed"
[ ! -e "$K/data/.settings.json.$HEX.tmp" ] || fail "the old temporary file was not removed"
[ -e "$K/data/.settings.json.01234567-89ab-cdef-0123-456789abcdef.tmp" ] || fail "the look-alike was removed"

finish
