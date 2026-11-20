#!/usr/bin/env bash
# Screens of slice 5c (KINIG-151, stickers and GIFs) in the real window on a macOS runner: scripts/screens/5c.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=5c. The data are synthetic (seed_demo with KINIGE_SEED_STICKERS=1: a pack of drawn stickers, stickers and
# GIFs in Inbox, a synthetic Telegram export imported twice, and, with KINIGE_SEED_GIF_MEMORY=1, seven heavy GIFs in a chat of their own) in a temporary
# directory. The Bot API is a fake server on the runner's localhost (`fake-bot.py`; the build has the `test-hooks` feature, `KINIGE_TEST_BOT_API` points at it
# and `KINIGE_TEST_TOKEN_STORE=memory` keeps the token in memory, so nothing touches a keychain or Telegram). Input is the screens driver (DRIVER=1).
#
# What it proves in the real window: the panel and its tabs, no token and then a token saved in Settings, a pack added by its link with progress and a
# result that counts what it skipped (`requests.txt`: what the fake server saw, with the token as <token>), a sticker sent at once, the re-imported entries
# (an unedited one is a sticker, the edited one keeps its text and shows a picture), the GIFs tab and the GIFs in the feed, kill -9 (the library and the sent
# stickers stay), search and the trash, English, large text and 640 px; `mirror.txt` and `mcp.txt` are what the copy and an agent see; `gif-memory.txt` is the
# peak of the web content process with the heavy GIFs; `cold-start.txt` is the start with this data. The import preview of the synthetic export (the line
# "Stickers: N sent...") needs the system file dialog, which no script drives: its numbers are a store test (`stickers_import.rs`) and a story.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5c}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The cat face and the heart as bytes: the file holds no invisible character, and bash 3.2 of the runner has no \u escape.
CAT=$'\xf0\x9f\x98\xba'
TOKEN="42:fake-token-for-screens"
LINK="https://t.me/addstickers/ScreensPack"
SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bot"
mkdir -p "$K/data" "$K/mirror" "$K/bot" "$OUT"
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect bot-hooks || { echo "ERROR: the Bot API test hooks are not in this build"; FAILS=$((FAILS + 1)); }
KINIGE_SEED_STICKERS=1 KINIGE_SEED_GIF_MEMORY=1 KINIGE_SEED_FAKE_BOT_DIR="$K/bot" cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
# An agent may read Inbox (the settings are rows of app_setting; the app reads them at launch).
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('mcp_enabled', '1'), ('mcp_mode', 'read'), ('mcp_chats', '[\"00000000-0000-4000-8000-000000000001\"]')"

# The fake server of the Bot API: one pack, one second per download so that the progress can be shot.
python3 "$REPO/scripts/screens/fake-bot.py" "$K/bot" "$K/bot.port" "$OUT/requests.txt" 1 &
BOT_PID=$!
trap 'kill "$BOT_PID" 2> /dev/null; cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
for _ in $(seq 1 50); do [ -s "$K/bot.port" ] && break; sleep 0.2; done
[ -s "$K/bot.port" ] || fail "the fake Bot API server did not start"
LS_ENV="KINIGE_TEST_BOT_API=http://127.0.0.1:$(cat "$K/bot.port") KINIGE_TEST_TOKEN_STORE=memory"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }

