#!/usr/bin/env bash
# Снимки слайса 4a («Доступ ИИ-агентов», MCP) в настоящем окне на macOS-раннере: scripts/screens/4a.sh <собранный Kinige.app>.
# Данные — синтетические (seed_demo) во временном каталоге; настоящая база не трогается. Результат — $OUT (docs/screens/4a).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/4a}"
SOURCE_APP="${1:?использование: $0 <путь к собранному Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

SOCKET="$K/data/mcp/kinige.sock"
BRIDGE="$APP/Contents/MacOS/kinige-mcp"
EVIDENCE="$OUT/background-launch.txt"

# Старые снимки уходят целиком: имена и набор могли измениться.
rm -f "$OUT"/*.png
rm -rf "$K/data" "$K/mirror"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
prepare_app "$SOURCE_APP"
WORK="$(chat_id 'Работа')"
BOOKS="$(chat_id 'Книги')"
INBOX="00000000-0000-4000-8000-000000000001"

dock_has_kinige() {
  limit 30 osascript -e 'tell application "System Events" to get name of every UI element of list 1 of process "Dock"' 2>&1 | grep -qw Kinige
}
app_type() { lsappinfo info -only ApplicationType "$(lsappinfo find pid="$(app_pid)")" 2>&1 | sed 's/.*=//'; }

# ---- 1. Запуск в фоне, как его делает мост: без значка в Dock и без окна; основной сокет; затем показ окна ----
{
  echo "# Запуск в фоне: open -g -a Kinige.app --args --background, синтетический каталог данных на раннере GitHub (macOS)."
  launch_background
  wait_for_process
  sleep 4
  echo "после --background: ApplicationType=$(app_type)  (UIElement = политика Accessory, значка в Dock нет)"
  if dock_has_kinige; then echo "Dock: значок Kinige ЕСТЬ"; fail "в фоне значок Kinige в Dock"; else echo "Dock: значка Kinige нет"; fi
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list | grep -c 'layer=0' | sed 's/^/окон обычного слоя: /'
  # Значок в menu bar: второе меню-бар процесса (status items) через System Events и окно значка (слой 25) среди окон процесса.
  items="$(limit 30 osascript -e "tell application \"System Events\" to tell (first process whose unix id is $(app_pid)) to get count of menu bar items of menu bar 2" 2>&1)"
  windows="$(KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list | grep -c 'layer=25')"
  echo "значок в menu bar: элементов во втором меню-баре процесса: $items; окон значка (слой 25): $windows"
  case "$items" in
    '' | *[!0-9]*) items=0 ;;
  esac
  if [ "$items" -ge 1 ] || [ "$windows" -ge 1 ]; then echo "значок в menu bar есть"; else fail "в фоне нет значка в menu bar"; fi
  echo "основной сокет (не запасной в TMPDIR):"
  stat -f '%Sp  %N' "$K/data/mcp" "$SOCKET"
  [ "$(stat -f %Lp "$SOCKET")" = "600" ] && [ "$(stat -f %Lp "$K/data/mcp")" = "700" ] || fail "права сокета"
  limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$K/debug/session-disabled.txt" || fail "сеанс по основному сокету"
  grep -o 'mcp_disabled[^"]*' "$K/debug/session-disabled.txt" | head -1 | sed 's/^/вызов при выключенном доступе: /'
  echo "процесс: $(ps -o command= -p "$(app_pid)" | sed 's#.*/Contents#.../Contents#')"
  launch_window
  wait_for_window
  sleep 2
  echo "после показа окна (open без аргументов, Reopen): ApplicationType=$(app_type)  (Foreground = Regular, значок в Dock)"
  if dock_has_kinige; then echo "Dock: значок Kinige есть"; else echo "Dock: значка Kinige нет"; fail "при окне значка нет в Dock"; fi
} > "$EVIDENCE" 2>&1
cat "$EVIDENCE"

# ---- 2. Настройки → строка «Доступ ИИ-агентов» → окно: выключено ----
press "Profile"
press "Settings"
scroll_to "AI agent access"
shoot 01-settings-agent-row
press "Configure"
shoot 02-access-off
# Доступ выключен, а журнал виден: в нём отказы на пробный сеанс из первого шага («denied: access is off»).
scroll_to "Access log"
has "denied: access is off" || fail "в журнале при выключенном доступе нет строк отказа"
shoot 02b-access-off-log
scroll_to "Allow agents to access the journal"
# ПРОБА (временно): второй «язычок» — случайный кадр после открытия окна или состояние «выключено»? Тот же вид позже.
sleep 8
shoot zz-probe-1-off-later
has "$SOCKET" && echo "путь основного сокета виден в окне" || fail "основного пути сокета нет в тексте окна ($SOCKET)"

# ---- 3. Включение: предупреждение, режим, чаты ----
press "Allow agents to access the journal"
shoot 03-warning
press "Turn on"
press "Read and write"
press "Работа" CheckBox
press "Inbox" CheckBox
shoot 04-access-on
# ПРОБА (временно): «выключено» второй раз, уже после включения.
press "Allow agents to access the journal" CheckBox
scroll_to "Allow agents to access the journal"
sleep 3
shoot zz-probe-2-off-again
press "Allow agents to access the journal" CheckBox
press "Turn on"

# ---- 4. Сеанс агента через мост из бандла: чтение, запись, закрытый чат ----
limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$OUT/mcp-session.txt" --chat "$WORK" --closed-chat "$BOOKS" || fail "сеанс агента"
sleep 4
scroll_to "Chats visible to the agent"
shoot 05-dialog-after-agent
scroll_to "Access log"
shoot 06-access-log
press "Show entries of this request"
shoot 07-access-log-expanded

# ---- 5. Режим «только чтение»: вызовы записи отклонены ----
scroll_to "Mode"
press "Read only"
limit 60 python3 "$REPO/scripts/mcp-session.py" "$BRIDGE" "$SOCKET" "$OUT/mcp-session-read-only.txt" --chat "$WORK" || fail "сеанс только чтение"
sleep 4
scroll_to "Access log"
shoot 08-access-log-denied
scroll_to "Mode"
press "Read and write"

# ---- 6. Лента: запись агента ----
press "Close"
press "Close"
# Чаты в сайдбаре — AXPopUpButton с названием и числом записей («Работа , entries: 2»).
press "Работа , entries" PopUpButton
shoot 09-feed-agent-entry

# ---- 7. kill -9 и перезапуск: данные на месте, старый сокет заменяется ----
kill -9 "$(app_pid)"
sleep 2
launch_window
wait_for_window
sleep 2
press "Работа , entries" PopUpButton
shoot 10-after-kill9-restart
press "Profile"
press "Settings"
scroll_to "AI agent access"
press "Configure"
scroll_to "Access log"
shoot 11-access-log-after-kill9

# ---- 8. Чат, исключённый из AI (значение пишется в базу напрямую: переключателя в интерфейсе нет) ----
sql "update chat set ai_excluded = 1 where id = '$BOOKS'"
sleep 4
scroll_to "Chats visible to the agent"
shoot 15-chat-excluded-from-ai

# ---- 9. Крупный текст: настройки, окно, журнал ----
press "Close"
press "Large"
scroll_to "Text size"
shoot 12-settings-large-text
scroll_to "AI agent access"
press "Configure"
scroll_to "Chats visible to the agent"
shoot 13-access-large-text
scroll_to "Access log"
shoot 14-access-log-large-text

finish
