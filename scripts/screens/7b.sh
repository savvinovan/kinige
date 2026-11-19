#!/usr/bin/env bash
# Screens of slice 7b (KINIG-136, the details of an attachment) in the real window on a macOS runner: scripts/screens/7b.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=7b. The data are synthetic (seed_demo with KINIGE_SEED_DETAILS=1: one small chat
# "Вложения" with hidden photos, a described photo, a renamed file card, an album with a hidden cell, a trashed hidden photo and three pending
# tiles) in a temporary directory; the real database is not touched. Input is the screens driver inside the app (KINIG-92, ADR-0016; DRIVER=1):
# it presses, right-clicks (a `contextmenu` event), types and reads by accessible name, so the dialog is filled in, search is typed and the
# menus are opened like a person would. Accessibility actions stay for what the web view cannot reach: resizing and closing the window.
# No mouse, no keyboard, no global shortcuts. What is in frame is checked from the pictures by the person who reads them, not only from the text.
#
# What it proves in the real window: a hidden photo that is never shown has no thumbnail file while its neighbours in the album have one
# (`thumbs.txt`: the cover asks for no picture), the cover is back after the chat is left, after the window went to the menu bar and after
# `kill -9`, the details are kept (`rows.txt`: the groups before and after an edit), the Markdown copy holds the photo under its new name
# (`mirror.txt`), search finds the new name and not the old one, and an agent reads the name, the description and the flag (`mcp.txt`).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7b}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror" "$OUT"
# The screens driver must be in this build, or the scenario has no hands.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }
KINIGE_SEED_DETAILS=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"

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
  L_CAPTION="Закат над заливом, июль"
  L_CARD="Договор аренды"
  L_PLAIN_TEXT="Обычное фото."
  L_ALBUM_TEXT="Прогулка вдоль реки."
  if [ "$UI" = "ru" ]; then
    L_CHAT="Вложения"; L_INBOX="Входящие"; L_TRASH="Корзина"; L_SEARCH="Поиск"; L_NONE="Ничего не найдено."; L_JUMP="К последним записям"; L_RESULTS="Результаты поиска"
    L_COVER="Скрытое фото, нажмите, чтобы показать"; L_DESC_HIDDEN="Описание скрыто"; L_HIDE="Снова скрыть фото"; L_VIEWER="Просмотр фото"; L_CLOSE="Закрыть"
    L_TILE_DETAILS="Свойства скрытого фото"; L_DIALOG="Свойства вложения"; L_CANCEL="Отмена"; L_SAVE="Сохранить"; L_SEND="Отправить"
    L_NAME="Название"; L_DESCRIPTION="Описание"; L_SPOILER="Скрыть как спойлер"; L_ENTRY_ITEM="Свойства вложения…"; L_HIDE_AS="Скрыть как спойлер"
  else
    L_CHAT="Вложения"; L_INBOX="Inbox"; L_TRASH="Trash"; L_SEARCH="Search"; L_NONE="Nothing found."; L_JUMP="Jump to the latest"; L_RESULTS="Search results"
    L_COVER="Hidden photo, press to show"; L_DESC_HIDDEN="Description hidden"; L_HIDE="Hide the photo again"; L_VIEWER="Photo viewer"; L_CLOSE="Close"
    L_TILE_DETAILS="Details of a hidden photo"; L_DIALOG="Attachment details"; L_CANCEL="Cancel"; L_SAVE="Save"; L_SEND="Send"
    L_NAME="Name"; L_DESCRIPTION="Description"; L_SPOILER="Hide as spoiler"; L_ENTRY_ITEM="Attachment details…"; L_HIDE_AS="Hide as spoiler"
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
  wait_name "${L_INBOX:-Входящие}" 60000 > /dev/null 2>&1 || true
  sleep 4
  detect_language
}

# press_nth <text> <n> [role]: the n-th (from 0) element whose name holds the text.
press_nth() {
  local query="$(driver_query "$1" "${3:-}"),\"nth\":$2" result
  if drive "$(drive_window)" "{\"kind\":\"press\",$query}"; then result=pressed; else result="$DRIVE_ERROR"; fi
  echo "press $1 #$2: $result"
  sleep "$PAUSE"
}
type_into() {
  if drive "$(drive_window)" "{\"kind\":\"type\",\"name\":\"$(json_escape "$1")\",\"text\":\"$(json_escape "$2")\"}"; then echo "typed into $1"; else echo "not typed into $1: $DRIVE_ERROR"; fi
  sleep "$PAUSE"
}
# How many elements have exactly this accessible name (the lines of the window text that equal it).
count_label() {
  driver_text "$(drive_window)" "$K/debug/count.txt" || return 1
  grep -cxF -- "$1" "$K/debug/count.txt" || true
}
thumb_ids() { ls "$K/data/thumbs" 2> /dev/null | sed -e 's/\..*$//' | sort; }

