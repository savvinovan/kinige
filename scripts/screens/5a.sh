#!/usr/bin/env bash
# Screens of slice 5a (KINIG-138, reactions) in the real window on a macOS runner: scripts/screens/5a.sh <built Kinige.app>.
# Run: gh workflow run screens.yml --ref <branch> -f slice=5a. The data are synthetic (seed_demo with KINIGE_SEED_REACTIONS=1: reactions on today's
# Inbox entries, ten on one of them, and on the entries of the "On this day" card, one put a year after its entry) in a temporary directory; the
# real database is not touched. Input is the screens driver inside the app (KINIG-92, ADR-0016; DRIVER=1): it presses, right-clicks, types and now
# focuses (the `focus` step, KINIG-138: a script cannot hover, so the hover bar is shown by the focus inside the entry, which is what `:focus-within`
# does for a person who tabs to it). No mouse, no keyboard, no global shortcuts.
#
# The feed's markup is in time order (oldest first), so today's four entries are the last four of every kind of button: "Купить билеты" is the
# last, "Идея" before it, "Планы на выходные" before that and "Проснулся рано" before that. Each press is checked in the database, so a wrong
# index is an error and not a wrong shot.
#
# What it proves in the real window: a click on an emoji of the bar puts a chip (`rows.txt`: the group and the derived row, before and after), the
# reactions and their times survive `kill -9`, search shows the chips and does not look inside them, the trash shows them, the Markdown copy has the
# line (`mirror.txt`), ten reactions disable the buttons that would add, and the screens are the same in English, in large text and at 640 px.
# `bench.txt` and `cold-start.txt` are the 100 000-entry measurements with reactions on a tenth of the entries.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5a}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
DRIVER=1
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SETTINGS_FILE="$K/data/settings.json"
# U+2764 U+FE0F as bytes: the file holds no invisible character, and bash 3.2 of the runner has no \u escape.
HEART=$'\xe2\x9d\xa4\xef\xb8\x8f'

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror" "$OUT"
# The screens driver must be in this build, or the scenario has no hands.
"$REPO/scripts/check-test-hooks.sh" "$REPO/web/dist" --binary "$SOURCE_APP/Contents/MacOS/kinige" --expect screens-driver || { echo "ERROR: the screens driver is not in this build"; FAILS=$((FAILS + 1)); }
KINIGE_SEED_REACTIONS=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
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
  if [ "$UI" = "ru" ]; then
    L_INBOX="Входящие"; L_TRASH="Корзина"; L_SEARCH="Поиск"; L_NONE="Ничего не найдено."; L_RESULTS="Результаты поиска"
    L_HEART_BTN="Реакция $HEART"; L_ADD="Поставить реакцию"; L_ADD_CARD="Поставить реакцию на запись"; L_DIALOG="Выбор реакции"; L_MORE="Ещё эмодзи"
    L_ADD_ITEM="Поставить реакцию…"; L_LATE="1 год спустя"; L_REACT_WORD="Реакция"; L_EMOJI_SEARCH="Найти эмодзи"; L_FIRE_WORD="огонь"
    L_LIMIT="На записи не больше 10 реакций"
  else
    L_INBOX="Inbox"; L_TRASH="Trash"; L_SEARCH="Search"; L_NONE="Nothing found."; L_RESULTS="Search results"
    L_HEART_BTN="Reaction $HEART"; L_ADD="Add reaction"; L_ADD_CARD="Add a reaction to the entry"; L_DIALOG="Choose a reaction"; L_MORE="More emoji"
    L_ADD_ITEM="Add reaction…"; L_LATE="1 year later"; L_REACT_WORD="Reaction"; L_EMOJI_SEARCH="Search emoji"; L_FIRE_WORD="fire"
    L_LIMIT="At most 10 reactions on an entry"
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