labels() {
  if [ "$UI" = "ru" ]; then
    L_INBOX="Входящие"; L_TRASH="Корзина"; L_SEARCH="Поиск"; L_NONE="Ничего не найдено."; L_SETTINGS="Настройки"
    L_EMOJI="Эмодзи"; L_TAB_STICKERS="Стикеры"; L_TAB_GIFS="GIF"; L_STICKER="Стикер $CAT"; L_ADD_PACK="Добавить набор стикеров"
    L_LINK="Ссылка на набор"; L_ADD="Добавить"; L_NO_TOKEN="Добавьте токен бота в Настройках"; L_PROGRESS="Загружено"; L_ADDED="Добавлен набор"
    L_OPEN_SETTINGS="Открыть Настройки"; L_TABS="Эмодзи, стикеры и GIF"
    L_TOKEN_FIELD="Токен бота Telegram"; L_SAVE="Сохранить"; L_SAVED="Токен сохранён"; L_DONE="Готово"; L_CAT_ENTRY="Этот кот мой любимый"
  else
    L_INBOX="Inbox"; L_TRASH="Trash"; L_SEARCH="Search"; L_NONE="Nothing found."; L_SETTINGS="Settings"
    L_EMOJI="Emoji"; L_TAB_STICKERS="Stickers"; L_TAB_GIFS="GIFs"; L_STICKER="Sticker $CAT"; L_ADD_PACK="Add a sticker pack"
    L_LINK="Pack link"; L_ADD="Add"; L_NO_TOKEN="Add your bot token in Settings"; L_PROGRESS="Downloading"; L_ADDED="Added"
    L_OPEN_SETTINGS="Open Settings"; L_TABS="Emoji, stickers and GIFs"
    L_TOKEN_FIELD="Telegram bot token"; L_SAVE="Save"; L_SAVED="Token saved"; L_DONE="Done"; L_CAT_ENTRY="Этот кот мой любимый"
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
type_into() {
  if drive "$(drive_window)" "{\"kind\":\"type\",\"name\":\"$(json_escape "$1")\",\"text\":\"$(json_escape "$2")\"}"; then echo "typed into $1"; else echo "not typed into $1: $DRIVE_ERROR"; fi
  sleep "$PAUSE"
}
escape_key() { drive "$(drive_window)" '{"kind":"key","key":"Escape","code":"Escape"}' > /dev/null 2>&1 || true; sleep "$PAUSE"; }
# The panel is a toggle: the state is read from the window first, so a panel that stayed open is not closed by the press that should open it.
open_panel() { has "$L_TABS" || press "$L_EMOJI" button; sleep 2; }
close_panel() { if has "$L_TABS"; then press "$L_EMOJI" button; fi; sleep 2; }
# A scroll that may find nothing (the feed draws the entries near the window only): no scenario error either way.
scroll_quiet() { drive_raw "$(drive_window)" "{\"kind\":\"scroll\",$(driver_query "$1")}" > /dev/null 2>&1 || true; sleep "$PAUSE"; }
count_where() { sqlw "select count(*) from $1"; }

# ---- 1. The Stickers tab: the strip of packs and the grid under the headings ----
set_language ru
launch_window
wait_for_window
wait_name "Входящие" 60000 || fail "the window did not show the Inbox"
sleep 4
detect_language
[ "$(count_where "sticker_pack")" = "2" ] || fail "the seed has not two packs (Cats and From Telegram)"
open_panel
press "$L_TAB_STICKERS" tab
wait_name "$L_STICKER" 10000 || fail "the Stickers tab shows no sticker"
sleep 2
shoot 01-stickers-tab

# ---- 2. "+" without a token: the dialog says where to add it ----
press "$L_ADD_PACK" button
wait_name "$L_NO_TOKEN" 10000 || fail "the add-pack dialog without a token does not say where to add it"
sleep 2
shoot 02-add-without-token
[ -s "$OUT/requests.txt" ] && fail "a request reached the fake server before a token was saved"

# ---- 3. Settings, from the dialog's "Open Settings": the token, saved, with "Remove" ----
press "$L_OPEN_SETTINGS" button
wait_for_settings
in_settings
# The Stickers page of Settings (the route is named like the tab).
has "$L_TOKEN_FIELD" || press "$L_TAB_STICKERS"
wait_name "$L_TOKEN_FIELD" 10000 || fail "Settings has no token field"
type_into "$L_TOKEN_FIELD" "$TOKEN"
press "$L_SAVE" button
wait_name "$L_SAVED" 10000 || fail "the token was not saved"
sleep 2
shoot_settings 03-settings-token
[ "$(sqlw "select value from app_setting where key = 'telegram_token_saved'")" = "1" ] || fail "the saved flag is not set"
if sqlite3 "$K/data/kinige.db" .dump | grep -q "fake-token-for-screens"; then fail "the token is in the database"; fi
in_main
# The dialog of step 2 may still be open behind the Settings window: it is closed by its Cancel (or has closed itself), and the panel is closed too.
has "$L_NO_TOKEN" && press "Отмена" button
has "$L_NO_TOKEN" && press "Cancel" button
close_panel

# ---- 4-5. Adding the pack: progress, then the result with the skipped one ----
open_panel
press "$L_TAB_STICKERS" tab
press "$L_ADD_PACK" button
wait_name "$L_LINK" 10000 || fail "the add-pack dialog did not open"
type_into "$L_LINK" "$LINK"
press "$L_ADD" button
wait_name "$L_PROGRESS" 20000 || fail "no progress line"
sleep 2
shoot 04-progress
wait_name "$L_ADDED" 60000 || fail "the pack was not added"
sleep 2
shoot 05-result-with-skips
[ "$(count_where "sticker_pack")" = "3" ] || fail "the pack is not in the library"
[ "$(sqlw "select count(*) from sticker where pack_id = (select id from sticker_pack where link_name = 'ScreensPack')")" = "6" ] || fail "the pack does not hold its six stickers"
grep -q '/bot<token>/getStickerSet?name=ScreensPack' "$OUT/requests.txt" || fail "the fake server did not see getStickerSet"
if grep -q 'fake-token-for-screens' "$OUT/requests.txt"; then fail "the token is in requests.txt"; fi
press "$L_DONE" button
sleep 2

# ---- 6. A sticker sent at once: no bubble, the draft stays ----
STICKERS_BEFORE="$(sqlw "select count(*) from attachment where sticker = 1")"
open_panel
press "$L_TAB_STICKERS" tab
press "$L_STICKER" button
sleep 3
[ "$(sqlw "select count(*) from attachment where sticker = 1")" = "$((STICKERS_BEFORE + 1))" ] || fail "the click did not send a sticker"
close_panel
scroll_to "Как тебе новые наклейки"
sleep 3
shoot 06-feed-stickers

# ---- 7. The re-imported entries: a sticker where there was a placeholder, the edited one keeps its text ----
scroll_to "$L_CAT_ENTRY"
sleep 3
shoot 07-reimported
[ "$(sqlw "select count(*) from message m where m.body_md = '' and exists (select 1 from attachment a where a.message_id = m.id and a.sticker = 1) and m.origin = 'import_telegram'")" -ge 3 ] || fail "the unedited imported stickers are not stickers"
[ "$(sqlw "select count(*) from message m where m.body_md like '%$L_CAT_ENTRY%' and exists (select 1 from attachment a where a.message_id = m.id and a.sticker = 1)")" = "1" ] || fail "the edited entry has no sticker attachment"

# ---- 9. The GIFs tab: the journal's own GIFs ----
open_panel
press "$L_TAB_GIFS" tab
sleep 3
shoot 09-gifs-tab
close_panel

# ---- 10. The GIFs in the feed: the small one plays, the large one shows its first frame with a badge ----
scroll_to "cat-large"
sleep 3
shoot 10-gifs-feed
has "GIF" || fail "no GIF badge in the feed"

# ---- gif-memory.txt: the web content process while the seven heavy GIFs scroll through ----
web_rss() { ps -axo rss=,comm= | awk '/WebContent/ { if ($1 > m) m = $1 } END { print m + 0 }'; }
{
  echo "# Peak resident size (KB) of the web content process, sampled while the chat 'GIF' (six GIFs near 7.5 MiB and one of 1.96 megapixels, all under the autoplay limits) scrolls past"
  echo "before opening the chat: $(web_rss)"
} > "$OUT/gif-memory.txt"
press "GIF" button
sleep 4
PEAK=0
for n in 1 2 3 4 5 6 7; do
  scroll_quiet "heavy-$((7 - n)).gif"
  sleep 3
  RSS="$(web_rss)"
  echo "after scrolling to GIF $n: $RSS" >> "$OUT/gif-memory.txt"
  [ "$RSS" -gt "$PEAK" ] && PEAK="$RSS"
done
echo "peak: $PEAK KB" >> "$OUT/gif-memory.txt"
press "$L_INBOX" button
sleep 3

# ---- Markdown copy and an agent: what each sees of a sticker ----
sleep 5
{
  echo "# The sticker lines of the Markdown copy (an image by its file name; no emoji)"
  find "$K/mirror" -type f -name '*.md' -exec grep -h 'sticker' {} + 2> /dev/null | sort | uniq -c
} > "$OUT/mirror.txt"
grep -q '!\[sticker.webp\](<' "$OUT/mirror.txt" || fail "the Markdown copy has no sticker line"
cat "$OUT/mirror.txt"
limit 60 python3 "$REPO/scripts/mcp-read.py" "$BRIDGE" "$SOCKET" "$OUT/mcp.txt" || fail "the agent session"
grep -Eq 'sticker[^a-z]+true' "$OUT/mcp.txt" || fail "the agent does not see a sticker's flag"

# ---- 11-12. kill -9: the library and the sent stickers stay; search by the stem; the trash ----
sqlw "update message set deleted_at = $(date +%s)000 where id = (select message_id from attachment where sticker = 1 order by rowid limit 1)"
ROWS_BEFORE="$(sqlw "select count(*), sum(sticker) from attachment; select count(*) from sticker; select count(*) from sticker_pack")"
restart_app
[ "$(sqlw "select count(*), sum(sticker) from attachment; select count(*) from sticker; select count(*) from sticker_pack")" = "$ROWS_BEFORE" ] || fail "the library or the stickers changed across kill -9"
ls "$K/data/stickers" | grep -v '^\.' | wc -l | tr -d ' ' > "$K/debug/library-files.txt"
[ "$(cat "$K/debug/library-files.txt")" = "$(sqlw "select count(*) from sticker")" ] || fail "the library files and rows differ after kill -9"
scroll_to "Как тебе новые наклейки"
sleep 3
shoot 11-after-kill-9
type_into "$L_SEARCH" "sticker"
sleep 5
{
  echo "search for the stem of the file name (sticker):"
  if has "$L_NONE"; then echo "nothing found"; else echo "found"; fi
} > "$OUT/search.txt"
has "$L_NONE" && fail "search by the stem sticker found nothing"
shoot 12-search
type_into "$L_SEARCH" ""
sleep 3
press "$L_TRASH"
sleep 4
shoot 12b-trash
press "$L_INBOX"
sleep 3

# ---- 13. English ----
set_language en
restart_app
[ "$UI" = "en" ] || fail "the window is not in English after the restart"
open_panel
press "$L_TAB_STICKERS" tab
wait_name "$L_STICKER" 10000 || fail "the English Stickers tab shows no sticker"
sleep 2
shoot 13-english

# ---- 14. Large text ----
sqlw "insert or replace into app_setting(key, value) values ('text_size', 'large')"
restart_app
open_panel
press "$L_TAB_STICKERS" tab
sleep 3
shoot 14-large-text
sqlw "insert or replace into app_setting(key, value) values ('text_size', 'normal')"

# ---- 15. A 640 px window ----
restart_app
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
open_panel
press "$L_TAB_STICKERS" tab
sleep 3
shoot 15-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- The cold start with this data, and the start of an empty one for comparison ----
kill -9 "$(app_pid)"
sleep 2
{
  KINIGE_DATA_DIR="$K/data" KINIGE_MIRROR_DIR="$K/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
{
  KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start-control.txt" 2>&1 || fail "cold start control"
cat "$OUT/cold-start.txt" "$OUT/gif-memory.txt"

finish
