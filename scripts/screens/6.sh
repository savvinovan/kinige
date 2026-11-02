#!/usr/bin/env bash
# Screens of slice 6 (the year's forest) in the real window on a macOS runner: scripts/screens/6.sh <built Kinige.app>.
# The data are synthetic (seed_demo: a full current year and the year before it, every growth stage, a week without entries,
# days with entries in several chats, one entry in the trash) in a temporary directory; the real database is not touched.
# Input is accessibility actions only (ax.js): no mouse, no keyboard, no global shortcuts. There is no typing, so the note that
# makes today's tree grow is put into the Inbox draft by the scenario through sql (as another process would before a restart)
# and sent with the "Send" button. The interface language is Russian, then English.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/6}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The tree of this window is huge (a feed of 200 entries, 371 cells of the strip, 365 buttons of the calendar) and System Events
# reads it one property at a time: a full dump of it took more than 120 s on the runner and the run timed out (first attempt).
# So no dumps here: the data are synthetic and live in a temporary directory (as in the Windows scenario), and the actions are
# arranged so that a search for an element never has to walk through the feed: the pane comes before the feed in the tree, the
# entry to scroll to is one of the oldest loaded ones (the tree runs from the old to the new), and the note is sent in the chat
# "Идеи", which has one entry, not in the Inbox.
export AX_TIMEOUT=150
dump() { :; }
dump_complete() { return 0; }

INBOX="00000000-0000-4000-8000-000000000001"
SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
# The app logs at debug level, so that the log holds the line with the first weekday and its source (week-info.txt).
plutil -insert LSEnvironment.KINIGE_LOG -string debug "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"
LOG_FILE="$HOME/Library/Logs/$BUNDLE_ID/kinige.log"
rm -f "$LOG_FILE"

sqlw() { sqlite3 -cmd '.timeout 5000' "$K/data/kinige.db" "$@"; }
set_language() { defaults write "$BUNDLE_ID" AppleLanguages -array "$1"; }

# Edit settings.json from outside (as 5.sh does): a temporary file and a replace. The code changes the dictionary d.
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

# "12 сентября" / "September 12" from YYYY-MM-DD in the language of the window.
short_date() {
  python3 - "$1" "$UI" <<'PY'
import sys
iso, lang = sys.argv[1], sys.argv[2]
y, m, d = (int(p) for p in iso.split("-"))
ru = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"]
en = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]
print(f"{d} {ru[m - 1]}" if lang == "ru" else f"{en[m - 1]} {d}")
PY
}

labels() {
  if [ "$UI" = "ru" ]; then
    L_EXPAND="Развернуть лес"; L_COLLAPSE="Свернуть лес"; L_HIDE="Скрыть лес"; L_SHOW="Показать лес"; L_WHOLE="Весь год"
    L_AUTUMN="Осень"; L_CALENDAR="Календарь"; L_FOREST="Лес"; L_PREV="Предыдущий год"; L_NEXT="Следующий год"
    L_BOARD="Лес ${YEAR} года"; L_STRIP="Дни ${YEAR} года"; L_PROFILE="Профиль"; L_SEND="Отправить"
    L_INBOX="Входящие"; L_IDEAS="Идеи"
  else
    L_EXPAND="Expand forest"; L_COLLAPSE="Collapse forest"; L_HIDE="Hide forest"; L_SHOW="Show forest"; L_WHOLE="Whole year"
    L_AUTUMN="Autumn"; L_CALENDAR="Calendar"; L_FOREST="Forest"; L_PREV="Previous year"; L_NEXT="Next year"
    L_BOARD="Forest of ${YEAR}"; L_STRIP="Days of ${YEAR}"; L_PROFILE="Profile"; L_SEND="Send"
    L_INBOX="Inbox"; L_IDEAS="Идеи"
  fi
}