# press_nth / focus_nth <text> <n> [role]: the n-th (from 0) element whose name holds the text.
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
# How many buttons carry the label. The window text lists a button twice (its aria-label and its title, which are the same words), except a button
# that cannot add: its title is the label and the reason, so it is listed once. At most one button of a label is in that state here (the entry that
# has ten), so the lines divided by two, rounded up, are the buttons.
count_buttons() {
  local lines
  lines="$(count_label "$1")" || return 1
  echo $(((lines + 1) / 2))
}
# The places of today's four entries among the bars the feed has drawn now (the markup is oldest first, today's entries are the last four).
locate_today() {
  COUNT="$(count_buttons "$L_HEART_BTN")"
  [ -n "$COUNT" ] && [ "$COUNT" -ge 4 ] || fail "the feed shows ${COUNT:-?} bars, today's four entries need at least four"
  LAST=$((COUNT - 1))
  FIRST_OF_TODAY=$((COUNT - 4))
}
# The key of Escape, to the focused element.
escape_key() { drive "$(drive_window)" '{"kind":"key","key":"Escape","code":"Escape"}' > /dev/null 2>&1 || true; sleep "$PAUSE"; }

# The newest live entry that starts with the text: the "On this day" seed has older entries with the same first words.
entry_id() { sqlw "select id from message where body_md like '$1%' and deleted_at is null order by created_at desc limit 1"; }
live_rows() { sqlw "select count(*) from message_reaction where message_id = '$1'"; }
rows_block() {
  local group hlc recent
  {
    echo "[$1]"
    sqlw "select substr(m.body_md, 1, 18), r.key, r.emoji, r.put_tz, datetime(r.put_at / 1000, 'unixepoch') from message_reaction r join message m on m.id = r.message_id order by m.created_at, r.put_at"
    # An empty value leaves no space at the end of the line (the files in docs/screens are checked for trailing whitespace).
    group="$(sqlw "select json_extract(groups_raw, '\$.reactions') from message where id = '${AWAKE:-}'")"
    echo "group of the entry the scenario reacts to:${group:+ $group}"
    hlc="$(sqlw "select json_extract(hlc_json, '\$.reactions.h') from message where id = '${AWAKE:-}'")"
    echo "hlc of the group:${hlc:+ $hlc}"
    recent="$(sqlw "select value from app_setting where key = 'recent_reactions'")"
    echo "recent list:${recent:+ $recent}"
  } >> "$OUT/rows.txt"
}

: > "$OUT/rows.txt"

# ---- 1. The Inbox: chips under the text, the hover bar shown by the focus ----
set_language ru
launch_window
wait_for_window
wait_name "Входящие" 60000 || fail "the window did not show the Inbox"
sleep 4
detect_language
wait_name "Купить билеты" 30000 || fail "the Inbox did not show today's entries"
locate_today
AWAKE="$(entry_id 'Проснулся рано')"
[ -n "$AWAKE" ] || fail "no entry to react to"
[ "$(live_rows "$AWAKE")" = "0" ] || fail "the entry to react to already has a reaction"
rows_block "before any reaction of the scenario"

# 01: the bar of the entry that has none: three quick emoji, "Add reaction", a divider, then the usual buttons. (The entry's own words are also the
# first words of an entry of a past year, so the scroll goes to the next entry, which is only today's.)
scroll_to "Идея для приложения"
focus_on "$L_HEART_BTN" button "$FIRST_OF_TODAY"
sleep 2
shoot 01-hover-bar
has "$L_ADD" || fail "the bar has no Add reaction button"

# ---- 2. A click on the heart: a chip under the text, nothing flies, no toast ----
press_nth "$L_HEART_BTN" "$FIRST_OF_TODAY" button
sleep 3
[ "$(live_rows "$AWAKE")" = "1" ] || fail "the click on the heart did not put a reaction on the entry (index $FIRST_OF_TODAY of $COUNT)"
shoot 02-after-heart
rows_block "after the heart"
has "Снять" || has "Remove" || fail "no chip is on screen after the click"

# ---- 3. The picker: recent reactions on top, a search, a pick ----
focus_on "$L_ADD" button "$FIRST_OF_TODAY"
press_nth "$L_ADD" "$FIRST_OF_TODAY" button
wait_name "$L_DIALOG" 10000 || fail "the picker did not open"
type_into "$L_EMOJI_SEARCH" "$L_FIRE_WORD"
sleep 2
shoot 03-picker
press "$L_FIRE_WORD" gridcell
sleep 3
[ "$(live_rows "$AWAKE")" = "2" ] || fail "the pick from the picker did not put a second reaction"
wait_name "$L_DIALOG" 10000 gone || fail "the picker did not close after a pick"
rows_block "after the pick"
# Whichever emoji the search put first: the popover below offers it, as the latest of the recent ones.
PICKED="$(sqlw "select emoji from message_reaction where message_id = '$AWAKE' and key != '$(sqlw "select key from message_reaction where message_id = '$AWAKE' order by put_at limit 1")' limit 1")"
[ -n "$PICKED" ] || fail "no second emoji in the entry's rows"
L_PICKED_BTN="$L_REACT_WORD $PICKED"

