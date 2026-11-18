#!/usr/bin/env bash
# Screens of the pending attachment tiles in the composer (KINIG-135) in the real window on a macOS runner: scripts/screens/kinig-135.sh <built Kinige.app>.
# The data are synthetic (seed_demo with KINIGE_SEED_ATTACH=1 and KINIGE_SEED_TILES=1: the draft of the chat "Идеи" holds ten tiles, five
# pictures of several shapes, among them a very wide one and a dark one, and five documents) in a temporary directory; the real database is not
# touched. Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. The native file dialog cannot be driven
# that way, so the tiles come from the draft the seed wrote; the scenario removes them through the x of each tile.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/kinig-135}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The forest strip makes the window tree large (371 cells) and slows every search for an element: this scenario is about the composer, so the
# forest is hidden first (the saved view state of slice 6), like in the slice 7 scenario.
export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_ATTACH=1 KINIGE_SEED_TILES=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }
shoot_light() { SHOOT_DUMP=0 shoot "$@"; }

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
    L_IDEAS="Идеи"; L_PHOTO="Фото"; L_REMOVE="Убрать"; L_VIEWER="Просмотр фото"; L_CLOSE="Закрыть"; L_EXTERNAL="Открыть в программе по умолчанию"
  else
    L_IDEAS="Идеи"; L_PHOTO="Photo"; L_REMOVE="Remove"; L_VIEWER="Photo viewer"; L_CLOSE="Close"; L_EXTERNAL="Open in default app"
  fi
}

detect_language() {
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

tiles() { sqlw "select count(*) from draft_attachment"; }
remove_tile() {
  press "${L_REMOVE} $1"
  sleep 2
  has "${L_REMOVE} $1" && fail "the tile $1 is still on screen after its x"
}

# ---- 1. Ten tiles (the most the composer takes) inside the field's box: a vertical scroll past two lines, no horizontal one ----
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
press "${L_IDEAS}"
sleep 3
[ "$(tiles)" = "10" ] || fail "the draft of the chat does not hold ten tiles ($(tiles))"
has "${L_REMOVE} IMG_0009.jpg" || fail "no tile for IMG_0009.jpg above the field"
has "${L_REMOVE} ночь.jpg" || fail "no tile for the dark picture above the field"
shoot 01-ten-tiles

# ---- 2. Large text and 640 px ----
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot_light 02-ten-tiles-large-text
edit_settings 'd["text_size"] = "normal"'
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot_light 03-ten-tiles-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- 3. Seven tiles go: a dark picture, a picture and a document are left ----
for name in IMG_0020.jpg IMG_0021.jpg IMG_0023.jpg IMG_0024.jpg "Договор аренды 2026, финальная версия.pdf" "смета на ремонт.pdf" "расписание.pdf"; do
  remove_tile "$name"
done
[ "$(tiles)" = "3" ] || fail "three tiles should be left in the draft ($(tiles))"
shoot 04-three-tiles

# ---- 4. A click on a picture tile opens the viewer on the unsent pictures: no "Open in default app" there ----
press "${L_PHOTO} IMG_0009.jpg"
sleep 3
has "${L_VIEWER}" || fail "the viewer did not open from the tile"
has "1 / 2" || fail "the viewer does not page through the two unsent pictures"
has "${L_EXTERNAL}" && fail "the viewer of an unsent picture offers to open it in the system app"
shoot 05-viewer-from-tile
press "${L_CLOSE}"
sleep 2
has "${L_VIEWER}" && fail "the viewer is still open after Close"

# ---- 5. One picture ----
remove_tile "ночь.jpg"
remove_tile "заметки.pdf"
[ "$(tiles)" = "1" ] || fail "one tile should be left in the draft ($(tiles))"
shoot 06-one-picture

# ---- 6. English ----
set_language en
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 6
detect_language
press "${L_IDEAS}"
sleep 3
has "${L_REMOVE} IMG_0009.jpg" || fail "english: the tile did not survive kill -9"
shoot_light 07-en

finish
