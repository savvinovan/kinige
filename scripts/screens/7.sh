#!/usr/bin/env bash
# Screens of slice 7 (attachments: photos and files) in the real window on a macOS runner: scripts/screens/7.sh <built Kinige.app>.
# The data are synthetic (seed_demo with KINIGE_SEED_ATTACH=1: pictures drawn by the seed, a PDF stub, an album of four, an entry
# with no text, a mixed entry, a file that is gone from the disk, a trashed photo, two pending tiles in the chat "Идеи", a
# Telegram chat "Поездка" imported twice: first without files, then with them) in a temporary directory; the real database is
# not touched. Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. The native file dialog,
# a paste from the clipboard and a drop from Finder cannot be driven that way: they are covered by the IPC tests
# (src-tauri/tests/ipc_slice7.rs), the store tests and vitest; here the pending tiles come from the draft the seed wrote, which is
# also what a restart or kill -9 brings back.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/7}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The window tree is read one property at a time, so the forest strip (371 cells) made every search for an element in the feed
# run into the time limit on the first attempt. The scenario hides the forest first (the saved view state of slice 6): this slice
# is about the feed, and the tree is then small. The shots are also copied into the debug folder when the scenario fails, so that
# a failed run can still be looked at (the debug artifact is never committed).
export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"
INBOX="00000000-0000-4000-8000-000000000001"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_ATTACH=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
plutil -insert LSEnvironment.KINIGE_LOG -string debug "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
rm -f "$LOG_FILE"

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
  if [ "$UI" = "ru" ]; then
    L_INBOX="Входящие"; L_IDEAS="Идеи"; L_TRIP="Поездка"; L_SEND="Отправить"; L_TRASH="Корзина"; L_BOOKS="Книги"; L_SNAPS="Снимки"
    L_SEARCH="Поиск"; L_SEARCH_RESULTS="Результаты поиска"; L_RESTORE_SNAPS="Восстановить запись из чата «Снимки»"; L_PURGE_ALBUM="Удалить навсегда запись из чата «Альбом»"
    L_CONFIRM_TITLE="Удалить навсегда?"; L_CONFIRM="Удалить навсегда | Удалить навсегда"
    L_PHOTO="Фото"; L_REMOVE="Убрать"; L_VIEWER="Просмотр фото"; L_NEXT="Следующее фото"; L_CLOSE="Закрыть"; L_MISSING="Файла нет"
  else
    L_INBOX="Inbox"; L_IDEAS="Идеи"; L_TRIP="Поездка"; L_SEND="Send"; L_TRASH="Trash"; L_BOOKS="Книги"; L_SNAPS="Снимки"
    L_SEARCH="Search"; L_SEARCH_RESULTS="Search results"; L_RESTORE_SNAPS="Restore the entry from “Снимки”"; L_PURGE_ALBUM="Delete forever the entry from “Альбом”"
    L_CONFIRM_TITLE="Delete forever?"; L_CONFIRM="Delete forever | Delete forever"
    L_PHOTO="Photo"; L_REMOVE="Remove"; L_VIEWER="Photo viewer"; L_NEXT="Next photo"; L_CLOSE="Close"; L_MISSING="File is missing"
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
}

# "x y width height" of the first element that matches: the four cells of the album, the caption under it and the bubble after it.
# Elements outside the visible part of the feed still have frames (a negative y is above it).
geometry() {
  for item in "${L_PHOTO} IMG_0001.jpg" "${L_PHOTO} IMG_0002.jpg" "${L_PHOTO} IMG_0003.jpg" "${L_PHOTO} IMG_0004.jpg" "Прогулка вдоль реки" "Меню на субботу"; do
    echo "$item: $(ax frame "$(app_pid)" "$item" 2>&1)"
  done
}

thumb_count() { find "$K/data/thumbs" -type f 2> /dev/null | wc -l | tr -d ' '; }

# One line of facts about the files: rows, files, thumbnails, tombstones, draft tiles. `files.txt` keeps a block of these per moment.
snap() {
  {
    echo "[$1]"
    echo "rows: $(sqlw "select count(*) from attachment")  draft tiles: $(sqlw "select count(*) from draft_attachment")  marked missing: $(sqlw "select count(*) from attachment_local where state = 'missing'")"
    echo "files in attachments/: $(find "$K/data/attachments" -maxdepth 1 -type f | wc -l | tr -d ' ')  thumbnails: $(thumb_count)  tombstones of attachments: $(sqlw "select count(*) from tombstone where entity = 'attachment'")"
  } >> "$OUT/files.txt"
}

