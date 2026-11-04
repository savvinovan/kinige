#!/usr/bin/env bash
# Shared helpers of the real-window screenshot scenarios on macOS (the screens.yml workflow). Sourced from scripts/screens/<slice>.sh.
# Expects these variables from the scenario: REPO (root), K (working directory), APP ($K/Kinige.app), OUT (where to put PNGs and texts).
# Input is accessibility actions only (AXPress, AXScrollToVisible) through scripts/screens/ax.js: no mouse, no keyboard,
# no global shortcuts. The working data is synthetic, in $K/data and $K/mirror.

FAILS=0
PAUSE="${PAUSE:-1.5}"
mkdir -p "$OUT" "$K/debug"

app_pid() { pgrep -f "$APP/Contents/MacOS/kinige( |\$)" | head -1; }
# A command with a time limit (macOS has no `timeout`): a hung osascript or session must not hold the job until the hour is up.
limit() {
  local seconds="$1"
  shift
  perl -e 'alarm shift; exec @ARGV' "$seconds" "$@"
}
ax() { limit "${AX_TIMEOUT:-30}" osascript -l JavaScript "$REPO/scripts/screens/ax.js" "$@"; }

# The app with temporary data directories: LSEnvironment in a copy of the bundle, ad-hoc signature again.
prepare_app() {
  local source="$1"
  rm -rf "$APP"
  cp -R "$source" "$APP"
  plutil -insert LSEnvironment -json \
    "{\"KINIGE_DATA_DIR\":\"$K/data\",\"KINIGE_MIRROR_DIR\":\"$K/mirror\"}" "$APP/Contents/Info.plist"
  codesign --force --deep -s - "$APP"
}

launch_background() { open -g -a "$APP" --args --background; }
launch_window() { open -a "$APP"; }

wait_for_window() {
  for _ in $(seq 1 40); do
    if KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null | grep -q 'layer=0'; then
      return 0
    fi
    sleep 1
  done
  echo "the window did not appear"
  FAILS=$((FAILS + 1))
}

wait_for_process() {
  for _ in $(seq 1 40); do
    [ -n "$(app_pid)" ] && return 0
    sleep 1
  done
  echo "the process did not start"
  FAILS=$((FAILS + 1))
}

# A window tree dump takes longer than a normal action (the feed, the card and the sidebar of slice 5 are thousands of elements), hence its own limit.
dump() { AX_TIMEOUT="${DUMP_TIMEOUT:-120}" ax dump "$(app_pid)" > "$K/debug/$1.txt" 2>&1; }

# The dump is real: not empty, has element lines (`AXGroup ...`) and no trace of a traversal cut off by the time limit.
dump_complete() { [ -s "$1" ] && grep -q '^ *AX' "$1" && ! grep -q 'Alarm clock' "$1"; }

fail() {
  echo "ERROR: $*"
  FAILS=$((FAILS + 1))
  dump "fail-$FAILS"
}

press() {
  local result
  result="$(ax press "$(app_pid)" "$@" 2>&1)"
  echo "press $*: $result"
  case "$result" in
    pressed*) ;;
    *) fail "not pressed: $*" ;;
  esac
  sleep "$PAUSE"
}

# A right click without a mouse: the AX action AXShowMenu on a button (the context menu of a sidebar chat opens).
show_menu() {
  local result
  result="$(ax showmenu "$(app_pid)" "$@" 2>&1)"
  echo "showmenu $*: $result"
  case "$result" in
    "menu shown"*) ;;
    *) fail "menu not shown: $*" ;;
  esac
  sleep "$PAUSE"
}

scroll_to() {
  local result
  result="$(ax scroll "$(app_pid)" "$1" 2>&1)"
  echo "scroll $1: $result"
  case "$result" in
    scrolled*) ;;
    *) fail "not scrolled: $1" ;;
  esac
  sleep "$PAUSE"
}

has() { [ "$(ax has "$(app_pid)" "$1" 2>&1)" = "yes" ]; }

# A shot of the Kinige window only. Before it, the window text is checked for foreign paths and names (personal data must not reach the repository).
# The second argument is the scripts/screenshot-window.sh mode (main by default, settings is the Settings window); which window the
# accessibility actions read is set by the AX_WINDOW variable (part of the window title; empty means the first window).
shoot() {
  local name="$1"
  local mode="${2:-main}"
  ax front "$(app_pid)" > /dev/null 2>&1
  sleep 1
  # A scenario with a large window tree may take the picture without the full dump (SHOOT_DUMP=0): only for shots that carry no
  # new text and no screen that can show a path. Every shot that is dumped is still checked for completeness and personal paths.
  if [ "${SHOOT_DUMP:-1}" = "0" ]; then
    KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" "$OUT/$name.png" kinige "$mode" || fail "no shot $name"
    return 0
  fi
  dump "shot-$name"
  # An empty dump (the traversal did not fit the limit, `Alarm clock`) or one without element lines: grep finds nothing in it, and the check
  # would pass silently. That is a scenario error, not "clean".
  dump_complete "$K/debug/shot-$name.txt" || fail "the dump of window $name is empty or cut off (the personal path check was not done)"
  if grep -E '/Users/[^r]|/Users/r[^u]|/private/tmp|/var/folders|alexandr|savvinovan' "$K/debug/shot-$name.txt" > "$K/debug/shot-$name.private" 2>&1; then
    fail "the text of window $name has personal paths or names: $(head -c 300 "$K/debug/shot-$name.private")"
  fi
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" "$OUT/$name.png" kinige "$mode" || fail "no shot $name"
}

# Since slice 4b the settings live in their own window ("Settings"), next to the main one ("Kinige"). Accessibility actions read
# the window named by AX_WINDOW; the shot mode tells screenshot-window.sh which window to capture.
in_settings() { export AX_WINDOW=Settings; }
in_main() { export AX_WINDOW=Kinige; }
shoot_settings() { in_settings; shoot "$1" settings; }
shoot_main() { in_main; shoot "$1" main; in_settings; }
settings_windows() { KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null | awk -F'\t' '$5 == "Settings" || $5 == "Настройки"' | wc -l | tr -d ' '; }

wait_for_settings() {
  for _ in $(seq 1 30); do
    [ "$(settings_windows)" -ge 1 ] && return 0
    sleep 1
  done
  fail "settings window did not appear"
}

sql() { sqlite3 "$K/data/kinige.db" "$@"; }
chat_id() { sql "select id from chat where title = '$1'"; }

finish() {
  echo "scenario errors: $FAILS"
  [ "$FAILS" -eq 0 ]
}
