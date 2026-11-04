#!/usr/bin/env bash
# Screens of the chat colour (KINIG-59) and the feed speed batch (KINIG-47) in the real window on a macOS runner:
# scripts/screens/chat-color.sh <built Kinige.app>. Run: gh workflow run screens.yml --ref <branch> -f slice=chat-color
# The data are synthetic (seed_demo with KINIGE_SEED_COLOR=1: chats without an emoji, one of them with an explicit grey, entries in
# them, one trashed) in a temporary directory; the real database is not touched. Input is accessibility actions only (ax.js): no
# mouse, no keyboard, no global shortcuts. A right click on a sidebar chat is the AX action AXShowMenu; the query of the search goes
# in the way slice 7 did it (the pasteboard and Edit > Paste from the menu bar).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/chat-color}"
SOURCE_APP="${1:?usage: $0 <path to the built Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# The forest strip makes the window tree large (371 cells); this batch is about the sidebar and the menus, so the forest is hidden first.
export AX_TIMEOUT=60
export DUMP_TIMEOUT=180

SETTINGS_FILE="$K/data/settings.json"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror" "$K/bench"
mkdir -p "$K/data" "$K/mirror"
KINIGE_SEED_COLOR=1 cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
sqlite3 "$K/data/kinige.db" "insert or replace into app_setting(key, value) values ('forest_view', 'hidden')"
# The shots are also copied into the debug folder when the scenario fails, so that a failed run can still be looked at.
trap 'cp "$OUT"/*.png "$K/debug/" 2> /dev/null' EXIT
prepare_app "$SOURCE_APP"
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"
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
    L_COLOUR="Цвет"; L_GREY="Серый"; L_ORANGE="Оранжевый"; L_SEARCH="Поиск"; L_SEARCH_RESULTS="Результаты поиска"
    L_TRASH="Корзина"; L_MORE="Ещё"; L_MOVE="Переместить в чат"
  else
    L_COLOUR="Colour"; L_GREY="Grey"; L_ORANGE="Orange"; L_SEARCH="Search"; L_SEARCH_RESULTS="Search results"
    L_TRASH="Trash"; L_MORE="More"; L_MOVE="Move to chat"
  fi
}

detect_language() {
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "window language: $UI"
}

# kill -9 and a launch: every start below proves that what the previous step did survives the process being killed.
restart_app() {
  kill -9 "$(app_pid)"
  sleep 2
  launch_window
  wait_for_window
  sleep 5
  detect_language
}

# What the database stores for each chat: a name or `null` (automatic). Nothing but an explicit pick is ever stored.
stored_colours() { sqlw "select title || ' = ' || coalesce(color, 'null') from chat where is_inbox = 0 or id != '00000000-0000-4000-8000-000000000001' order by sort_key"; }

# ---- 1. The sidebar as it was: no chat changed its colour, nothing was written to the database for the automatic ones ----
stored_colours > "$K/debug/colours-before-launch.txt"
set_language ru
launch_window
wait_for_window
sleep 6
detect_language
has "Проекты" || fail "the sidebar has no chat \"Проекты\" (the seed is missing)"
shoot 01-sidebar-colours
stored_colours > "$K/debug/colours-after-launch.txt"
{
  echo "# What the database stores in chat.color after the first launch (a name, or null = automatic: the colour is computed from the chat id on every read)."
  cat "$K/debug/colours-after-launch.txt"
  cmp -s "$K/debug/colours-before-launch.txt" "$K/debug/colours-after-launch.txt" \
    && echo "RESULT: the launch wrote nothing: the stored values are the same as before it" \
    || echo "RESULT: the stored values changed during the launch"
} > "$OUT/colours.txt"
cat "$OUT/colours.txt"
cmp -s "$K/debug/colours-before-launch.txt" "$K/debug/colours-after-launch.txt" || fail "the launch rewrote chat colours"
[ "$(sqlw "select count(*) from chat where color is not null")" = "1" ] || fail "exactly one chat (the seed's grey one) should have a stored colour"

# ---- 2. The chat menu of a chat shown as a letter tile, then its "Colour" submenu ----
show_menu "Проекты"
has "${L_COLOUR}" || fail "the chat menu has no \"${L_COLOUR}\" item"
press "${L_COLOUR}" MenuItem
has "${L_GREY}" || fail "the colour submenu has no \"${L_GREY}\" row"
shoot 02-colour-submenu

# ---- 3. Pick grey: the tile turns grey at once, without a toast; the database holds the name ----
press "${L_GREY}" MenuItem
sleep 2
[ "$(sqlw "select color from chat where title = 'Проекты'")" = "grey" ] || fail "the pick did not reach the database"
has "${L_GREY}" && fail "the menu stayed open after the pick"
shoot 03-picked-grey