# One block of facts: every image, hidden or not, and whether it has a thumbnail file yet. `thumbs.txt` keeps a block per moment.
thumbs_block() {
  {
    echo "[$1]"
    echo "thumbnail files: $(thumb_ids | wc -l | tr -d ' ')"
    sqlw "select id, filename, spoiler from attachment where kind = 'image' order by filename" | while IFS='|' read -r id filename spoiler; do
      if thumb_ids | grep -q "^${id}$"; then has_thumb=yes; else has_thumb=no; fi
      echo "$filename  hidden=$spoiler  thumbnail=$has_thumb"
    done
  } >> "$OUT/thumbs.txt"
}
thumb_of() { grep -F "$1" "$OUT/thumbs.txt" | tail -1; }

# The groups of one attachment (never `core`: it holds the file key) and the columns the window reads.
rows_block() {
  {
    echo "[$1]"
    sqlw "select json_extract(groups_raw, '\$.name'), json_extract(groups_raw, '\$.description'), json_extract(groups_raw, '\$.privacy'), json_extract(hlc_json, '\$.name.h'), json_extract(hlc_json, '\$.description.h'), json_extract(hlc_json, '\$.privacy.h') from attachment where filename = 'IMG_0108.jpg'"
    echo "columns: $(sqlw "select display_name, description, spoiler from attachment where filename = 'IMG_0108.jpg'")"
  } >> "$OUT/rows.txt"
}

: > "$OUT/thumbs.txt"
: > "$OUT/rows.txt"

# ---- 1. The chat: covers, the caption line, the album, the file card ----
set_language ru
launch_window
wait_for_window
wait_name "Входящие" 60000 || fail "the window did not show the Inbox"
sleep 4
detect_language
press "${L_CHAT}"
wait_name "${L_PLAIN_TEXT}" 30000 || fail "the chat did not show its last entry"
rows_block "IMG_0108.jpg before any edit"

# 01: the lone cover, and below it the covered photo with a description, whose caption line says so (scrolled to from above, so it ends at the
# bottom of the frame with its whole entry above it).
drive_raw "$(drive_window)" "{\"kind\":\"scroll\",$(driver_query "${L_COVER}"),\"nth\":0}" > /dev/null
sleep 2
scroll_to "${L_DESC_HIDDEN}"
sleep 3
thumbs_block "the covers are in frame, the album has not been on screen"
shoot 01-covers-single
has "${L_DESC_HIDDEN}" || fail "the hidden photo with a description does not say so"
if has "${L_CAPTION}"; then fail "a hidden photo's description is on screen"; fi
# 01b: the album with one covered cell, then the facts: the plain cells have a thumbnail file now, the hidden one has none.
scroll_to "${L_ALBUM_TEXT}"
sleep 4
shoot 01b-album-covered-cell
thumbs_block "the album has been on screen"
for plain in IMG_0103.jpg IMG_0105.jpg IMG_0106.jpg; do
  thumb_of "$plain" | grep -q 'thumbnail=yes' || fail "$plain, a plain cell of the album, has no thumbnail file"
done
for hidden in IMG_0101.jpg IMG_0102.jpg IMG_0104.jpg IMG_0109.jpg; do
  thumb_of "$hidden" | grep -q 'hidden=1  thumbnail=no' || fail "$hidden, hidden and never shown, has a thumbnail file or is not hidden"
done
# 01c: the file card under its new name, with its description.
scroll_to "${L_CARD}"
sleep 3
shoot 01c-file-card
has "Подписанный экземпляр" || fail "the file card does not show its description"