# A press that returns whether it worked, for a step that has a fallback (no failure is counted here).
try_press() { [ "$(ax press "$(app_pid)" "$@" 2>&1 | cut -c1-7)" = "pressed" ]; }

IDEAS="$(chat_id 'Идеи')"
BOOKS="$(chat_id 'Книги')"
TRIP="$(chat_id 'Поездка')"
: > "$OUT/files.txt"

# ---- 1. Inbox: an album of four, a portrait photo with no text, a card with a caption, a mixed entry, a missing file ----
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
if ! has "${L_PHOTO} IMG_0002.jpg"; then
  # Nothing after this can work: stop here with the window tree and the shot instead of failing every step at a minute apiece.
  shoot_light() { SHOOT_DUMP=0 shoot "$@"; }
  fail "no photo of the album in the feed"
  shoot_light 00-failed-first-look
  finish
  exit 1
fi
has "меню.pdf" || fail "no file card in the feed"
has "${L_MISSING}" || fail "the missing file is not marked"
[ "$(sqlw "select count(*) from attachment_local where state = 'missing'")" = "1" ] || fail "the sweep did not mark exactly one file as missing"
# The window opens at the bottom of the feed; the album is above it and its pictures load lazily. Bring the album into view first,
# so that its thumbnails are requested and the shot shows the grid.
scroll_to "${L_PHOTO} IMG_0002.jpg"
sleep 4
shoot 01-feed-album-and-files
[ "$(thumb_count)" -ge 4 ] || fail "fewer than four thumbnails were built with the album in view ($(thumb_count))"

# ---- 2. The feed does not jump when the thumbnails load ----
# Relaunch with the thumbnail cache emptied. The album is above the visible part, so none of its pictures is requested: the geometry
# of its cells, the caption and the next bubble is read from the tree as it is (laid out from the width and height in the row). Then
# the album is scrolled into view, the thumbnails are built, and the geometry is read again. Positions are compared relative to the
# first cell, so the scroll itself does not count; sizes are compared as they are.
kill -9 "$(app_pid)"
sleep 2
rm -rf "$K/data/thumbs"
launch_window
wait_for_window
sleep 6
BEFORE_THUMBS="$(thumb_count)"
{
  echo "# Boxes (x y width height) from the window tree: the four cells of the album, its caption, the bubble after it."
  echo "# [before]: the album is above the visible part, its pictures are not loaded. [after]: it is scrolled into view and loaded."
  echo "[before]"
  geometry
  echo "thumbnails built: ${BEFORE_THUMBS}"
} > "$OUT/no-jump.txt"
scroll_to "${L_PHOTO} IMG_0002.jpg"
for _ in $(seq 1 30); do
  [ "$(thumb_count)" -ge $((BEFORE_THUMBS + 4)) ] && break
  sleep 1
done
sleep 2
{
  echo "[after]"
  geometry
  echo "thumbnails built: $(thumb_count)"
} >> "$OUT/no-jump.txt"
cat "$OUT/no-jump.txt"
python3 - "$OUT/no-jump.txt" >> "$OUT/no-jump.txt" <<'PY' || fail "the album or the text around it changed its layout when the thumbnails loaded"
import re, sys

sections, current = {}, None
for line in open(sys.argv[1], encoding="utf-8"):
    line = line.rstrip("\n")
    if line in ("[before]", "[after]"):
        current = line.strip("[]")
        sections[current] = {"boxes": {}, "thumbs": 0}
    elif current and (m := re.match(r"(.+): (-?\d+) (-?\d+) (\d+) (\d+)$", line)):
        sections[current]["boxes"][m.group(1)] = tuple(int(g) for g in m.groups()[1:])
    elif current and line.startswith("thumbnails built:"):
        sections[current]["thumbs"] = int(line.split(":")[1])

def relative(boxes):
    base = next((b for name, b in boxes.items() if name.endswith("IMG_0001.jpg")), None)
    if base is None:
        return None
    return {name: (x - base[0], y - base[1], w, h) for name, (x, y, w, h) in boxes.items()}

before, after = relative(sections["before"]["boxes"]), relative(sections["after"]["boxes"])
grew = sections["after"]["thumbs"] - sections["before"]["thumbs"]
print()
if before is None or after is None or len(before) < 6 or before.keys() != after.keys():
    print("RESULT: the boxes were not all found: before", before, "after", after)
    sys.exit(1)
