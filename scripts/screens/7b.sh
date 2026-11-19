#!/usr/bin/env bash
# Screens of slice 7b (KINIG-136, the details of an attachment) in the real window on a macOS runner: scripts/screens/7b.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=7b. The data are synthetic (seed_demo with KINIGE_SEED_DETAILS=1: one small chat
# "Вложения" with hidden photos, a described photo, a renamed file card, an album with a hidden cell, a trashed hidden photo and two pending tiles)
# in a temporary directory; the real database is not touched. The chat is small on purpose: ax.js visits an element at a time, and a long feed
# makes an element late in the tree "absent" by timeout. Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts,
# so nothing is typed here (the dialog's fields are covered by vitest and the store tests); what is present or absent is read from the window dump
# of a shot, which is complete or the shot fails. The language is Russian, then English; large text; a 640 px window.
#
# What it proves in the real window: a hidden photo that is never shown has no thumbnail file (`thumbs.txt`: the cover asks for no picture, its
# neighbour does), the cover is back after the chat is left and after `kill -9`, the details are kept, and the Markdown copy holds the photo under
# its new name (`mirror.txt`, `rows.txt`). The entry's menu, the right-click menu and search need a pointer or typing, which this runner does not
# have: they are covered by vitest; the MCP answer by the store tests.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7b}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

export AX_TIMEOUT=90
export DUMP_TIMEOUT=150
SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror" "$OUT"
KINIGE_SEED_DETAILS=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }

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
  L_CHAT="Вложения"
  if [ "$UI" = "ru" ]; then
    L_INBOX="Входящие"; L_COVER="Скрытое фото, нажмите, чтобы показать"; L_HIDE="Снова скрыть фото"; L_VIEWER="Просмотр фото"; L_CLOSE="Закрыть"
    L_TILE_DETAILS="Свойства скрытого фото"; L_DIALOG="Свойства вложения"; L_CANCEL="Отмена"; L_SEND="Отправить"; L_TRASH="Корзина"
    L_DESC_HIDDEN="Описание скрыто"; L_CAPTION="Закат над заливом, июль"; L_PLAIN="Фото IMG_0108.jpg"
  else
    L_INBOX="Inbox"; L_COVER="Hidden photo, press to show"; L_HIDE="Hide the photo again"; L_VIEWER="Photo viewer"; L_CLOSE="Close"
    L_TILE_DETAILS="Details of a hidden photo"; L_DIALOG="Attachment details"; L_CANCEL="Cancel"; L_SEND="Send"; L_TRASH="Trash"
    L_DESC_HIDDEN="Description hidden"; L_CAPTION="Закат над заливом, июль"; L_PLAIN="Photo IMG_0108.jpg"
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
  sleep 5
  detect_language
}

thumb_ids() { ls "$K/data/thumbs" 2> /dev/null | sed -e 's/\..*$//' | sort; }

# One block of facts: which attachments are hidden, which have a thumbnail file. `thumbs.txt` keeps a block per moment.
thumbs_block() {
  {
    echo "[$1]"
    echo "thumbnail files: $(thumb_ids | wc -l | tr -d ' ')"
    sqlw "select a.id, a.kind, a.spoiler from attachment a join message m on m.id = a.message_id where a.kind = 'image' order by a.filename" | while IFS='|' read -r id kind spoiler; do
      if thumb_ids | grep -q "^${id}$"; then has_thumb=yes; else has_thumb=no; fi
      echo "image $(printf '%.8s' "$id")  hidden=$spoiler  thumbnail=$has_thumb"
    done
  } >> "$OUT/thumbs.txt"
}

: > "$OUT/thumbs.txt"

# ---- 1. The chat: hidden photos are covers, their neighbours are pictures ----
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
has "${L_INBOX}" || fail "the window did not show the Inbox"
press "${L_CHAT}"
sleep 5
has "${L_PLAIN}" || fail "no plain photo in the chat"
thumbs_block "the chat just opened, nothing hidden was touched"
scroll_to "${L_COVER}"
sleep 4
shoot 01-covers-in-the-feed
has "${L_COVER}" || fail "no cover in the feed"
has "${L_DESC_HIDDEN}" || fail "the hidden photo with a description does not say so"
has "${L_CAPTION}" && fail "a hidden photo's description is on screen"
thumbs_block "the feed shows the covers"
# The proof of the cover: no hidden, never-shown photo has a thumbnail file; a plain photo does.
HIDDEN_WITH_THUMB="$(grep -c 'hidden=1  thumbnail=yes' "$OUT/thumbs.txt" || true)"
PLAIN_WITH_THUMB="$(grep -c 'hidden=0  thumbnail=yes' "$OUT/thumbs.txt" || true)"
[ "${HIDDEN_WITH_THUMB}" = "0" ] || fail "a hidden photo that was never shown has a thumbnail file"
[ "${PLAIN_WITH_THUMB}" -ge 1 ] || fail "no plain photo has a thumbnail file, so the check proves nothing"

