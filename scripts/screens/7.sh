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
    L_INBOX="Входящие"; L_IDEAS="Идеи"; L_TRIP="Поездка"; L_SEND="Отправить"; L_TRASH="Корзина"
    L_PHOTO="Фото"; L_REMOVE="Убрать"; L_VIEWER="Просмотр фото"; L_NEXT="Следующее фото"; L_CLOSE="Закрыть"; L_MISSING="Файла нет"
  else
    L_INBOX="Inbox"; L_IDEAS="Идеи"; L_TRIP="Поездка"; L_SEND="Send"; L_TRASH="Trash"
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

# "x y width height" of the first element that matches: the boxes of the photos and the card, before and after the thumbnails load.
frames() {
  for item in "${L_PHOTO} IMG_0001.jpg" "${L_PHOTO} IMG_0005.jpg" "меню.pdf" "${L_MISSING}"; do
    echo "$item: $(ax frame "$(app_pid)" "$item" 2>&1)"
  done
}

IDEAS="$(chat_id 'Идеи')"
TRIP="$(chat_id 'Поездка')"

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
shoot 01-feed-photos-and-files
ls "$K/data/thumbs" > "$K/debug/thumbs-first.txt" 2>&1
[ -s "$K/debug/thumbs-first.txt" ] || fail "no thumbnail was built for the feed"

# ---- 2. The feed does not jump: the boxes before and after the thumbnails are built again ----
# The window is relaunched with the thumbnail cache emptied: the boxes are measured right after the window appears and again later.
kill -9 "$(app_pid)"
sleep 2
rm -rf "$K/data/thumbs"
launch_window
wait_for_window
{
  echo "# Boxes (x y width height) of the first matches in the feed, the thumbnail cache emptied before the launch."
  echo "# 'early' is the first moment the window was readable, 'late' is after the thumbnails were built."
  echo "[early]"
  frames
  sleep 8
  echo "[late]"
  frames
  echo "thumbnails built: $(ls "$K/data/thumbs" 2>/dev/null | wc -l | tr -d ' ')"
} > "$OUT/no-jump.txt"
cat "$OUT/no-jump.txt"
# Only the sizes are compared (the last two numbers of a line): a position also depends on the scroll the window was at when it was read.
sizes() { grep -E '^[^#\[].*: [-0-9]+ [-0-9]+ [0-9]+ [0-9]+$' | sed -E 's/ [-0-9]+ [-0-9]+ ([0-9]+) ([0-9]+)$/ \1 \2/'; }
early="$(sed -n '/^\[early\]/,/^\[late\]/p' "$OUT/no-jump.txt" | sizes)"
late="$(sed -n '/^\[late\]/,/^thumbnails built/p' "$OUT/no-jump.txt" | sizes)"
[ -n "$early" ] && [ "$early" = "$late" ] || fail "the feed boxes changed size when the thumbnails were built (or were not found)"

# ---- 3. Pending tiles of the chat "Идеи": they come from the draft; kill -9 keeps them ----
press "${L_IDEAS}"
sleep 3
has "${L_REMOVE} IMG_0009.jpg" || fail "no tile for IMG_0009.jpg above the field"
has "${L_REMOVE} заметки.pdf" || fail "no tile for заметки.pdf above the field"
shoot 02-pending-tiles
restart_app
detect_language
press "${L_IDEAS}"
sleep 3
has "${L_REMOVE} IMG_0009.jpg" || fail "the tile did not survive kill -9"
[ "$(sqlw "select count(*) from draft_attachment")" = "2" ] || fail "draft_attachment does not hold two rows after kill -9"
shoot_light() { SHOOT_DUMP=0 shoot "$@"; }
shoot_light 03-pending-tiles-after-kill9

# ---- 4. Remove one tile, then send the other one with no text ----
press "${L_REMOVE} заметки.pdf"
sleep 2
[ "$(sqlw "select count(*) from draft_attachment")" = "1" ] || fail "the removed tile is still in draft_attachment"
has "${L_REMOVE} заметки.pdf" && fail "the removed tile is still on screen"
shoot 04-one-tile-removed
BEFORE="$(sqlw "select count(*) from message where chat_id = '$IDEAS' and deleted_at is null")"
press "${L_SEND}"
sleep 4
AFTER="$(sqlw "select count(*) from message where chat_id = '$IDEAS' and deleted_at is null")"
[ "${AFTER}" -eq $((BEFORE + 1)) ] || fail "the entry with a photo and no text was not sent (${BEFORE} -> ${AFTER})"
[ "$(sqlw "select count(*) from draft_attachment")" = "0" ] || fail "the draft still has tiles after sending"
has "${L_PHOTO} IMG_0009.jpg" || fail "the sent photo is not in the feed"
shoot 05-sent-without-text