if grew < 4:
    print(f"RESULT: only {grew} thumbnails were built after the scroll: the pictures did not load")
    sys.exit(1)
if before != after:
    print("RESULT: the layout changed:", {n: (before[n], after[n]) for n in before if before[n] != after[n]})
    sys.exit(1)
print(f"RESULT: no jump: {len(before)} boxes keep their sizes and their offsets from the first cell while {grew} thumbnails load")
PY
tail -2 "$OUT/no-jump.txt"

# ---- 3. Pending tiles of the chat "Идеи": they come from the draft; kill -9 keeps them ----
press "${L_IDEAS}"
sleep 3
has "${L_REMOVE} IMG_0009.jpg" || fail "no tile for IMG_0009.jpg above the field"
has "${L_REMOVE} заметки.pdf" || fail "no tile for заметки.pdf above the field"
shoot 02-pending-tiles
shoot_light() { SHOOT_DUMP=0 shoot "$@"; }

# ---- 3. Paste an image from the clipboard through the menu bar (Edit > Paste: an AX press, not a key) ----
# Whether WKWebView hands an image on the pasteboard to `clipboardData.files` is a finding, not an assumption: the result goes to
# `paste.txt` either way, and the scenario goes on.
press "${L_BOOKS}"
sleep 3
sips -s format png "$K/data/seed-files/IMG_0001.jpg" --out "$K/clip.png" > /dev/null 2>&1
osascript -e "set the clipboard to (read (POSIX file \"$K/clip.png\") as «class PNGf»)" > "$K/debug/clipboard.txt" 2>&1
ax front "$(app_pid)" > /dev/null 2>&1
sleep 1
PASTE_MENU="$(ax menu "$(app_pid)" "Edit|Правка" "Paste|Вставить" 2>&1)"
sleep 5
PASTED="$(sqlw "select filename from draft_attachment where draft_key = 'chat:$BOOKS' limit 1")"
{
  echo "menu: ${PASTE_MENU}"
  echo "clipboard: $(cat "$K/debug/clipboard.txt" | head -c 200)"
  if [ -n "${PASTED}" ]; then
    echo "RESULT: the pasted image became a tile (${PASTED}); WKWebView handed it to clipboardData.files"
  else
    echo "RESULT: no tile after Edit > Paste with a PNG on the pasteboard; WKWebView did not hand the image to the page (a finding)"
  fi
} > "$OUT/paste.txt"
cat "$OUT/paste.txt"
shoot_light 03-paste-from-clipboard
if [ -n "${PASTED}" ]; then
  try_press "${L_REMOVE} ${PASTED}" || fail "the pasted tile could not be removed"
  sleep 2
  [ "$(sqlw "select count(*) from draft_attachment where draft_key = 'chat:$BOOKS'")" = "0" ] || fail "the pasted tile is still in draft_attachment"
fi
press "${L_IDEAS}"
sleep 2
restart_app
detect_language
press "${L_IDEAS}"
sleep 3
has "${L_REMOVE} IMG_0009.jpg" || fail "the tile did not survive kill -9"
[ "$(sqlw "select count(*) from draft_attachment")" = "2" ] || fail "draft_attachment does not hold two rows after kill -9"
shoot_light 04-pending-tiles-after-kill9

# ---- 4. Remove one tile, then send the other one with no text ----
press "${L_REMOVE} заметки.pdf"
sleep 2
[ "$(sqlw "select count(*) from draft_attachment")" = "1" ] || fail "the removed tile is still in draft_attachment"
has "${L_REMOVE} заметки.pdf" && fail "the removed tile is still on screen"
shoot 05-one-tile-removed
BEFORE="$(sqlw "select count(*) from message where chat_id = '$IDEAS' and deleted_at is null")"
press "${L_SEND}"
sleep 4
AFTER="$(sqlw "select count(*) from message where chat_id = '$IDEAS' and deleted_at is null")"
[ "${AFTER}" -eq $((BEFORE + 1)) ] || fail "the entry with a photo and no text was not sent (${BEFORE} -> ${AFTER})"
[ "$(sqlw "select count(*) from draft_attachment")" = "0" ] || fail "the draft still has tiles after sending"
has "${L_PHOTO} IMG_0009.jpg" || fail "the sent photo is not in the feed"
shoot 06-sent-without-text