# ---- 2. A click shows that photo and no other; its caption and Hide ----
scroll_to "${L_DESC_HIDDEN}"
press_nth "${L_COVER}" 1
sleep 2
scroll_to "${L_CAPTION}"
sleep 3
shoot 02-revealed-with-caption-and-hide
has "${L_HIDE}" || fail "a revealed photo has no Hide button"
has "${L_CAPTION}" || fail "the revealed photo's description is not shown"
scroll_to "${L_ALBUM_TEXT}"
press_nth "${L_COVER}" 1
sleep 3
shoot 02b-album-cell-revealed

# ---- 3. The viewer opens with the caption ----
scroll_to "${L_CAPTION}"
press "${L_CAPTION}"
wait_name "${L_VIEWER}" 10000 || fail "the viewer did not open"
sleep 2
shoot 03-viewer-with-caption
press "${L_CLOSE}"
sleep 2

# ---- 4. The right-click menu of a photo: Hide as spoiler covers it at once ----
scroll_to "кофейня.jpg"
show_menu "кофейня.jpg"
wait_name "${L_HIDE_AS}" 10000 "" MenuItem || fail "the photo's menu did not open"
shoot 04-photo-menu
press "${L_HIDE_AS}" MenuItem
sleep 3
scroll_to "${L_DESC_HIDDEN}"
shoot 04b-hidden-from-the-menu

# ---- 5. The tiles: a covered one, a shown one and a file ----
COVERS="$(count_label "${L_COVER}")"
[ -n "$COVERS" ] && [ "$COVERS" -ge 2 ] || fail "no covered tiles to press (covers: ${COVERS:-?})"
press_nth "${L_COVER}" $((COVERS - 1))
sleep 2
shoot 05-composer-tiles
has "${L_TILE_DETAILS}" || fail "no Details button on the covered tile"

# ---- 6. The dialog from a tile, then Cancel; Send: the entry is in frame, at the end of the feed ----
press_nth "${L_TILE_DETAILS}" 0
wait_name "${L_DIALOG}" 10000 || fail "the dialog did not open"
shoot 06-dialog-from-the-tile
has "${L_NAME}" && has "${L_DESCRIPTION}" || fail "the dialog's fields have no captions"
press "${L_CANCEL}"
wait_name "${L_DIALOG}" 10000 gone || fail "the dialog is still open after Cancel"
press "${L_SEND}"
sleep 4
if has "${L_JUMP}"; then fail "after Send the feed is not at its end (a \"${L_JUMP}\" button is there)"; fi
shoot 06b-sent-covered
has "заметки.pdf" || fail "the sent file card does not carry its new name"
[ "$(sqlw "select count(*) from attachment where spoiler = 1 and display_name = 'тайное.jpg'")" = "1" ] || fail "the sent photo lost its details"

# ---- 7. The entry's menu and the dialog: a name, a description, the box, Save; then the entry ----
scroll_to "${L_PLAIN_TEXT}"
show_menu "${L_PLAIN_TEXT}"
press "${L_ENTRY_ITEM}" MenuItem
wait_name "${L_DIALOG}" 10000 || fail "the dialog did not open from the entry's menu"
type_into "${L_NAME}" "ужин"
type_into "${L_DESCRIPTION}" "Ужин на веранде"
press "${L_SPOILER}" CheckBox
shoot 07-dialog-filled
press "${L_SAVE}"
wait_name "${L_DIALOG}" 10000 gone || fail "the dialog did not close on Save"
sleep 2
rows_block "IMG_0108.jpg after the dialog"
[ "$(sqlw "select display_name || '|' || description || '|' || spoiler from attachment where filename = 'IMG_0108.jpg'")" = "ужин.jpg|Ужин на веранде|1" ] || fail "the saved details are not what was typed"
scroll_to "${L_PLAIN_TEXT}"
sleep 3
shoot 07b-entry-after-save

# ---- 8. Leaving the chat covers the photos again ----
press "${L_INBOX}"
sleep 3
press "${L_CHAT}"
sleep 4
if has "${L_CAPTION}"; then fail "a photo shown before is still shown after the chat was left and opened again"; fi
scroll_to "${L_COVER}"
shoot 08-back-covered

