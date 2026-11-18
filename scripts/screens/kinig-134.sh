#!/usr/bin/env bash
# Screens of the forest switch (KINIG-134) in the real window on a macOS runner: scripts/screens/kinig-134.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=kinig-134 (the slice name is lower case, so the pictures go to docs/screens/kinig-134/).
# The data are synthetic (seed_demo with KINIGE_SEED_FOREST=1) in a temporary directory; the real database is not touched.
# Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. The switch is the button "Лес" / "Forest" at the
# right end of the top bar; the profile menu item is the other way to the same setting. The window tree with the strip is large, so only
# the shots with new text are dumped in full (the same arrangement as scripts/screens/6.sh).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/kinig-134}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=150
export DUMP_TIMEOUT=420

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_FOREST=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

shoot_light() { SHOOT_DUMP=0 shoot "$@"; }
sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }
forest_view() { sqlw "select coalesce(value, '') from app_setting where key = 'forest_view'"; }

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
  if [ "$UI" = "ru" ]; then
    L_TOGGLE="Лес"; L_EXPAND="Развернуть лес"; L_PROFILE="Профиль"; L_HIDE="Скрыть лес"; L_SHOW="Показать лес"
  else
    L_TOGGLE="Forest"; L_EXPAND="Expand forest"; L_PROFILE="Profile"; L_HIDE="Hide forest"; L_SHOW="Show forest"
  fi
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
  sleep 4
  detect_language
}

# The switch is pressed by role: "Forest" is also the name of the region and of a radio button of the expanded header. WebKit reports a
# button with `aria-pressed` as AXCheckBox (value 1 = pressed = shown), not AXButton (the first run of this scenario pressed "Button" and found nothing).
toggle() { press "${L_TOGGLE}" CheckBox; sleep 2; }

# ---- 1. The forest is shown (the default): the switch at the top right, the strip below it ----
set_language ru
launch_window
wait_for_window
sleep 5
detect_language
has "${L_EXPAND}" || fail "no strip: the forest is not shown at the start"
has "${L_TOGGLE}" || fail "no forest switch in the window"
shoot 01-forest-shown

# ---- 2. The switch hides it; the setting is stored ----
toggle
[ "$(forest_view)" = "hidden" ] || fail "forest_view is not 'hidden' after the switch"
has "${L_EXPAND}" && fail "the strip is still on screen after the switch"
shoot 02-forest-hidden

# ---- 3. The same switch, in the same place, shows it again ----
toggle
[ "$(forest_view)" = "strip" ] || fail "forest_view is not 'strip' after the second press"
has "${L_EXPAND}" || fail "no strip after the second press"
shoot_light 03-shown-again-by-the-button

# ---- 4. The profile menu is the other way to the same setting: it hides, the switch is still there ----
press "${L_PROFILE}"
sleep 1
press "${L_HIDE}" MenuItem
sleep 3
[ "$(forest_view)" = "hidden" ] || fail "forest_view is not 'hidden' after the profile menu item"
has "${L_TOGGLE}" || fail "the switch is gone while the forest is hidden"
shoot_light 04-hidden-from-the-menu

# ---- 5. kill -9: still hidden, the switch is there and brings it back ----
restart_app
has "${L_EXPAND}" && fail "the forest came back after kill -9 although it was hidden"
has "${L_TOGGLE}" || fail "no switch after kill -9"
shoot_light 05-after-kill9-still-hidden
toggle
has "${L_EXPAND}" || fail "no strip after the switch that followed kill -9"
# And the menu follows the button: the open profile menu offers "Hide forest" (the picture shows it; the unit tests assert it).
press "${L_PROFILE}"
sleep 1
shoot_light 06-menu-offers-hide

# ---- 7. English ----
set_language en
restart_app
shoot 07-en-shown
toggle
shoot 08-en-hidden

# ---- 9. Large text and a 640 px window ----
toggle
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot_light 09-large-text
edit_settings 'd["text_size"] = "normal"'
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot_light 10-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

finish