# ---- 5. Lightbox over the album ----
press "${L_INBOX}"
sleep 3
press "${L_PHOTO} IMG_0002.jpg"
sleep 3
has "${L_VIEWER}" || fail "the photo viewer did not open"
has "2 / 4" || fail "the viewer does not say 2 / 4"
shoot 07-lightbox
press "${L_NEXT}"
sleep 3
has "3 / 4" || fail "the next photo did not show 3 / 4"
shoot_light 08-lightbox-next
press "${L_CLOSE}"
sleep 2
has "${L_VIEWER}" && fail "the viewer is still open after Close"

# ---- 6. Trash: the photos go there with the entry; restore brings one back, "Delete forever" removes the files of another ----
snap "before the trash steps"
ALBUM_IDS="$(sqlw "select a.id from attachment a join message m on m.id = a.message_id join chat c on c.id = m.chat_id where c.title = 'Альбом'" | tr '\n' ' ')"
[ "$(echo "${ALBUM_IDS}" | wc -w | tr -d ' ')" = "2" ] || fail "the entry that will be deleted for good does not have two attachments (${ALBUM_IDS})"
press "${L_TRASH}"
sleep 3
has "${L_PHOTO} IMG_0008.jpg" || fail "the trashed entry shows no photo"
shoot 09-trash-with-photos
# Restore: the entry and its photo come back, the file never moved.
press "${L_RESTORE_SNAPS}"
sleep 3
[ "$(sqlw "select count(*) from message m join chat c on c.id = m.chat_id where c.title = 'Снимки' and m.deleted_at is null")" = "1" ] || fail "the entry was not restored"
[ "$(sqlw "select count(*) from attachment a join message m on m.id = a.message_id join chat c on c.id = m.chat_id where c.title = 'Снимки' and a.deleted_at is null")" = "1" ] || fail "the restored entry lost its photo"
snap "after restore"
press "${L_SNAPS}"
sleep 3
has "${L_PHOTO} IMG_0010.jpg" || fail "the restored photo is not in its chat"
shoot 10-restored-in-chat
# Delete forever: confirmation first (the default focus is on Cancel), then the rows, the files and the thumbnails go.
press "${L_TRASH}"
sleep 3
press "${L_PURGE_ALBUM}"
sleep 2
has "${L_CONFIRM_TITLE}" || fail "the confirmation did not open"
shoot_light 11-delete-forever-confirmation
try_press "${L_CONFIRM}" || fail "the confirmation button could not be pressed"
sleep 4
[ "$(sqlw "select count(*) from attachment a join message m on m.id = a.message_id join chat c on c.id = m.chat_id where c.title = 'Альбом'")" = "0" ] || fail "the purged entry still has attachment rows"
for id in ${ALBUM_IDS}; do
  [ -z "$(find "$K/data/attachments" "$K/data/thumbs" -maxdepth 1 -name "${id}.*" 2> /dev/null)" ] || fail "a file of the purged entry is still on disk (${id})"
  [ "$(sqlw "select count(*) from tombstone where entity = 'attachment' and id = '${id}'")" = "1" ] || fail "no tombstone for the purged attachment ${id}"
done
snap "after delete forever"

# ---- 7. The Telegram chat imported twice: the "not imported" line is gone, the photos and the file are attached ----
press "${L_TRIP}"
sleep 4
[ "$(sqlw "select count(*) from attachment a join message m on m.id = a.message_id where m.chat_id = '$TRIP'")" = "4" ] || fail "the Telegram chat does not hold four attachments"
[ "$(sqlw "select count(*) from message where chat_id = '$TRIP' and body_md like '%не импортировано%'")" = "0" ] || fail "a placeholder line is still there after the second import"
has "Договор.pdf" || fail "the imported PDF is not in the feed"
shoot 12-telegram-photos

# ---- 8. The Markdown copy: files next to the day file, links above the text, no keys anywhere ----
{
  echo "# Files of the Markdown copy (path relative to the copy folder, size in bytes)"
  (cd "$K/mirror" && find . -type f -not -name '.*' | sort | while IFS= read -r f; do printf '%s %s\n' "$f" "$(stat -f %z "$f")"; done)
  echo "# Link lines of the copy"
  grep -rhE '^!?\[[^]]+\]\(<' "$K/mirror" --include='*.md' | sort | uniq | head -30
} > "$OUT/mirror.txt"
cat "$OUT/mirror.txt"
grep -q '^!\[IMG_0001.jpg\](<' "$OUT/mirror.txt" || fail "no image link for IMG_0001.jpg in the Markdown copy"
grep -q '^\[меню.pdf\](<' "$OUT/mirror.txt" || fail "no file link for меню.pdf in the Markdown copy"
grep -qE '/[0-9]{4}-[0-9]{2}-[0-9]{2}/IMG_0001.jpg [0-9]+$' "$OUT/mirror.txt" || fail "the picture file itself is not next to the day file"
grep -q '^!\[IMG_0010.jpg\](<' "$OUT/mirror.txt" || fail "the restored entry is not in the Markdown copy"
grep -qE 'IMG_0011|смета.pdf' "$OUT/mirror.txt" && fail "a file of the entry deleted for good is still in the Markdown copy"
if sqlw "select blob_key from attachment union select remote_blob_id from attachment" | while read -r secret; do grep -rl "$secret" "$K/mirror" 2> /dev/null; done | grep -q .; then
  fail "a blob key or remote blob id reached the Markdown copy"
