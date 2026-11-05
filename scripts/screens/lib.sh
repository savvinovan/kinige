#!/usr/bin/env bash
# Shared helpers of the real-window screenshot scenarios on macOS (the screens.yml workflow). Sourced from scripts/screens/<slice>.sh.
# Expects these variables from the scenario: REPO (root), K (working directory), APP ($K/Kinige.app), OUT (where to put PNGs and texts).
# Input is accessibility actions only (AXPress, AXScrollToVisible) through scripts/screens/ax.js: no mouse, no keyboard,
# no global shortcuts. A scenario that sets DRIVER=1 (before sourcing this file) uses the screens driver instead (KINIG-92, ADR-0016): the
# app, built with it, finds the element by its accessible name inside its own web view and clicks it; `press`, `show_menu`, `scroll_to`,
# `has`, `shoot` and `fail` then go through `drive`. AX stays for what the web view cannot reach (the menu bar, resizing the window).
# The working data is synthetic, in $K/data and $K/mirror.

FAILS=0
PAUSE="${PAUSE:-1.5}"
DRIVER="${DRIVER:-}"
DRIVE_DIR="${KINIGE_SCREENS_DRIVER_DIR:-$K/driver}"
DRIVE_TIMEOUT="${DRIVE_TIMEOUT:-20}"
DRIVE_SEQ=0
DRIVE_REPLY=""
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
  if [ "$DRIVER" = "1" ]; then
    rm -rf "$DRIVE_DIR"
    mkdir -p "$DRIVE_DIR"
    plutil -insert LSEnvironment.KINIGE_SCREENS_DRIVER_DIR -string "$DRIVE_DIR" "$APP/Contents/Info.plist"
  fi
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

# ---- The screens driver (KINIG-92, ADR-0016) ----
# A step is a request file the app picks up and a reply file it writes; the page does the work (scripts/screens/driver.ts). A reply that
# does not come within DRIVE_TIMEOUT seconds, or that says ok:false, is a scenario error naming the step. No AX call and no window dump.

# Which window a driver step goes to, from AX_WINDOW (the same variable that selects the AX window): "Settings" or "Настройки" is the
# settings window, anything else the main one.
drive_window() {
  case "${AX_WINDOW:-}" in
    Settings* | Настройки*) echo settings ;;
    *) echo main ;;
  esac
}

# JSON text of a string for a request (names are labels: no control characters).
json_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

# The AX role names of ax.js, as ARIA roles.
driver_role() {
  case "$1" in
    Button | PopUpButton | MenuButton) echo button ;;
    CheckBox) echo checkbox ;;
    RadioButton) echo radio ;;
    MenuItem) echo menuitem ;;
    *) printf '%s' "$1" | tr 'A-Z' 'a-z' ;;
  esac
}

# `"name":"...","role":"..."` for a step: the text is a substring of an accessible name, the optional second argument a role.
driver_query() {
  local query="\"name\":\"$(json_escape "$1")\""
  [ -n "${2:-}" ] && query="$query,\"role\":\"$(driver_role "$2")\""
  printf '%s' "$query"
}

# drive_raw <window> <step json> [seconds]: sends the step and waits for the reply. DRIVE_REPLY holds the reply file. Returns 0 for
# ok:true, 1 for ok:false (DRIVE_ERROR holds the page's words), 2 for no reply in time. It never calls `fail`.
drive_raw() {
  local window="$1" step="$2" ticks id requests
  ticks=$((${3:-$DRIVE_TIMEOUT} * 20))
  DRIVE_SEQ=$((DRIVE_SEQ + 1))
  id="d$$-$DRIVE_SEQ"
  DRIVE_REPLY=""
  DRIVE_ERROR=""
  requests="$DRIVE_DIR/requests"
  mkdir -p "$requests" "$DRIVE_DIR/replies"
  # A temporary name the app skips, then a rename: the app never sees half a request.
  printf '{"id":"%s","window":"%s","step":%s}' "$id" "$window" "$step" > "$requests/.$id.tmp"
  mv "$requests/.$id.tmp" "$requests/$id.json"
  while [ ! -f "$DRIVE_DIR/replies/$id.json" ]; do
    if [ "$ticks" -le 0 ]; then
      rm -f "$requests/$id.json"
      DRIVE_ERROR="no reply within ${3:-$DRIVE_TIMEOUT} s"
      return 2
    fi
    sleep 0.05
    ticks=$((ticks - 1))
  done
  DRIVE_REPLY="$(cat "$DRIVE_DIR/replies/$id.json")"
  case "$DRIVE_REPLY" in
    *'"ok":true'*) return 0 ;;
  esac
  DRIVE_ERROR="$(printf '%s' "$DRIVE_REPLY" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("error",""))' 2> /dev/null)"
  return 1
}

# The "value" of the last reply, as plain text.
drive_value() { printf '%s' "$DRIVE_REPLY" | python3 -c 'import json,sys; sys.stdout.write(json.load(sys.stdin).get("value",""))'; }

# drive <window> <step json> [seconds]: like drive_raw, but a failure is a scenario error (`fail`) naming the step and the reason.
drive() {
  drive_raw "$@"
  local status=$?
  [ "$status" -eq 0 ] || fail "driver step $2 for $1: $DRIVE_ERROR"
  return "$status"
}

