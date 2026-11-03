#!/usr/bin/env bash
# Shots of slice 4a ("AI agent access", MCP) in the real window on the macOS runner: scripts/screens/4a.sh <built Kinige.app>.
# The data is synthetic (seed_demo) in a temporary directory; the real database is not touched. The result is $OUT (docs/screens/4a).
# Since slice 4b the access settings live in the separate "Settings" window (tree: "AI agent access", "Visible chats",
# "Access log"), not in a dialog of the main window; steps 2-9 drive that window the way 4b.sh does.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/4a}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"
EVIDENCE="$OUT/background-launch.txt"

# Old shots are removed entirely: the names and the set may have changed.
rm -f "$OUT"/*.png
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
WORK="$(chat_id 'Работа')"
BOOKS="$(chat_id 'Книги')"
INBOX="00000000-0000-4000-8000-000000000001"

dock_has_kinige() {
  limit 30 osascript -e 'tell application "System Events" to get name of every UI element of list 1 of process "Dock"' 2>&1 | grep -qw Kinige
}
app_type() { lsappinfo info -only ApplicationType "$(lsappinfo find pid="$(app_pid)")" 2>&1 | sed 's/.*=//'; }

# ---- 1. Background launch, the way the bridge does it: no Dock icon and no window; the main socket; then showing the window ----
{
  echo "# Background launch: open -g -a Kinige.app --args --background, a synthetic data directory on the GitHub runner (macOS)."
  launch_background
  wait_for_process
  sleep 4
  echo "after --background: ApplicationType=$(app_type)  (UIElement = Accessory policy, no Dock icon)"
  if dock_has_kinige; then echo "Dock: the Kinige icon IS there"; fail "the Kinige icon is in the Dock in the background"; else echo "Dock: no Kinige icon"; fi
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list | grep -c 'layer=0' | sed 's/^/windows of the normal layer: /'
  # The menu bar icon: the process's second menu bar (status items) through System Events and the icon window (layer 25) among the process windows.
  items="$(limit 30 osascript -e "tell application \"System Events\" to tell (first process whose unix id is $(app_pid)) to get count of menu bar items of menu bar 2" 2>&1)"
  windows="$(KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list | grep -c 'layer=25')"
  echo "menu bar icon: items in the process's second menu bar: $items; icon windows (layer 25): $windows"
  case "$items" in
    '' | *[!0-9]*) items=0 ;;
  esac
  if [ "$items" -ge 1 ] || [ "$windows" -ge 1 ]; then echo "the menu bar icon is there"; else fail "no menu bar icon in the background"; fi
  echo "the main socket (not the fallback in TMPDIR):"
  stat -f '%Sp  %N' "$K/data/mcp" "$SOCKET"
  [ "$(stat -f %Lp "$SOCKET")" = "600" ] && [ "$(stat -f %Lp "$K/data/mcp")" = "700" ] || fail "socket permissions"
  limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$K/debug/session-disabled.txt" || fail "session over the main socket"
  grep -o 'mcp_disabled[^"]*' "$K/debug/session-disabled.txt" | head -1 | sed 's/^/call while access is off: /'
  echo "process: $(ps -o command= -p "$(app_pid)" | sed 's#.*/Contents#.../Contents#')"
  launch_window
  wait_for_window
  sleep 2
  echo "after showing the window (open without arguments, Reopen): ApplicationType=$(app_type)  (Foreground = Regular, Dock icon)"
  if dock_has_kinige; then echo "Dock: the Kinige icon is there"; else echo "Dock: no Kinige icon"; fail "no Dock icon while the window is shown"; fi
} > "$EVIDENCE" 2>&1
cat "$EVIDENCE"

# ---- 2. Profile -> Settings -> "AI agent access": access is off, the log shows the refusals of the first step ----
open_settings() {
  in_main
  press "Profile"
  press "Settings"
  wait_for_settings
  sleep 2
  in_settings
}
open_settings
press "AI agent access"
shoot_settings 01-access-off
press "Access log"
has "denied: access is off" || fail "the log has no refusal lines while access is off"
shoot_settings 02-access-off-log
press "AI agent access"
scroll_to "How to connect"
has "$SOCKET" && echo "the main socket path is visible in the window" || fail "the main socket path is not in the window text ($SOCKET)"

# ---- 3. Turning on: the warning, the mode, the visible chats ----
press "Allow agents to access the journal"
shoot_settings 03-warning
press "Turn on"
press "Read and write"
press "Configure"
press "Работа" CheckBox
press "Inbox" CheckBox
shoot_settings 04-access-on

# ---- 4. An agent session through the bridge from the bundle: reading, writing, a closed chat ----
limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$OUT/mcp-session.txt" --chat "$WORK" --closed-chat "$BOOKS" || fail "agent session"
sleep 4
shoot_settings 05-visible-chats-after-agent
press "Access log"
shoot_settings 06-access-log
press "Show entries of this request"
shoot_settings 07-access-log-expanded

# ---- 5. Read-only mode: write calls are refused ----
press "AI agent access"
press "Read only"
limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$OUT/mcp-session-read-only.txt" --chat "$WORK" || fail "read-only session"
sleep 4
press "Access log"
shoot_settings 08-access-log-denied
press "AI agent access"
press "Read and write"

# ---- 6. The feed: the agent's entry ----
in_main
# Chats in the sidebar are an AXPopUpButton with the title and the entry count ("Работа , entries: 2").
press "Работа , entries" PopUpButton
shoot_main 09-feed-agent-entry

# ---- 7. kill -9 and a restart: the data is in place, the old socket is replaced ----
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 2
in_main
press "Работа , entries" PopUpButton
shoot_main 10-after-kill9-restart
open_settings
press "AI agent access"
press "Access log"
shoot_settings 11-access-log-after-kill9

# ---- 8. A chat excluded from AI (written to the database directly: there is no switch in the interface) ----
sql "update chat set ai_excluded = 1 where id = '$BOOKS'"
sleep 4
press "Visible chats"
has "excluded from AI" || fail "the chat list has no \"excluded from AI\" mark"
shoot_settings 15-chat-excluded-from-ai

# ---- 9. Large text: settings, visible chats, the log ----
press "Appearance"
press "Large"
shoot_settings 12-settings-large-text
press "AI agent access"
press "Visible chats"
shoot_settings 13-access-large-text
press "Access log"
shoot_settings 14-access-log-large-text

finish