fi

# ---- 9. Files on disk against rows: no orphans, nothing half-copied ----
snap "end of the scenario"
{
  echo "[leftovers]"
  echo "leftover .part files: $(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null | wc -l | tr -d ' ')"
  echo "orphans moved aside: $(find "$K/data/attachments/.orphans" -type f 2> /dev/null | wc -l | tr -d ' ')"
  echo "rows without a blob key or remote blob id: $(sqlw "select count(*) from attachment where blob_key = '' or remote_blob_id = ''")"
} >> "$OUT/files.txt"
cat "$OUT/files.txt"
ROWS="$(sqlw "select count(*) from attachment")"
TILES="$(sqlw "select count(*) from draft_attachment")"
MISSING="$(sqlw "select count(*) from attachment_local where state = 'missing'")"
FILES="$(find "$K/data/attachments" -maxdepth 1 -type f | wc -l | tr -d ' ')"
[ "$FILES" -eq $((ROWS + TILES - MISSING)) ] || fail "files on disk (${FILES}) and rows (${ROWS}, tiles ${TILES}, missing ${MISSING}) do not match"

# ---- 10. Open in the system app: a clone of the file, never the stored file itself ----
press "${L_INBOX}"
sleep 3
press "меню.pdf"
sleep 6
OPENED="$(find "$K/data/open" -name 'меню.pdf' 2> /dev/null | head -1)"
[ -n "$OPENED" ] || fail "no clone of меню.pdf under open/ after opening the card"
{
  echo "clone exists: yes"
  echo "clone mode: $(stat -f %Lp "$OPENED")"
  echo "stored file untouched: $(cmp -s "$OPENED" "$K/data/attachments/$(sqlw "select id from attachment where filename = 'меню.pdf'").pdf" && echo same || echo different)"
} > "$OUT/open.txt"
cat "$OUT/open.txt"
ax front "$(app_pid)" > /dev/null 2>&1

# ---- 11. Search by file name: «маршрут» is in no text, only in the name маршрут.pdf of an attachment ----
# The query goes in the way a person's paste does: the word on the pasteboard, the search field focused with an AX action, Edit > Paste
# from the menu bar (setting the field's AXValue directly did not reach the page as typing: the first run of this step proved it).
# The results section ("Search results") exists only while a search is open, so seeing it, and the entry in it, is the evidence.
# Whether the page took the paste is a finding: the result goes to `search.txt` either way.
press "${L_INBOX}"
sleep 2
osascript -e 'set the clipboard to "маршрут"' > "$K/debug/clipboard-search.txt" 2>&1
SEARCH_FOCUS="$(ax focus "$(app_pid)" "${L_SEARCH}" 2>&1)"
sleep 1
SEARCH_MENU="$(ax menu "$(app_pid)" "Edit|Правка" "Paste|Вставить" 2>&1)"
sleep 5
if has "${L_SEARCH_RESULTS}" && has "Фото и два документа к поездке"; then
  SEARCH_RESULT="RESULT: the search results are open and the entry is in them: found by the file name маршрут.pdf (its text does not contain the word)"
else
  SEARCH_RESULT="RESULT: the search results did not open or did not hold the entry after the paste (a finding)"
fi
{
  echo "focus: ${SEARCH_FOCUS}"
  echo "menu: ${SEARCH_MENU}"
  echo "text of the entry: $(sqlw "select body_md from message where body_md like 'Фото и два документа%'")"
  echo "entries whose text has the word: $(sqlw "select count(*) from message where body_md like '%маршрут%'")"
  echo "${SEARCH_RESULT}"
} > "$OUT/search.txt"
cat "$OUT/search.txt"
shoot_light 13-search-by-file-name