# ---- 9. The window goes to the menu bar and comes back: a photo that was shown is covered ----
press_nth "${L_COVER}" 0
sleep 2
has "${L_HIDE}" || fail "no photo is shown before the window is closed"
PID_BEFORE="$(app_pid)"
limit 30 osascript -e "tell application \"System Events\" to tell (first process whose unix id is $PID_BEFORE) to click (first button of window 1 whose subrole is \"AXCloseButton\")" 2>&1 | head -2
sleep 3
launch_window
wait_for_window
sleep 4
[ "$(app_pid)" = "$PID_BEFORE" ] || fail "closing the window ended the process: the check below would prove nothing"
{
  echo "A photo was shown, the window was closed (to the menu bar) and shown again; the process is the same (pid $PID_BEFORE)."
  if has "${L_HIDE}"; then echo "a Hide button is still there: the photo is still shown"; else echo "no Hide button: the photo is covered again"; fi
} > "$OUT/close.txt"
cat "$OUT/close.txt"
if has "${L_HIDE}"; then fail "the photo is still shown after the window was closed and shown again"; fi
shoot 09-after-close-to-the-menu-bar

# ---- 10. Search: the new name is found, the old one is not ----
type_into "${L_SEARCH}" "закат"
wait_name "${L_RESULTS}" 10000 > /dev/null 2>&1 || true
sleep 4
if has "${L_NONE}"; then fail "search for the new name found nothing"; fi
shoot 10-search-new-name
type_into "${L_SEARCH}" "IMG_0102"
sleep 5
{
  echo "search for the old file name of the renamed photo:"
  if has "${L_NONE}"; then echo "nothing found (as it should be)"; else echo "something was found"; fi
} >> "$OUT/search.txt"
has "${L_NONE}" || fail "search for the old file name still finds the renamed photo"
shoot 10b-search-old-name
type_into "${L_SEARCH}" ""
sleep 3

# ---- 11. The Markdown copy and the rows: facts, never the file keys ----
{
  echo "# The Markdown copy of the chat: the day files' links to pictures and files, and the files of the day folders (names only)."
  find "$K/mirror" -type f -name '*.md' -exec grep -h '^!\[\|^\[' {} + 2> /dev/null | sort
  echo "# files in the day folders"
  find "$K/mirror" -mindepth 3 -type f ! -name '*.md' -exec basename {} \; 2> /dev/null | sort
} > "$OUT/mirror.txt"
grep -q 'закат.jpg' "$OUT/mirror.txt" || fail "the renamed hidden photo is not in the Markdown copy under its new name"
grep -q 'ужин.jpg' "$OUT/mirror.txt" || fail "the photo hidden from the dialog is not in the Markdown copy under its new name"

# ---- 12. An agent reads the chat through the bridge: name, description and flag ----
edit_settings 'd["agent_access"]["enabled"] = True; d["agent_access"]["mode"] = "read"; d["agent_access"]["chats"] = [{"title": "Вложения"}]'
limit 60 python3 "$REPO/scripts/mcp-read.py" "$BRIDGE" "$SOCKET" "$OUT/mcp.txt" || fail "the agent session"
# The tool's answer is JSON inside a JSON string, so its quotes are escaped in the protocol file.
grep -Eq 'spoiler[^a-z]+true' "$OUT/mcp.txt" || fail "the agent does not see a hidden photo's flag"
grep -q 'закат' "$OUT/mcp.txt" || fail "the agent does not see the new name"

# ---- 13. kill -9: every hidden photo is covered, the details are kept ----
ROWS_BEFORE="$(sqlw "select count(*), sum(spoiler), count(display_name), count(description) from attachment")"
restart_app
press "${L_CHAT}"
sleep 5
has "${L_COVER}" || fail "no cover after kill -9 and a restart"
if has "${L_HIDE}"; then fail "a photo is shown after a restart"; fi
[ "$(sqlw "select count(*), sum(spoiler), count(display_name), count(description) from attachment")" = "${ROWS_BEFORE}" ] || fail "the details changed across kill -9"
scroll_to "${L_COVER}"
sleep 3
shoot 13-after-kill-9

# ---- 14. The trash: a hidden photo is a cover there too ----
press "${L_TRASH}"
sleep 4
has "${L_COVER}" || fail "the hidden photo in the trash is not covered"
shoot 14-trash-covered

# ---- 15. English ----
press "${L_INBOX}"
sleep 2
set_language en
restart_app
press "${L_CHAT}"
sleep 5
has "${L_COVER}" || fail "no cover in the English window"
scroll_to "${L_COVER}"
sleep 3
shoot 15-english

# ---- 16. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
scroll_to "${L_COVER}"
sleep 3
shoot 16-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 17. A 640 px window ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
scroll_to "${L_COVER}"
sleep 3
shoot 17-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

finish
