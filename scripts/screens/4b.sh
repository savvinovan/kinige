#!/usr/bin/env bash
# Снимки слайса 4b (окно «Настройки» и файл settings.json) в настоящем окне на macOS-раннере: scripts/screens/4b.sh <собранный Kinige.app>.
# Данные — синтетические (seed_demo) во временном каталоге; настоящая база не трогается. Результат — $OUT (docs/screens/4b).
# Ввод — только действия доступности (ax.js): ни мыши, ни клавиатуры, ни глобальных сочетаний. Правки файла настроек делает
# сам сценарий («редактор» или агент): временный файл рядом и mv, как делают редакторы.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/4b}"
SOURCE_APP="${1:?использование: $0 <путь к собранному Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SETTINGS_FILE="$K/data/settings.json"
SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"
INBOX="00000000-0000-4000-8000-000000000001"

rm -f "$OUT"/*.png "$OUT"/*.txt
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
WORK="$(chat_id 'Работа')"
BOOKS="$(chat_id 'Книги')"

in_settings() { export AX_WINDOW=Settings; }
in_main() { export AX_WINDOW=Kinige; }
shoot_settings() { in_settings; shoot "$1" settings; }
shoot_main() { in_main; shoot "$1" main; in_settings; }
settings_windows() { KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null | awk -F'\t' '$5 == "Settings" || $5 == "Настройки"' | wc -l | tr -d ' '; }

wait_for_settings() {
  for _ in $(seq 1 30); do
    [ "$(settings_windows)" -ge 1 ] && return 0
    sleep 1
  done
  fail "окно настроек не появилось"
}

# Правка файла «снаружи»: временный файл и mv (режим 0600). Аргумент — содержимое.
write_settings() {
  printf '%s' "$1" > "$K/data/.editor.tmp"
  chmod 600 "$K/data/.editor.tmp"
  mv "$K/data/.editor.tmp" "$SETTINGS_FILE"
  sleep 4
}

# Правка файла через python: код меняет словарь d.
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

setting() { sql "select value from app_setting where key = '$1'"; }

# ---- 1. Запуск, главное окно ----
launch_window
wait_for_window
sleep 2
{
  echo "# Окно «Настройки» открывается из профиля; повторное нажатие поднимает то же окно, второго не появляется."
  echo "окон «Settings» до нажатия: $(settings_windows)"
} > "$OUT/one-window.txt"
in_main
press "Profile"
press "Settings"
wait_for_settings
sleep 2
echo "после первого нажатия: $(settings_windows)" >> "$OUT/one-window.txt"
press "Profile"
press "Settings"
sleep 2
echo "после второго нажатия: $(settings_windows)" >> "$OUT/one-window.txt"
[ "$(settings_windows)" = "1" ] || fail "окон настроек не одно"
cat "$OUT/one-window.txt"

# ---- 2. Разделы дерева ----
in_settings
shoot_settings 01-appearance
press "Keyboard shortcuts"
shoot_settings 02-shortcuts
press "Copies"
shoot_settings 03-mirror
press "AI agent access"
shoot_settings 04-access-off

# ---- 3. Включение: предупреждение поверх окна, содержимое под ним не сдвигается ----
# Пока предупреждение открыто (aria-modal), WebKit прячет остальную страницу от доступности, поэтому рамки под ним
# не читаются. Здесь сверяются рамки до показа и после закрытия предупреждения; во время показа геометрию (в том числе с
# постоянными полосами прокрутки) проверяет tests/visual/scrollbars.spec.ts, а на снимке 05 страница под подложкой видна.
{
  echo "# Рамки (x y ширина высота, пиксели экрана) до показа предупреждения и после его закрытия («Отмена»)."
  echo "флажок доступа до:    $(ax frame "$(app_pid)" "Allow agents to access the journal")"
  echo "пункт дерева до:      $(ax frame "$(app_pid)" "Copies")"
} > "$OUT/no-shift.txt"
press "Allow agents to access the journal"
has "Turn on access" || has "Turn on" || fail "предупреждение не показано"
shoot_settings 05-warning
press "Cancel"
{
  echo "флажок доступа после: $(ax frame "$(app_pid)" "Allow agents to access the journal")"
  echo "пункт дерева после:   $(ax frame "$(app_pid)" "Copies")"
} >> "$OUT/no-shift.txt"
cat "$OUT/no-shift.txt"
before_box="$(sed -n 's/^флажок доступа до: *//p' "$OUT/no-shift.txt")"
after_box="$(sed -n 's/^флажок доступа после: *//p' "$OUT/no-shift.txt")"
[ -n "$before_box" ] && [ "$before_box" = "$after_box" ] || fail "флажок сдвинулся при предупреждении ($before_box -> $after_box)"
press "Allow agents to access the journal"
press "Turn on"
press "Read and write"

# ---- 4. Подэкран «Видимые чаты» ----
press "Configure"
press "Inbox" CheckBox
shoot_settings 06-visible-chats

# ---- 5. Сеанс агента через мост: журнал на несколько страниц ----
for round in 1 2 3; do
  limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$K/debug/session-$round.txt" --chat "$INBOX" --closed-chat "$BOOKS" || fail "сеанс агента $round"