detect_language() {
  # By the name of the Inbox in the sidebar: it is the first in the tree.
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

restart_app() {
  kill -9 "$(app_pid)"
  sleep 2
  launch_window
  wait_for_window
  sleep 4
}

# The journal day of the runner (the day changes at 04:00) and the dates the scenario aims at.
TODAY="$(python3 -c 'import datetime as d; print((d.datetime.now() - d.timedelta(hours=4)).strftime("%Y-%m-%d"))')"
YEAR="${TODAY%%-*}"
# A day 20..35 days back whose first entry is in the Inbox (its text ends with the date): inside the first page of the feed.
OLDER="$(sqlw "select journal_date from message where chat_id = '$INBOX' and deleted_at is null and body_md like '% · ' || journal_date and journal_date <= date('${TODAY}', '-20 day') and journal_date >= date('${TODAY}', '-35 day') order by journal_date desc limit 1")"
[ -n "${OLDER}" ] || fail "no first entry of a day 20..35 days back in the Inbox"
# One of the oldest entries on the first page of the Inbox (the feed loads the last 200): the first entry of a day whose text
# ends with the date, so that the search finds it near the start of the tree.
EARLY="$(sqlw "select journal_date from (select * from message where chat_id = '$INBOX' and deleted_at is null order by journal_date desc, created_at desc, id desc limit 200) where body_md like '% · ' || journal_date order by journal_date limit 1")"
EARLY_MARK="· ${EARLY}"
[ -n "${EARLY}" ] || fail "no dated entry on the first page of the Inbox"
IDEAS="$(chat_id 'Идеи')"
echo "today (journal): ${TODAY}; the day of the calendar click: ${OLDER}; the oldest loaded dated entry: ${EARLY}"

# ---- 1. Inbox with the strip (the default view) ----
set_language ru
launch_window
wait_for_window
sleep 5
detect_language
has "${L_EXPAND}" || fail "no 'Expand forest' button: the strip is not above the feed"
has "${L_STRIP}" || fail "no strip grid '${L_STRIP}'"
[ -z "$(sqlw "select value from app_setting where key = 'forest_view'")" ] || fail "forest_view is set before anyone chose it"
shoot 01-inbox-strip

# ---- 2. Expand: the isometric board of the year, the feed below it ----
press "${L_EXPAND}"
sleep 3
has "${L_BOARD}" || fail "no board '${L_BOARD}'"
[ "$(sqlw "select value from app_setting where key = 'forest_view'")" = "expanded" ] || fail "forest_view is not 'expanded' in the database"
shoot 02-expanded-whole-year

# ---- 3. Autumn zoom (a quick zoom, not a filter) ----
press "${L_AUTUMN}"
sleep 2
shoot 03-autumn-zoom

# ---- 6. The feed moves to an older day: the outlined cell follows the day at the top of the feed ----
# (before the jump of step 4: a scroll that the feed made itself is not reported until the person scrolls again)
press "${L_WHOLE}"
scroll_to "${EARLY_MARK}"
sleep 3
shoot 06-feed-older-day-selection-followed

# ---- 4. A click on the board opens the day in the feed, with the card of the selected day ----
press "${L_BOARD}" "Group|Application|Unknown|Image"
sleep 3
shoot 04-selected-day-with-card

# ---- 5. Collapse: the feed below the strip, on the day that was opened ----
press "${L_COLLAPSE}"
sleep 2
[ "$(sqlw "select value from app_setting where key = 'forest_view'")" = "strip" ] || fail "forest_view is not 'strip' after collapse"
shoot 05-feed-opened-on-the-day

# ---- 7. The previous year ----
press "${L_EXPAND}"
sleep 2
press "${L_PREV}"
sleep 3
has "Лес $((YEAR - 1)) года" || has "Forest of $((YEAR - 1))" || fail "the board did not switch to the previous year"
shoot 07-previous-year

# ---- 8. Calendar: twelve month grids; a click on a day does what a click on a tree does ----
press "${L_NEXT}"
sleep 2
press "${L_CALENDAR}"
sleep 3
shoot 08-calendar
press "$(short_date "${OLDER}"), "
sleep 3
shoot 08b-calendar-click-opened-the-day
[ "$(sqlw "select value from app_setting where key = 'forest_mode'")" = "calendar" ] || fail "forest_mode is not 'calendar' in the database"
press "${L_FOREST}"
sleep 2

# ---- 9-10. Today's tree before and after a sent note ----
press "${L_AUTUMN}"
sleep 2
shoot 09-today-before-note
NOTE="Синтетическая запись для роста сегодняшнего дерева: сто знаков, чтобы вес дня поднялся на следующую стадию роста."
sqlw "insert or replace into draft(key, body, updated_at) values ('chat:$IDEAS', '${NOTE}', $(python3 -c 'import time; print(int(time.time()*1000))'))"
BEFORE_NOTES="$(sqlw "select count(*) from message where journal_date = '${TODAY}' and deleted_at is null")"
# The chat reads its draft when it is opened.
press "${L_IDEAS}"
sleep 2
press "${L_SEND}"
sleep 4
AFTER_NOTES="$(sqlw "select count(*) from message where journal_date = '${TODAY}' and deleted_at is null")"
[ "${AFTER_NOTES}" -eq $((BEFORE_NOTES + 1)) ] || fail "the note was not sent (${BEFORE_NOTES} -> ${AFTER_NOTES} entries today)"
shoot 10-today-after-note

# ---- 11. English ----
sqlw "delete from draft where key = 'chat:$IDEAS'"
set_language en
restart_app
detect_language
shoot 11-en

# ---- 12. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
shoot 12-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 13. A 640 px window ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
shoot 13-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- 14. Hide from the strip button ----
press "${L_COLLAPSE}"
sleep 2
press "${L_HIDE}"
sleep 2
[ "$(sqlw "select value from app_setting where key = 'forest_view'")" = "hidden" ] || fail "forest_view is not 'hidden' in the database"
has "${L_EXPAND}" && fail "the forest is still on screen after Hide"
shoot 14-forest-hidden

# ---- 15. kill -9: still hidden; then Profile -> Show forest brings the strip back ----
restart_app
detect_language
has "${L_EXPAND}" && fail "the forest came back after kill -9 although it was hidden"
shoot 15-after-kill9-still-hidden
press "${L_PROFILE}"
sleep 1
press "${L_SHOW}"
sleep 3
has "${L_EXPAND}" || fail "no strip after 'Show forest' in the profile menu"
[ "$(sqlw "select value from app_setting where key = 'forest_view'")" = "strip" ] || fail "forest_view is not 'strip' after Show forest"
shoot 16-shown-again-from-profile-menu

# ---- Real-engine probe of the first weekday (WKWebView): the app log holds the source and the result ----
{
  echo "# Which locale API of the web view gave the first weekday (ADR-0013). A missing getWeekInfo is a finding, not a failure, as long as the fallback is used."
  echo "runner locale: $(defaults read -g AppleLocale 2>/dev/null)"
  echo "runner languages: $(defaults read -g AppleLanguages 2>/dev/null | tr -d '\n ')"
  echo "runner first weekday setting: $(defaults read -g AppleFirstWeekday 2>/dev/null || echo 'not set')"
  echo "macOS: $(sw_vers -productVersion)"
  grep -h "first weekday" "$LOG_FILE" 2>/dev/null | sed -E 's/^\[?[0-9T:.+ -]+\]? *//' | head -4 || true
} > "$OUT/week-info.txt"
cat "$OUT/week-info.txt"
grep -q "forest: first weekday" "$LOG_FILE" 2>/dev/null || fail "the app log has no 'forest: first weekday' line (log: $LOG_FILE)"

# ---- Privacy of the log: no dates, excerpts or chat names ----
if grep -E "$TODAY|$OLDER|Синтетическая запись|Прошёлся к реке|Книги" "$LOG_FILE" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds dates, entry text or chat names: $(head -c 300 "$K/debug/log-private.txt")"
fi

# ---- Measurements on 100 000 synthetic entries: the year table in import_bench, the cold start of the app ----
kill -9 "$(app_pid)"
sleep 2
cargo run --release -q -p kinige-store --example import_bench -- "$K/bench" synthetic 100000 > "$OUT/bench.txt" 2>&1 || fail "import_bench"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
cat "$OUT/bench.txt" "$OUT/cold-start.txt"

finish