# ---- 5. Lightbox over the album ----
press "${L_INBOX}"
sleep 3
press "${L_PHOTO} IMG_0002.jpg"
sleep 3
has "${L_VIEWER}" || fail "the photo viewer did not open"
has "2 / 4" || fail "the viewer does not say 2 / 4"
shoot 06-lightbox
press "${L_NEXT}"
sleep 3
has "3 / 4" || fail "the next photo did not show 3 / 4"
shoot_light 07-lightbox-next
press "${L_CLOSE}"
sleep 2
has "${L_VIEWER}" && fail "the viewer is still open after Close"

# ---- 6. Trash: the photo goes there with the entry ----
press "${L_TRASH}"
sleep 3
has "${L_PHOTO} IMG_0008.jpg" || fail "the trashed entry shows no photo"
shoot 08-trash-with-photo

# ---- 7. The Telegram chat imported twice: the "not imported" line is gone, the photos and the file are attached ----
press "${L_TRIP}"
sleep 4
[ "$(sqlw "select count(*) from attachment a join message m on m.id = a.message_id where m.chat_id = '$TRIP'")" = "4" ] || fail "the Telegram chat does not hold four attachments"
[ "$(sqlw "select count(*) from message where chat_id = '$TRIP' and body_md like '%не импортировано%'")" = "0" ] || fail "a placeholder line is still there after the second import"
has "Договор.pdf" || fail "the imported PDF is not in the feed"
shoot 09-telegram-photos

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
if sqlw "select blob_key from attachment union select remote_blob_id from attachment" | while read -r secret; do grep -rl "$secret" "$K/mirror" 2> /dev/null; done | grep -q .; then
  fail "a blob key or remote blob id reached the Markdown copy"
fi

# ---- 9. Files on disk against rows: no orphans, nothing half-copied ----
{
  echo "rows: $(sqlw "select count(*) from attachment")"
  echo "rows marked missing: $(sqlw "select count(*) from attachment_local where state = 'missing'")"
  echo "files in attachments/: $(find "$K/data/attachments" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  echo "leftover .part files: $(find "$K/data/attachments/.tmp" -name '*.part' 2> /dev/null | wc -l | tr -d ' ')"
  echo "orphans moved aside: $(find "$K/data/attachments/.orphans" -type f 2> /dev/null | wc -l | tr -d ' ')"
  echo "thumbnails: $(find "$K/data/thumbs" -type f 2> /dev/null | wc -l | tr -d ' ')"
  echo "rows without a blob key or remote blob id: $(sqlw "select count(*) from attachment where blob_key = '' or remote_blob_id = ''")"
} > "$OUT/files.txt"
cat "$OUT/files.txt"
ROWS="$(sqlw "select count(*) from attachment")"
MISSING="$(sqlw "select count(*) from attachment_local where state = 'missing'")"
FILES="$(find "$K/data/attachments" -maxdepth 1 -type f | wc -l | tr -d ' ')"
[ "$FILES" -eq $((ROWS - MISSING)) ] || fail "files on disk (${FILES}) and rows (${ROWS}, missing ${MISSING}) do not match"

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

# ---- 11. English, large text, 640 px ----
set_language en
restart_app
detect_language
press "${L_INBOX}"
sleep 3
shoot 10-en
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot_light 11-large-text
edit_settings 'd["text_size"] = "normal"'
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot_light 12-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- Privacy of the log: no file names, chat names or paths ----
if sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "IMG_00|меню|билеты|маршрут|заметки|Договор|photo_[0-9]|Поездка|seed-files|tg-media|attachments/|/open/" > "$K/debug/log-private.txt" 2>&1; then
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
cat "$OUT/bench.txt" "$OUT/cold-start.txt" "$OUT/import-media.txt"
RSS="$(awk '/maximum resident/ {print $1}' "$K/debug/media-time.txt")"
[ "${RSS:-0}" -gt 0 ] && [ "${RSS:-0}" -lt 300000000 ] || fail "peak memory of the 2 GB import is ${RSS:-unknown} bytes (limit 300 MB)"

finish