done
sleep 4
press "Access log"
scroll_to "Show more"
shoot_settings 07-access-log-long

# ---- 6. Файл настроек без ошибок; файл до и после правки из окна ----
press "Settings file"
shoot_settings 08-file-ok
{
  echo "# settings.json: синтетический файл на раннере. Права файла (ls -l) и содержимое."
  echo "== как записало приложение =="
  ls -l "$SETTINGS_FILE"
  cat "$SETTINGS_FILE"
} > "$OUT/settings-file.txt"
# Чужой ключ в файле («агент» дописал заметку): приложение его хранит и после правки из окна.
edit_settings 'd["x_my_note"] = "keep me"'
press "Appearance"
press "Large"
sleep 3
{
  echo
  echo "== в файл дописан чужой ключ x_my_note; в окне выбран «Large» =="
  ls -l "$SETTINGS_FILE"
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
grep -q '"x_my_note": "keep me"' "$SETTINGS_FILE" && grep -q '"text_size": "large"' "$SETTINGS_FILE" || fail "правка из окна потеряла чужой ключ или не записалась"
[ "$(stat -f %Lp "$SETTINGS_FILE")" = "600" ] || fail "права файла не 0600"
press "Normal"
sleep 3
edit_settings 'd.pop("x_my_note", None)'

# ---- 7. Внешняя правка: размер текста и чат «Работа» в доступе агента ----
cp "$SETTINGS_FILE" "$K/debug/settings-good.json"
edit_settings 'd["text_size"] = "large"; d["agent_access"]["chats"].append({"title": "Работа"})'
{
  echo
  echo "== внешняя правка: text_size = large, в agent_access.chats добавлен чат «Работа» (по названию) =="
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
[ "$(setting text_size)" = "large" ] || fail "text_size из файла не применён"
sql "select value from app_setting where key = 'mcp_chats'" | grep -q "$WORK" || fail "чат из файла не отмечен"
press "AI agent access"
has "Changed via settings.json" || fail "нет строки «Изменено через settings.json»"
shoot_settings 09-after-external-edit
press "Access log"
has "access changed" || fail "в журнале нет строки «settings.json · access changed»"
shoot_settings 09b-access-log-after-edit
shoot_main 10-main-large-text

# ---- 8. Файл не разобран ----
write_settings '{"text_size": '
press "Settings file"
has "could not be read" || fail "нет сообщения «Файл не разобран»"
shoot_settings 11-file-unreadable
[ "$(setting text_size)" = "large" ] || fail "битый файл изменил настройки"

# ---- 9. Ошибки нескольких ключей ----
write_settings '{"text_size":"huge","shortcuts":{"capture":"???"},"mirror":{"folder":"relative/path"},"agent_access":{"mode":"write","chats":[{"id":"no-such-chat"}]},"surprise":1}'
has "text_size: this value is not allowed" || fail "нет ошибки text_size"
has "shortcuts.capture: this shortcut cannot be used" || fail "нет ошибки сочетания"
has "mirror.folder: the folder path must be absolute" || fail "нет ошибки папки копии"
has "agent_access.mode: this value is not allowed" || fail "нет ошибки режима"
has "agent_access.chats[0]: no such chat" || fail "нет ошибки чата"
has "surprise" || fail "нет предупреждения о неизвестном ключе"
shoot_settings 12-file-key-errors
[ "$(setting text_size)" = "large" ] || fail "ошибки ключей изменили настройки"
write_settings "$(cat "$K/debug/settings-good.json" | python3 -c 'import json,sys; d=json.load(sys.stdin); d["text_size"]="large"; print(json.dumps(d))')"

# ---- 10. Крупный текст ----
press "AI agent access"
shoot_settings 13-large-text

# ---- 11. Наименьший размер окна ----
{
  echo "# Окно уменьшено до 100 x 100: наименьший размер задан окну (640 x 440), меньше оно не становится."
  echo "размер после запроса: $(ax resize "$(app_pid)" 100 100)"
} > "$OUT/min-size.txt"
cat "$OUT/min-size.txt"
sleep 2
shoot_settings 14-min-size

# ---- 12. kill -9 после правки в окне и после правки файла ----
press "Appearance"
press "Normal"
sleep 3
edit_settings 'd["text_size"] = "large"'
kill -9 "$(app_pid)"
sleep 2
{
  echo
  echo "== kill -9 после правки в окне (Normal) и правки файла (large) =="
  cat "$SETTINGS_FILE"
} >> "$OUT/settings-file.txt"
launch_window
wait_for_window
sleep 2
in_main
press "Profile"
press "Settings"
wait_for_settings
sleep 2
in_settings
press "Appearance"
shoot_settings 15-after-kill9
{
  echo
  echo "== после перезапуска =="
  cat "$SETTINGS_FILE"
  echo "text_size в базе: $(setting text_size)"
  echo "доступ агента: $(setting mcp_enabled), режим: $(setting mcp_mode), чатов: $(setting mcp_chats | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')"
} >> "$OUT/settings-file.txt"
[ "$(setting text_size)" = "large" ] || fail "после kill -9 text_size не large"

finish
