#!/usr/bin/env bash
# Screens of the photo viewer (KINIG-87: close on click, zoom, rotate) in the real window on a macOS runner: scripts/screens/viewer.sh <built Kinige.app>.
# The data are synthetic (seed_demo with KINIGE_SEED_ATTACH=1, the same album of four as slice 7) in a temporary directory; the real database
# is not touched. Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. So the scenario drives the toolbar
# buttons (rotate, zoom in, the percentage, next photo, close). The Cmd/Ctrl + wheel, the keys, drag and the close by a click on the
# backdrop are pointer and keyboard input: they are covered by vitest (LightboxClose, LightboxZoom, LightboxRotate tests). Whether an AX press
# can reach the backdrop is tried once and written to backdrop-click.txt, whatever the answer.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/viewer}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The forest strip makes the window tree large (371 cells) and slows every search for an element: this scenario is about the viewer, so the
# forest is hidden first (the saved view state of slice 6), like in the slice 7 scenario.
export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_ATTACH=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

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
    L_INBOX="Входящие"; L_PHOTO="Фото"; L_VIEWER="Просмотр фото"; L_NEXT="Следующее фото"; L_CLOSE="Закрыть"
    L_ROTATE="Повернуть влево"; L_ZOOM_IN="Увеличить"; L_ZOOM_OUT="Уменьшить"; L_FIT="Вписать в окно"
  else
    L_INBOX="Inbox"; L_PHOTO="Photo"; L_VIEWER="Photo viewer"; L_NEXT="Next photo"; L_CLOSE="Close"
    L_ROTATE="Rotate left"; L_ZOOM_IN="Zoom in"; L_ZOOM_OUT="Zoom out"; L_FIT="Fit to window"
  fi
}

detect_language() {
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

open_viewer() {
  press "${L_INBOX}"
  sleep 3
  scroll_to "${L_PHOTO} IMG_0002.jpg"
  sleep 3
  press "${L_PHOTO} IMG_0002.jpg"
  sleep 3
  has "${L_VIEWER}" || fail "the photo viewer did not open"
  has "2 / 4" || fail "the viewer does not say 2 / 4"
}

: > "$OUT/checks.txt"
check() { echo "$1" >> "$OUT/checks.txt"; }

# ---- 1. The viewer at fit size with the new toolbar ----
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
if ! has "${L_PHOTO} IMG_0002.jpg"; then
  fail "no photo of the album in the feed"
  shoot_light 00-failed-first-look
  finish
  exit 1
fi
open_viewer
for name in "${L_ROTATE}" "${L_ZOOM_OUT}" "${L_ZOOM_IN}" "${L_FIT}"; do
  has "$name" || fail "the toolbar has no button '$name'"
done
has "100%" || fail "the viewer does not start at 100%"
check "fit: 100%, toolbar buttons present: ${L_ROTATE}, ${L_ZOOM_OUT}, ${L_FIT}, ${L_ZOOM_IN}"
shoot 01-viewer-fit-toolbar

# ---- 2. Rotated through the button: the photo turns a quarter counter-clockwise and fits the height ----
press "${L_ROTATE}"
sleep 2
shoot_light 02-rotated

# ---- 3. Zoomed through the button (150%, then 200%) in the turned frame ----
press "${L_ZOOM_IN}"
sleep 1
has "150%" || fail "zoom in did not show 150%"
press "${L_ZOOM_IN}"
sleep 2
has "200%" || fail "zoom in did not show 200%"
check "rotated once, zoomed twice: 200%"
shoot_light 03-rotated-zoomed

# ---- 4. Large text with the viewer open: the toolbar grows, the percentage keeps its width ----
edit_settings 'd["text_size"] = "large"'
sleep 2
has "200%" || fail "the viewer lost its zoom when the text size changed"
shoot_light 04-large-text-rotated-zoomed
edit_settings 'd["text_size"] = "normal"'
sleep 2

# ---- 5. The percentage resets to fit; another photo starts upright and at fit ----
press "${L_FIT}"
sleep 2
has "100%" || fail "the percentage button did not reset the zoom to fit"
press "${L_ZOOM_IN}"
sleep 1
press "${L_NEXT}"
sleep 3
has "3 / 4" || fail "the next photo did not show 3 / 4"
has "100%" || fail "zoom was not reset on the next photo"
check "next photo: 3 / 4, back at 100% and upright"
shoot_light 05-next-photo-fit

# ---- 6. Closing. A click on the backdrop is pointer input: one AX press on the dialog itself is tried and its answer is kept ----
BACKDROP="$(ax press "$(app_pid)" "${L_VIEWER}" Group 2>&1 | head -c 200)"
sleep 2
if has "${L_VIEWER}"; then
  echo "AX press on the dialog (the backdrop): '${BACKDROP}'. The viewer stayed open: an AX press is not a pointer click, closing by a click on the backdrop and on the photo is covered by vitest (LightboxClose.test.tsx)." > "$OUT/backdrop-click.txt"
else
  echo "AX press on the dialog (the backdrop): '${BACKDROP}'. The viewer closed." > "$OUT/backdrop-click.txt"
fi
cat "$OUT/backdrop-click.txt"
if has "${L_VIEWER}"; then
  press "${L_CLOSE}"
  sleep 2
fi
has "${L_VIEWER}" && fail "the viewer is still open after closing"
shoot_light 06-feed-after-close

# ---- 7. English: the names and the toolbar ----
set_language en
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 6
detect_language
open_viewer
press "${L_ROTATE}"
press "${L_ZOOM_IN}"
sleep 2
has "150%" || fail "english: zoom in did not show 150%"
shoot_light 07-en-rotated-zoomed
press "${L_CLOSE}"
sleep 2

finish