# ---- 4. Search: the chat row and the entry rows carry the tile ----
restart_app
osascript -e 'set the clipboard to "проект"' > "$K/debug/clipboard-search.txt" 2>&1
SEARCH_FOCUS="$(ax focus "$(app_pid)" "${L_SEARCH}" 2>&1)"
sleep 1
SEARCH_MENU="$(ax menu "$(app_pid)" "Edit|Правка" "Paste|Вставить" 2>&1)"
sleep 5
{
  echo "focus: ${SEARCH_FOCUS}"
  echo "menu: ${SEARCH_MENU}"
  if has "${L_SEARCH_RESULTS}" && has "Цветной план проекта"; then
    echo "RESULT: the search results are open and the entry of the grey chat is in them"
  else
    echo "RESULT: the search results did not open or did not hold the entry after the paste (a finding)"
  fi
} > "$OUT/search.txt"
cat "$OUT/search.txt"
has "Цветной план проекта" || fail "the search did not find the entry"
shoot 04-search-tiles

# ---- 5. The trash: the plate of a deleted entry shows the chat's tile ----
restart_app
press "${L_TRASH}"
sleep 3
has "Цветная запись, которую я удалил" || fail "the trash has no entry of the chat \"Проекты\""
shoot 05-trash-plate

# ---- 6. "Move to chat": chats with a letter show their tile in their colour, the others their emoji ----
restart_app
press "${L_MORE}"
press "${L_MOVE}" MenuItem
has "Linux notes" || fail "the Move submenu has no chat \"Linux notes\""
shoot 06-move-submenu

# ---- 7. English ----
restart_app
set_language en
restart_app
show_menu "Проекты"
press "${L_COLOUR}" MenuItem
has "${L_GREY}" || fail "the colour submenu has no \"${L_GREY}\" row (English)"
shoot 07-en-colour-submenu

# ---- 8. kill -9 right after a new pick: the colour is still there ----
set_language ru
restart_app
show_menu "Мама"
press "${L_COLOUR}" MenuItem
press "${L_ORANGE}" MenuItem
sleep 2
[ "$(sqlw "select color from chat where title = 'Мама'")" = "orange" ] || fail "the second pick did not reach the database"
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 6
detect_language
{
  echo "# chat.color after kill -9 and a launch"
  stored_colours
  [ "$(sqlw "select color from chat where title = 'Проекты'")" = "grey" ] && [ "$(sqlw "select color from chat where title = 'Мама'")" = "orange" ] \
    && echo "RESULT: both picks survived" || echo "RESULT: a pick was lost"
} > "$OUT/after-kill9.txt"
cat "$OUT/after-kill9.txt"
grep -q "both picks survived" "$OUT/after-kill9.txt" || fail "a colour pick did not survive kill -9"
shoot 08-after-kill9

# ---- 9. Large text ----
edit_settings 'd["text_size"] = "large"'
sleep 2
show_menu "Проекты"
press "${L_COLOUR}" MenuItem
shoot 09-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- Privacy of the log: no chat names or entry texts ----
if sed -E 's/^\[[^]]*\]\[[^]]*\]//' "$LOG_FILE" | grep -E "Проекты|Linux notes|Мама|Цветн" > "$K/debug/log-private.txt" 2>&1; then
  fail "the app log holds chat names or entry texts: $(head -c 300 "$K/debug/log-private.txt")"
fi

# ---- Measurements on 100 000 synthetic entries (KINIG-47): the tray's recent chats and the window's reads while an agent searches ----
kill -9 "$(app_pid)"
sleep 2
cargo build --release -q -p kinige-store --example import_bench || fail "build of import_bench"
BENCH="$REPO/target/release/examples/import_bench"
[ -x "$BENCH" ] || BENCH="${CARGO_TARGET_DIR:-$REPO/target}/release/examples/import_bench"
/usr/bin/time -l "$BENCH" "$K/bench" synthetic 100000 > "$OUT/bench.txt" 2> "$K/debug/bench-time.txt" || fail "import_bench 100000"
grep -E "maximum resident|peak memory|real" "$K/debug/bench-time.txt" >> "$OUT/bench.txt"
grep -E "recent chats|AI agent" "$OUT/bench.txt" > "$OUT/feed-speed.txt"
cat "$OUT/feed-speed.txt"
[ "$(grep -c "recent chats" "$OUT/feed-speed.txt")" -ge 3 ] || fail "the bench printed no recent chats lines (the plan, before, after)"
[ "$(grep -c "AI agent" "$OUT/feed-speed.txt")" -ge 1 ] || fail "the bench printed no agent contention line"

finish
