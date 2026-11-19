#!/usr/bin/env bash
# Screens of KINIG-132 (pictures without a frame, the time on the picture) in the real window on a macOS runner:
# scripts/screens/kinig-132.sh <built Kinige.app>.
# The data are synthetic (seed_demo with KINIGE_SEED_ATTACH=1, the same Inbox as slice 7: an album of four with a caption, a portrait
# photo with no text, a document, a mixed entry) in a temporary directory; the real database is not touched. Input is accessibility
# actions only (ax.js): no mouse, no keyboard, no global shortcuts. A real click on the picture under the pill is pointer input: the pill
# has `pointer-events: none` (the rule is checked by web/src/ui/message/messageBleedCss.test.ts), and here an AX press on the picture shows the
# viewer still opens.
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
export AX_TIMEOUT=120
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_ATTACH=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
# The Inbox keeps its newest KEEP entries (the pictures are the newest the seed writes) and the older ones move to an archived chat before the
# app starts: ax.js visits an element at a time, and an element late in the tree of the whole Inbox (21 entries, three of them albums with
# pictures) was not reached within AX_TIMEOUT on the first run (a timeout reads as "not found"; the run took over an hour). A guard makes a seed that no longer
# has the album among the newest entries fail before the app is launched.
INBOX="00000000-0000-4000-8000-000000000001"
KEEP=10
ARCHIVED="$(sqlite3 "$K/data/kinige.db" "select id from chat where archived_at is not null and id != '$INBOX' limit 1")"
[ -n "${ARCHIVED}" ] || { echo "ERROR: no archived chat in the seed"; exit 1; }
sqlite3 "$K/data/kinige.db" "update message set chat_id = '$ARCHIVED' where chat_id = '$INBOX' and deleted_at is null and id not in (select id from message where chat_id = '$INBOX' and deleted_at is null order by journal_date desc, created_at desc, id desc limit $KEEP)"
KEPT_ALBUM="$(sqlite3 "$K/data/kinige.db" "select count(*) from message where chat_id = '$INBOX' and deleted_at is null and body_md like 'Прогулка вдоль реки%'")"
[ "${KEPT_ALBUM}" = "1" ] || { echo "ERROR: the album entry is not among the ${KEEP} newest entries of the Inbox"; exit 1; }
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
ALONE="${L_PHOTO} IMG_0005.jpg"
CAPTION="Прогулка вдоль реки"
# The entry just above the album in the feed (text of the seed's "Купить билеты на субботу и позвонить маме.").
BEFORE="позвонить маме"
FIT_LOG="$OUT/fit.txt"
mkdir -p "$OUT"
: > "$FIT_LOG"

# "x y width height" of an element, or nothing when it is not found.
frame_of() {
  local text
  text="$(ax frame "$(app_pid)" "$1" 2>&1)"
  case "$text" in
    *[!0-9\ -]* | "") echo "" ;;
    *) echo "$text" ;;
  esac
}

# Whether the album entry is whole in the window: the top of its first picture is below the top of the feed and the caption ends above the
# composer. The frames are read from the window tree (screen coordinates); the search field marks the top of the window, the field above
# which the feed ends marks the bottom.
album_whole() {
  local top caption search composer
  top="$(frame_of "${L_PHOTO} IMG_0001.jpg")"
  caption="$(frame_of "$CAPTION")"
  search="$(frame_of "Поиск")"
  composer="$(frame_of "Написать заметку")"
  echo "frames: first picture [$top] caption [$caption] search [$search] composer [$composer]" >> "$FIT_LOG"
  [ -n "$top" ] && [ -n "$caption" ] && [ -n "$search" ] && [ -n "$composer" ] || return 1
  local f
  read -r -a f <<< "$top $caption $search $composer"
  # Indexes: 1 first picture y; 5 and 7 caption y and height; 9 search y; 13 composer y.
  # The composer's text is the middle of its field: the feed ends about 50 px above it (first run: a caption that ended 13 px above the text was
  # behind the composer, although the check allowed it), and the bubble ends about 12 px under the caption, so 65.
  [ "${f[1]}" -ge $((f[9] - 15)) ] && [ $((f[5] + f[7])) -le $((f[13] - 65)) ]
}

# Brings the whole album entry (pictures, caption, time) into view. What the runs showed of a scroll-to-visible: an element that is out of
# view (or behind the composer) is put in the middle of the window; one that is partly visible is moved only as far as needed, to the
# nearest edge; one that is whole in the window does not move the feed. So the first picture and the caption are asked for in turn: the
# picture centres the album's first row, the caption then centres the last line (the first picture ends up partly above the window), and
# the picture, now partly visible, is brought to the top edge of the feed with the caption well above the composer. A small window fits
# after one step. The candidates are tried until the entry fits; what was tried and what the frames were is written to fit.txt, whatever
# the result.
show_album() {
  local candidate
  for candidate in "$CAPTION" "${L_PHOTO} IMG_0001.jpg" "$CAPTION" "${L_PHOTO} IMG_0001.jpg" "$CAPTION" "${L_PHOTO} IMG_0001.jpg"; do
    echo "scroll to: $candidate" >> "$FIT_LOG"
    scroll_to "$candidate"
    sleep 4
    if album_whole; then
      echo "whole: yes" >> "$FIT_LOG"
      return 0
    fi
    echo "whole: no" >> "$FIT_LOG"
  done
  return 0
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
scroll_to "$ALONE"
sleep 3
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