# ---- 4. Several chips, the late one: the card's entry carries "1 year later" ----
scroll_to "Купить билеты"
sleep 2
shoot 04-several-chips
has "$L_LATE" || fail "no chip says it was put a year later"

# ---- 5. The card: the smile button, the small popover, a pick, the chip under the entry ----
scroll_to "$L_ADD_CARD"
focus_on "$L_ADD_CARD" button 0
press_nth "$L_ADD_CARD" 0 button
wait_name "$L_DIALOG" 10000 || fail "the card's popover did not open"
sleep 1
shoot 05-card-popover
CARD_ENTRY="$(sqlw "select message_id from message_reaction where emoji = '🥲' limit 1")"
BEFORE_CARD="$(sqlw "select count(*) from message_reaction where message_id = '$CARD_ENTRY'")"
# The popover's quick emoji are the recent ones first (the pick and the heart of the steps above), then the fixed ones; its buttons come last in the markup.
COUNT_PICKED="$(count_buttons "$L_PICKED_BTN")"
[ "$COUNT_PICKED" -ge 1 ] || fail "the popover has no button for the emoji picked above"
press_nth "$L_PICKED_BTN" $((COUNT_PICKED - 1)) button
sleep 3
wait_name "$L_DIALOG" 10000 gone || fail "the popover did not close after a pick"
[ "$(sqlw "select count(*) from message_reaction where message_id = '$CARD_ENTRY' and emoji = '$PICKED'")" = "1" ] || fail "the pick in the card's popover put nothing on the card's entry"
# The line under the card's first entry ("1 more entry of this day") is scrolled to, so the entry and its new chip are in the shot.
scroll_to "запись этого дня"
sleep 2
shoot 05b-card-chip
rows_block "after the card popover"
echo "reactions on the first card entry: before $BEFORE_CARD, after $(sqlw "select count(*) from message_reaction where message_id = '$CARD_ENTRY'")" >> "$OUT/rows.txt"

# ---- 6. kill -9: the reactions and their times are in place ----
ROWS_BEFORE="$(sqlw "select message_id, key, emoji, put_at, put_tz from message_reaction order by 1, 2")"
GROUPS_BEFORE="$(sqlw "select id, groups_raw, hlc_json from message where groups_raw like '%\"reactions\"%' order by id")"
restart_app
wait_name "Купить билеты" 30000 || fail "no entries after kill -9"
[ "$(sqlw "select message_id, key, emoji, put_at, put_tz from message_reaction order by 1, 2")" = "$ROWS_BEFORE" ] || fail "the reactions changed across kill -9"
[ "$(sqlw "select id, groups_raw, hlc_json from message where groups_raw like '%\"reactions\"%' order by id")" = "$GROUPS_BEFORE" ] || fail "the groups changed across kill -9"
scroll_to "Купить билеты"
sleep 3
shoot 06-after-kill-9
has "Снять" || has "Remove" || fail "no chips on screen after kill -9"
rows_block "after kill -9"

# ---- 7. Search: the entry shows with its chips; search does not look inside the reactions ----
type_into "$L_SEARCH" "билеты"
wait_name "$L_RESULTS" 10000 > /dev/null 2>&1 || true
sleep 4
if has "$L_NONE"; then fail "search for the text of an entry found nothing"; fi
shoot 07-search
type_into "$L_SEARCH" "огонь"
sleep 5
{
  echo "search for the name of a reaction (the fire):"
  if has "$L_NONE"; then echo "nothing found (as it should be)"; else echo "something was found"; fi
} >> "$OUT/search.txt"
has "$L_NONE" || fail "search found an entry by the name of its reaction"
type_into "$L_SEARCH" ""
sleep 3