# ---- 12. The forest card with photos (the method of slice 6: the expanded board and the pinned caption of the selected day) ----
kill -9 "$(app_pid)"
sleep 2
sqlw "insert or replace into app_setting(key, value) values ('forest_view', 'expanded')"
launch_window
wait_for_window
sleep 8
# The tree of the expanded forest is large: a full dump with a long limit, and the caption is read from that dump.
DUMP_TIMEOUT=420 shoot 14-forest-card-with-photos
{
  echo "# Lines of the window tree of the expanded forest that hold the day card summary"
  grep -E '[0-9]+ (записей|записи|запись|notes?) · [0-9]+ (фото|photos?)' "$K/debug/shot-14-forest-card-with-photos.txt" | sed -E 's/^ +//' | head -3
} > "$OUT/forest-card.txt"
cat "$OUT/forest-card.txt"
[ "$(grep -c '·' "$OUT/forest-card.txt")" -ge 1 ] || fail "the forest card does not say how many photos the day has"
kill -9 "$(app_pid)"
sleep 2
sqlw "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"

# ---- 13. English, large text, 640 px ----
set_language en
launch_window
wait_for_window
sleep 6
detect_language
press "${L_INBOX}"
sleep 3
shoot 15-en
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot_light 16-large-text
edit_settings 'd["text_size"] = "normal"'
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot_light 17-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- Privacy of the log: no file names, chat names or paths ----
if sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "IMG_00|меню|билеты|маршрут|заметки|Договор|photo_[0-9]|Поездка|Снимки|Альбом|смета|seed-files|tg-media|attachments/|/open/" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds file names, chat names or paths: $(head -c 300 "$K/debug/log-private.txt")"
fi
if sqlw "select blob_key from attachment union select remote_blob_id from attachment" | while read -r secret; do grep -l "$secret" "$LOG_FILE" 2> /dev/null; done | grep -q .; then
  fail "a blob key or remote blob id reached the log"
fi

# ---- Measurements: 100 000 entries with 10 000 photos, a 2 GB import with peak memory, the cold start with all of it ----
kill -9 "$(app_pid)"
sleep 2
rm -rf "$K/data/open"
cargo build --release -q -p kinige-store --example import_bench || fail "build of import_bench"
BENCH="$REPO/target/release/examples/import_bench"
[ -x "$BENCH" ] || BENCH="${CARGO_TARGET_DIR:-$REPO/target}/release/examples/import_bench"
/usr/bin/time -l "$BENCH" "$K/bench" synthetic 100000 --images 10000 > "$OUT/bench.txt" 2> "$K/debug/bench-time.txt" || fail "import_bench 100000 with 10000 photos"
grep -E "maximum resident|peak memory|real" "$K/debug/bench-time.txt" >> "$OUT/bench.txt"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
rm -rf "$K/bench"
/usr/bin/time -l "$BENCH" "$K/bench-media" synthetic 200 --files 40 --file-mb 50 > "$OUT/import-media.txt" 2> "$K/debug/media-time.txt" || fail "import_bench 2 GB"
grep -E "maximum resident|peak memory|real" "$K/debug/media-time.txt" >> "$OUT/import-media.txt"
rm -rf "$K/bench-media"
# The control for the forest year table: the same code, the same VM, the same 100 000 entries, no photos. The difference between the
# two lines is what the attachments cost; the VM itself drifts between runs, so a number from another run is not a comparison.
"$BENCH" "$K/bench-plain" synthetic 100000 > "$OUT/bench-no-images.txt" 2>&1 || fail "import_bench 100000 without photos"
rm -rf "$K/bench-plain"
{
  echo "# get_forest_year on the same VM and the same code in this run: with 10 000 photos (bench.txt) and without photos (bench-no-images.txt)"
  echo "with photos:    $(grep 'forest year table' "$OUT/bench.txt" | sed -E 's/.*calls over [0-9.]+: //; s/;.*//')"
  echo "without photos: $(grep 'forest year table' "$OUT/bench-no-images.txt" | sed -E 's/.*calls over [0-9.]+: //; s/;.*//')"
} > "$OUT/forest-compare.txt"
cat "$OUT/forest-compare.txt"
cat "$OUT/bench.txt" "$OUT/cold-start.txt" "$OUT/import-media.txt"
RSS="$(awk '/maximum resident/ {print $1}' "$K/debug/media-time.txt")"
[ "${RSS:-0}" -gt 0 ] && [ "${RSS:-0}" -lt 300000000 ] || fail "peak memory of the 2 GB import is ${RSS:-unknown} bytes (limit 300 MB)"

finish
