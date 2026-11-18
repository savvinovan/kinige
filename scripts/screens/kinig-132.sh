#!/usr/bin/env bash
# Screens of KINIG-132 (pictures without a frame, the time on the picture) in the real window on a macOS runner:
# scripts/screens/kinig-132.sh <built Kinige.app>.
# The data are synthetic (seed_demo with KINIGE_SEED_ATTACH=1, the same Inbox as slice 7: an album of four with a caption, a portrait
# photo with no text, a document, a mixed entry) in a temporary directory; the real database is not touched. Input is accessibility
# actions only (ax.js): no mouse, no keyboard, no global shortcuts. A real click on the picture under the pill is pointer input: the pill
# has `pointer-events: none` (checked in message-bleed.css and by vitest), and here an AX press on the picture shows the viewer still opens.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/kinig-132}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The forest strip makes the window tree large and slows every search for an element: this scenario is about the feed, so the forest
# is hidden first (the saved view state of slice 6), like in the slice 7 scenario.
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

L_PHOTO="Фото"
L_VIEWER="Просмотр фото"
L_CLOSE="Закрыть"
ALBUM="${L_PHOTO} IMG_0002.jpg"
ALONE="${L_PHOTO} IMG_0005.jpg"
CAPTION="Прогулка вдоль реки"

# The album first, so that its pictures load, then its caption, so that the whole entry, caption and time included, is in the shot.
show_album() {
  scroll_to "$ALBUM"
  sleep 3
  scroll_to "$CAPTION"
  sleep 3
}

# ---- 1. A picture with no text: it fills the bubble, the time is a pill on it ----
set_language ru
launch_window
wait_for_window
sleep 6
if ! has "$ALONE"; then
  fail "no picture-only entry in the feed"
  shoot_light 00-failed-first-look
  finish
  exit 1
fi
scroll_to "$ALONE"
sleep 4
shoot 01-picture-only

# ---- 2. Pictures with a caption: the album edge to edge at the top, the text under it, the time in the footer ----
show_album
shoot 02-pictures-with-text

# ---- 3. The picture is still the button that opens the viewer ----
press "$ALONE"
sleep 3
has "${L_VIEWER}" || fail "the viewer did not open from the picture with the pill on it"
shoot_light 03-viewer-opens
press "${L_CLOSE}"
sleep 2
has "${L_VIEWER}" && fail "the viewer is still open after Close"

# ---- 4. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
scroll_to "$ALONE"
sleep 4
shoot 04-large-text-picture-only
show_album
shoot_light 05-large-text-pictures-with-text
edit_settings 'd["text_size"] = "normal"'

# ---- 5. A 640 px window ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
scroll_to "$ALONE"
sleep 3
shoot_light 06-narrow-640-picture-only
show_album
shoot_light 07-narrow-640-pictures-with-text
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

finish
