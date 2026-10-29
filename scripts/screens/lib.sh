#!/usr/bin/env bash
# Общие помощники сценариев снимков настоящего окна на macOS (workflow screens.yml). Подключается из scripts/screens/<слайс>.sh.
# Ждёт от сценария переменные: REPO (корень), K (рабочий каталог), APP ($K/Kinige.app), OUT (куда класть PNG и тексты).
# Ввод — только действия доступности (AXPress, AXScrollToVisible) через scripts/screens/ax.js: ни мыши, ни клавиатуры,
# ни глобальных сочетаний. Рабочие данные — синтетические, в $K/data и $K/mirror.

FAILS=0
PAUSE="${PAUSE:-1.5}"
mkdir -p "$OUT" "$K/debug"

app_pid() { pgrep -f "$APP/Contents/MacOS/kinige( |\$)" | head -1; }
ax() { osascript -l JavaScript "$REPO/scripts/screens/ax.js" "$@"; }

# Приложение со временными каталогами данных: LSEnvironment в копии бандла, ad-hoc подпись заново.
prepare_app() {
  local source="$1"
  rm -rf "$APP"
  cp -R "$source" "$APP"
  plutil -insert LSEnvironment -json \
    "{\"KINIGE_DATA_DIR\":\"$K/data\",\"KINIGE_MIRROR_DIR\":\"$K/mirror\"}" "$APP/Contents/Info.plist"
  codesign --force --deep -s - "$APP"
}

launch_background() { open -g -a "$APP" --args --background; }
launch_window() { open -a "$APP"; }

wait_for_window() {
  for _ in $(seq 1 40); do
    if KINIGE_PID="$(app_pid)" "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null | grep -q 'layer=0'; then
      return 0
    fi
    sleep 1
  done
  echo "окно не появилось"
  FAILS=$((FAILS + 1))
}

wait_for_process() {
  for _ in $(seq 1 40); do
    [ -n "$(app_pid)" ] && return 0
    sleep 1
  done
  echo "процесс не запустился"
  FAILS=$((FAILS + 1))
}

dump() { ax dump "$(app_pid)" > "$K/debug/$1.txt" 2>&1; }

fail() {
  echo "ОШИБКА: $*"
  FAILS=$((FAILS + 1))
  dump "fail-$FAILS"
}

press() {
  local result
  result="$(ax press "$(app_pid)" "$@" 2>&1)"
  echo "press $*: $result"
  case "$result" in
    pressed*) ;;
    *) fail "не нажато: $*" ;;
  esac
  sleep "$PAUSE"
}

scroll_to() {
  local result
  result="$(ax scroll "$(app_pid)" "$1" 2>&1)"
  echo "scroll $1: $result"
  case "$result" in
    scrolled*) ;;
    *) fail "не прокручено: $1" ;;
  esac
  sleep "$PAUSE"
}

has() { [ "$(ax has "$(app_pid)" "$1" 2>&1)" = "yes" ]; }

# Снимок только окна Kinige. Перед ним проверяется, что в тексте окна нет чужих путей и имён (личные данные не попадут в репозиторий).
shoot() {
  local name="$1"
  ax front "$(app_pid)" > /dev/null 2>&1
  sleep 1
  dump "shot-$name"
  if grep -E '/Users/[^r]|/Users/r[^u]|/private/tmp|/var/folders|alexandr|savvinovan' "$K/debug/shot-$name.txt" > "$K/debug/shot-$name.private" 2>&1; then
    fail "в тексте окна $name личные пути или имена: $(head -c 300 "$K/debug/shot-$name.private")"
  fi
  KINIGE_PID="$(app_pid)" "$REPO/scripts/screenshot-window.sh" "$OUT/$name.png" kinige main || fail "нет снимка $name"
}

sql() { sqlite3 "$K/data/kinige.db" "$@"; }
chat_id() { sql "select id from chat where title = '$1'"; }

finish() {
  echo "ошибок сценария: $FAILS"
  [ "$FAILS" -eq 0 ]
}