# The text the window shows (page text, labels, titles), written to a file; for the personal-path check and for the debug record.
driver_text() {
  local window="$1" file="$2"
  drive_raw "$window" '{"kind":"text"}' "${3:-$DRIVE_TIMEOUT}" || return 1
  drive_value > "$file"
}

fail() {
  echo "ERROR: $*"
  FAILS=$((FAILS + 1))
  if [ "$DRIVER" = "1" ]; then
    driver_text "$(drive_window)" "$K/debug/fail-$FAILS.txt" 5 > /dev/null 2>&1 || true
  else
    dump "fail-$FAILS"
  fi
}

# `press <text> [role]`: an element whose accessible name holds the text (AX: a button, menu item, check box, radio button).
press() {
  local result
  if [ "$DRIVER" = "1" ]; then
    if drive "$(drive_window)" "{\"kind\":\"press\",$(driver_query "$@")}"; then result=pressed; else result="$DRIVE_ERROR"; fi
    echo "press $*: $result"
    sleep "$PAUSE"
    return 0
  fi
  result="$(ax press "$(app_pid)" "$@" 2>&1)"
  echo "press $*: $result"
  case "$result" in
    pressed*) ;;
    *) fail "not pressed: $*" ;;
  esac
  sleep "$PAUSE"
}

# A right click without a mouse: the AX action AXShowMenu on a button, or the driver's `contextmenu` event (the context menu of a sidebar
# chat opens).
show_menu() {
  local result
  if [ "$DRIVER" = "1" ]; then
    if drive "$(drive_window)" "{\"kind\":\"menu\",$(driver_query "$@")}"; then result="menu shown"; else result="$DRIVE_ERROR"; fi
    echo "showmenu $*: $result"
    sleep "$PAUSE"
    return 0
  fi
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
  if [ "$DRIVER" = "1" ]; then
    if drive "$(drive_window)" "{\"kind\":\"scroll\",$(driver_query "$1")}"; then result=scrolled; else result="$DRIVE_ERROR"; fi
    echo "scroll $1: $result"
    sleep "$PAUSE"
    return 0
  fi
  result="$(ax scroll "$(app_pid)" "$1" 2>&1)"
  echo "scroll $1: $result"
  case "$result" in
    scrolled*) ;;
    *) fail "not scrolled: $1" ;;
  esac
  sleep "$PAUSE"
}

# Whether the window has an element or text with these words; no scenario error either way.
has() {
  if [ "$DRIVER" = "1" ]; then
    drive_raw "$(drive_window)" "{\"kind\":\"has\",$(driver_query "$1")}" && [ "$(drive_value)" = "yes" ]
    return
  fi
  [ "$(ax has "$(app_pid)" "$1" 2>&1)" = "yes" ]
}

# wait_name <text> [timeout ms] [gone] [role]: with the driver, waits inside the page (a poll every 50 ms) until the element shows or, with
# `gone`, disappears. Returns 1 on a timeout, without a scenario error. Without the driver it polls `has` every second.
wait_name() {
  local name="$1" timeout_ms="${2:-10000}" gone="${3:-}" role="${4:-}" gone_json="" query waited=0
  if [ "$DRIVER" = "1" ]; then
    [ "$gone" = "gone" ] && gone_json=',"gone":true'
    query="$(driver_query "$name" "$role")"
    drive_raw "$(drive_window)" "{\"kind\":\"wait\",$query$gone_json,\"timeoutMs\":$timeout_ms}" $((timeout_ms / 1000 + 10))
    return
  fi
  while [ "$waited" -lt $((timeout_ms / 1000)) ]; do
    if [ "$gone" = "gone" ]; then has "$name" || return 0; else has "$name" && return 0; fi
    sleep 1
    waited=$((waited + 1))
  done
  return 1
}

# Personal paths and names that must never reach a screenshot of the repository: another user's home, runner temp paths, my names.
PERSONAL_PATTERN='/Users/[^r]|/Users/r[^u]|/private/tmp|/var/folders|alexandr|savvinovan'

# A shot of the Kinige window only. Before it, the window text is checked for foreign paths and names (personal data must not reach the repository).
# The second argument is the scripts/screenshot-window.sh mode (main by default, settings is the Settings window); which window the
# accessibility actions read is set by the AX_WINDOW variable (part of the window title; empty means the first window).
shoot() {
  local name="$1"
  local mode="${2:-main}"
  if [ "$DRIVER" = "1" ]; then
    drive_raw "$(drive_window)" '{"kind":"front"}' > /dev/null
    sleep 1
    # The text through the driver (about 100 ms, where an AX dump takes up to 3 minutes): every shot is checked, none is skipped.
    if ! driver_text "$(drive_window)" "$K/debug/shot-$name.txt"; then
      fail "no window text for $name through the driver (the personal path check was not done): $DRIVE_ERROR"
    elif [ ! -s "$K/debug/shot-$name.txt" ]; then
      fail "the text of window $name is empty (the personal path check was not done)"
    elif grep -E "$PERSONAL_PATTERN" "$K/debug/shot-$name.txt" > "$K/debug/shot-$name.private" 2>&1; then
      fail "the text of window $name has personal paths or names: $(head -c 300 "$K/debug/shot-$name.private")"
    fi
    KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" "$OUT/$name.png" kinige "$mode" || fail "no shot $name"
    return 0
  fi
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
  if grep -E "$PERSONAL_PATTERN" "$K/debug/shot-$name.txt" > "$K/debug/shot-$name.private" 2>&1; then
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