# ---- 2. A click shows that photo and no other ----
press "${L_COVER}"
sleep 3
press "${L_COVER}"
sleep 3
shoot 02-two-revealed-with-caption-and-hide
has "${L_HIDE}" || fail "a revealed photo has no Hide button"
has "${L_CAPTION}" || fail "the revealed photo's description is not shown"

# ---- 3. The viewer opens with the caption ----
press "${L_CAPTION}"
sleep 3
has "${L_VIEWER}" || fail "the viewer did not open"
shoot 03-viewer-with-caption
press "${L_CLOSE}"
sleep 2

# ---- 4. The tiles above the field: a covered photo with Details and the x ----
scroll_to "${L_PLAIN}"
sleep 3
shoot 04-composer-tiles
has "${L_TILE_DETAILS}" || fail "no Details button on the covered tile"

# ---- 5. The dialog from a tile's Details ----
press "${L_TILE_DETAILS}"
sleep 3
has "${L_DIALOG}" || fail "the dialog did not open"
shoot 05-dialog-from-the-tile
press "${L_CANCEL}"
sleep 2
has "${L_DIALOG}" && fail "the dialog is still open after Cancel"

# ---- 6. Sending: the entry arrives with its photo covered and its file card under the new name ----
press "${L_SEND}"
sleep 4
shoot 06-sent-covered
has "заметки.pdf" || fail "the sent file card does not carry its new name"
[ "$(sqlw "select count(*) from attachment where spoiler = 1 and display_name = 'тайное.jpg'")" = "1" ] || fail "the sent photo lost its details"

# ---- 7. Leaving the chat covers the photos again ----
press "${L_INBOX}"
sleep 3
press "${L_CHAT}"
sleep 4
has "${L_CAPTION}" && fail "a photo shown before is still shown after the chat was left and opened again"
scroll_to "${L_COVER}"
sleep 3
shoot 07-back-covered

# ---- 9. kill -9: every hidden photo is covered, the details are kept ----
ROWS_BEFORE="$(sqlw "select count(*), sum(spoiler), count(display_name), count(description) from attachment")"
restart_app
press "${L_CHAT}"
sleep 5
has "${L_COVER}" || fail "no cover after kill -9 and a restart"
has "${L_CAPTION}" && fail "a photo is shown after a restart"
[ "$(sqlw "select count(*), sum(spoiler), count(display_name), count(description) from attachment")" = "${ROWS_BEFORE}" ] || fail "the details changed across kill -9"
scroll_to "${L_COVER}"
sleep 3
shoot 09-after-kill-9

# ---- 14. The trash: a hidden photo is a cover there too ----
press "${L_TRASH}"
sleep 4
has "${L_COVER}" || fail "the hidden photo in the trash is not covered"
shoot 14-trash-covered

# ---- The Markdown copy and the rows: facts, never the file keys (`core` is not printed) ----
{
  echo "# The Markdown copy of the chat: the day files' links to pictures and the day folders (names only)."
  find "$K/mirror" -type f -name '*.md' -exec grep -h '^!\[\|^\[' {} + 2> /dev/null | sort
  echo "# files in the day folders"
  find "$K/mirror" -mindepth 3 -type f ! -name '*.md' -exec basename {} \; 2> /dev/null | sort
} > "$OUT/mirror.txt"
cat "$OUT/mirror.txt"
grep -q 'закат.jpg' "$OUT/mirror.txt" || fail "the renamed hidden photo is not in the Markdown copy under its new name"
{
  echo "# The three details groups and the HLC record of one attachment (core is never printed: it holds the file key)."
  sqlw "select json_extract(groups_raw, '\$.name'), json_extract(groups_raw, '\$.description'), json_extract(groups_raw, '\$.privacy'), hlc_json from attachment where display_name = 'закат.jpg'"
  echo "# the same columns the window reads"
  sqlw "select display_name, description, spoiler from attachment where display_name = 'закат.jpg'"
} > "$OUT/rows.txt"

# ---- 11. English ----
press "${L_INBOX}"
sleep 2
set_language en
restart_app
press "${L_CHAT}"
sleep 5
has "${L_COVER}" || fail "no cover in the English window"
scroll_to "${L_COVER}"
sleep 3
shoot 11-english

# ---- 12. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
scroll_to "${L_COVER}"
sleep 3
shoot 12-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 13. A 640 px window ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
scroll_to "${L_COVER}"
sleep 3
shoot 13-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

finish
