#!/usr/bin/env bash
# Shots of slice 5 ("On this day" and the "N years later" reply) in the real window on the macOS runner: scripts/screens/5.sh <built Kinige.app>.
# The data is synthetic (seed_demo) in a temporary directory; the real database is not touched. The result is $OUT (docs/screens/5).
# Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. There is no typing from the keyboard, so
# the scenario itself puts an unfinished reply into the draft through sql (as another process would do before the app restarts),
# and "the entry went to the trash" is done by editing the database from outside, after which the feed is opened again.
# The UI language is Russian (`AppleLanguages` in the app domain), English at the end; button labels are chosen by the language
# that the window shows (`UI`), so the scenario does not break if the system failed to set the language.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# Traversing the window tree with the feed, the card and the sidebar is slow, so presses and searches wait up to 150 s. The dump for the personal
# path check (`shoot`) in `lib.sh` has its own limit (`DUMP_TIMEOUT`, 120 s): a dump cut off by time is empty, and the check would check
# nothing (on an empty dump `shoot` fails).
export AX_TIMEOUT=150

INBOX="00000000-0000-4000-8000-000000000001"
SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt "$OUT"/*.md
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }

# Editing settings.json "from outside" (as in 4b.sh): a temporary file and a replace. The code changes the dictionary d.
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

# The date "31 октября 2023" / "October 31, 2023" from YYYY-MM-DD in the language of the window.
long_date() {
  python3 - "$1" "$UI" <<'PY'
import sys
iso, lang = sys.argv[1], sys.argv[2]
y, m, d = (int(p) for p in iso.split("-"))
ru = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"]
en = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]
print(f"{d} {ru[m - 1]} {y}" if lang == "ru" else f"{en[m - 1]} {d}, {y}")
PY
}

# Window labels in two languages.
labels() {
  if [ "$UI" = "ru" ]; then
    L_HIDE="Скрыть до завтра"; L_SEND="Отправить"; L_INBOX="Входящие"; L_BOOKS="Книги"; L_ANNA="Анна"
    L_REPLY="Ответить на запись"; L_QUOTE="ответ на запись"; L_PLAIN_QUOTE="Ответ на запись"; L_STRIP="Ответ на запись"
    L_TRASH="Запись в корзине"
  else
    L_HIDE="Hide until tomorrow"; L_SEND="Send"; L_INBOX="Inbox"; L_BOOKS="Книги"; L_ANNA="Анна"
    L_REPLY="Reply to the entry of"; L_QUOTE="reply to the entry of"; L_PLAIN_QUOTE="Reply to the entry of"; L_STRIP="Replying to the entry of"
    L_TRASH="Entry in the trash"
  fi
}

detect_language() {
  # By the title of the Inbox chat in the sidebar: it is at the start of the tree, so the search is fast (the card may be absent, and then the search goes through the whole tree).
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

restart_app() {
  kill -9 "$(app_pid)"
  sleep 2
  launch_window
  wait_for_window
  sleep 3
}

TODAY="$(python3 -c 'import datetime as d; print((d.datetime.now() - d.timedelta(hours=4)).strftime("%Y-%m-%d"))')"
OLD_ID="$(sqlw "select id from message where body_md like 'Так будет лучше%' and deleted_at is null")"
OLD_DATE="$(sqlw "select journal_date from message where id = '${OLD_ID}'")"
[ -n "${OLD_ID}" ] || fail "the database has no three-year-old entry"
echo "today by the journal: ${TODAY}; the old entry: ${OLD_DATE}"

# ---- 1. Inbox with the card ----
set_language ru
launch_window
wait_for_window
sleep 3
detect_language
OLD_LABEL="$(long_date "${OLD_DATE}")"
has "${L_HIDE}" || fail "the window has no \"On this day\" card (the \"${L_HIDE}\" button)"
# The card is tall: the feed is open at the bottom edge, the card title with the first year is scrolled into view.
scroll_to "${L_HIDE}"
shoot 01-inbox-card

# ---- 2. "Reply" on the three-year-old entry: the line above the field ----
press "${L_REPLY} ${OLD_LABEL}"
sleep 2
[ "$(sqlw "select reply_to_id from draft where key = 'chat:$INBOX'")" = "${OLD_ID}" ] || fail "the draft with the reply is not saved in the database"
shoot 02-reply-line

# ---- 3. The scenario puts the reply text into the draft; kill -9 and launch: the text and the line are in place ----
sqlw "update draft set body = 'До сих пор так думаю' where key = 'chat:$INBOX'"
restart_app
detect_language
shoot 03-draft-after-restart

# ---- 4. "Send": the reply in the feed with a quote ----
press "${L_SEND}"
sleep 2
[ "$(sqlw "select reply_to_id from message where body_md = 'До сих пор так думаю'")" = "${OLD_ID}" ] || fail "the reply has no reply_to_id in the database"
[ -z "$(sqlw "select 1 from draft where key = 'chat:$INBOX'")" ] || fail "the draft was not cleared by sending"
shoot 04-reply-sent
# The Markdown copy: under the reply's time heading, a line with a link to the day file of the old entry.
sleep 3
MIRROR_FILE="$K/mirror/Inbox/${TODAY%%-*}/${TODAY}.md"
if [ -f "$MIRROR_FILE" ]; then
  cp "$MIRROR_FILE" "$OUT/mirror-reply.md"
  grep -q '↩ \[' "$OUT/mirror-reply.md" || fail "the day file has no ↩ line"
else
  fail "no day file $MIRROR_FILE"
fi

# ---- 5. A click on the quote: the old entry in context ----
press "${L_QUOTE} ${OLD_LABEL}"
sleep 2
shoot 05-old-entry-from-quote

# ---- 6. The old entry went to the trash (the database edited from outside): the quote "Запись в корзине" ----
sqlw "update message set deleted_at = $(python3 -c 'import time; print(int(time.time()*1000))') where id = '${OLD_ID}'"
press "${L_BOOKS}"
press "${L_INBOX}"
sleep 2
shoot 06-quote-in-trash
sqlw "update message set deleted_at = NULL where id = '${OLD_ID}'"

# ---- 7. The close cross of the card ----
press "${L_HIDE}"
sleep 2
[ "$(sqlw "select value from app_setting where key = 'on_this_day_dismissed'")" = "${TODAY}" ] || fail "the dismissal is not recorded for today's journal day"
shoot 07-card-dismissed

# ---- 8. kill -9: no card, the reply with the quote is in place ----
restart_app
detect_language
[ "$(sqlw "select reply_to_id from message where body_md = 'До сих пор так думаю'")" = "${OLD_ID}" ] || fail "after kill -9 the reply has no reply_to_id"
shoot 08-after-kill9

# ---- 9. The chat "Книги": no card ----
press "${L_BOOKS}"
shoot 09-books-chat

# ---- 11. Large text (the dismissal is cleared, returning to Inbox reads the card again) ----
sqlw "delete from app_setting where key = 'on_this_day_dismissed'"
edit_settings 'd["text_size"] = "large"'
press "${L_INBOX}"
sleep 2
has "${L_HIDE}" || fail "no card after the dismissal was cleared"
# The card is tall: the feed is open at the bottom edge, the card title with the first year is scrolled into view.
scroll_to "${L_HIDE}"
shoot 11-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 12-13. An imported personal Telegram chat: quotes without the "N years later" caption ----
press "${L_ANNA}"
sleep 2
shoot 12-telegram-chat-quotes
press "${L_PLAIN_QUOTE} $(long_date 2023-03-14)"
sleep 2
shoot 13-telegram-quote-opened

# ---- 10. English ----
press "${L_INBOX}"
sleep 1
sqlw "delete from app_setting where key = 'on_this_day_dismissed'"
set_language en
restart_app
detect_language
has "${L_HIDE}" || fail "no card in the English window"
# The card is tall: the feed is open at the bottom edge, the card title with the first year is scrolled into view.
scroll_to "${L_HIDE}"
shoot 10-en

# ---- Measurements on 100,000 synthetic entries: the card in import_bench, the cold start of the app ----
kill -9 "$(app_pid)"
sleep 2
cargo run --release -q -p kinige-store --example import_bench -- "$K/bench" synthetic 100000 > "$OUT/bench.txt" 2>&1 || fail "import_bench measurement"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
cat "$OUT/bench.txt" "$OUT/cold-start.txt"

finish