# ---- 8. The trash: the chips are there and cannot be changed ----
press "$L_TRASH"
sleep 4
shoot 08-trash
has "Реакции" || has "Reactions" || fail "the trash entry shows no chips"
if has "$L_HEART_BTN"; then fail "the trash has reaction buttons"; fi
press "$L_INBOX"
sleep 3

# ---- 9. The context menu: Add reaction… opens the small popover ----
scroll_to "Идея для приложения"
show_menu "Идея для приложения"
wait_name "$L_ADD_ITEM" 10000 "" menuitem || fail "the entry's menu has no Add reaction item"
shoot 09-menu-add-reaction
press "$L_ADD_ITEM" menuitem
wait_name "$L_DIALOG" 10000 || fail "the menu item did not open the popover"
shoot 09b-menu-popover
press "$L_MORE" button
sleep 2
shoot 09c-menu-popover-full
escape_key
wait_name "$L_DIALOG" 10000 gone || fail "Esc did not close the popover"

# ---- 10. Ten reactions: the buttons that would add are muted and say why ----
scroll_to "Планы на выходные"
locate_today
focus_on "$L_HEART_BTN" button $((COUNT - 3))
sleep 2
shoot 10-ten-reactions
has "$L_LIMIT" || fail "the bar of a full entry does not say why it cannot add"

# ---- 11. The Markdown copy: the line under the entry ----
sleep 5
{
  echo "# The reaction lines of the Markdown copy (an emoji and the day it was put on, joined by a dot)"
  find "$K/mirror" -type f -name '*.md' -exec grep -h ' 20[0-9][0-9]-[0-9][0-9]-[0-9][0-9]\( · \|$\)' {} + 2> /dev/null | grep -v '^#\|^↩\|^!\|^\[' | sort
} > "$OUT/mirror.txt"
grep -q '🔥 20' "$OUT/mirror.txt" || fail "the Markdown copy has no reaction line"
cat "$OUT/mirror.txt"

# ---- 12. English ----
set_language en
restart_app
wait_name "Купить билеты" 30000 || fail "no entries in the English window"
[ "$UI" = "en" ] || fail "the window is not in English after the restart"
scroll_to "Купить билеты"
sleep 3
shoot 11-english

# ---- 13. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
scroll_to "Купить билеты"
sleep 3
shoot 12-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 14. A 640 px window ----
echo "window: $(ax resize "$(app_pid)" 640 800)"
sleep 3
scroll_to "Купить билеты"
sleep 3
locate_today
focus_on "$L_HEART_BTN" button "$LAST"
sleep 2
shoot 13-narrow-640
echo "window: $(ax resize "$(app_pid)" 1280 800)"
sleep 3

# ---- Measurements: 100 000 entries, reactions on a tenth of them: opening Inbox, the cold start ----
kill -9 "$(app_pid)"
sleep 2
rm -rf "$K/data/open"
cargo build --release -q -p kinige-store --example import_bench || fail "build of import_bench"
BENCH="$REPO/target/release/examples/import_bench"
[ -x "$BENCH" ] || BENCH="${CARGO_TARGET_DIR:-$REPO/target}/release/examples/import_bench"
"$BENCH" "$K/bench" synthetic 100000 > "$OUT/bench.txt" 2>&1 || fail "import_bench 100000"
grep -q 'opening Inbox with reactions' "$OUT/bench.txt" || fail "the bench did not measure opening Inbox with reactions"
P95="$(sed -nE 's/.*opening Inbox with reactions.*p95 ([0-9.]+) ms.*/\1/p' "$OUT/bench.txt" | head -1)"
[ -n "$P95" ] && awk -v p="$P95" 'BEGIN { exit !(p < 300) }' || fail "opening Inbox with reactions on a tenth of 100 000 entries: p95 ${P95:-unknown} ms is not under 300 ms"
{
  KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start-control-before.txt" 2>&1 || fail "cold start control (before)"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "cold start measurement"
{
  KINIGE_BIN="$APP/Contents/MacOS/kinige" limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start-control-after.txt" 2>&1 || fail "cold start control (after)"
rm -rf "$K/bench"
cat "$OUT/bench.txt" "$OUT/cold-start.txt"

finish
