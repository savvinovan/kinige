#!/usr/bin/env bash
# Проверка помощников lib.sh без macOS: `shoot` обязан падать на пустом или оборванном дампе окна (KINIG-45), иначе проверка
# личных путей молча ничего не проверяет. Запуск: bash scripts/screens/lib.test.sh (в CI — задание web).
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
K="$WORK/k"
OUT="$WORK/out"
APP="$WORK/Kinige.app"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# Подмены: приложение «есть», снимок «сделан», обход дерева отвечает тем, что задано в DUMP_ANSWER.
app_pid() { echo 1; }
limit() { return 0; }
sleep() { :; }
ax() {
  case "$1" in
    dump) printf '%b' "$DUMP_ANSWER" ;;
    *) echo ok ;;
  esac
}

PROBLEMS=0
expect_fails() {
  local name="$1" want="$2"
  FAILS=0
  shoot "case" > /dev/null 2>&1
  if [ "$want" = "fail" ] && [ "$FAILS" -eq 0 ]; then
    echo "НЕ ПАДАЕТ: $name"
    PROBLEMS=$((PROBLEMS + 1))
  elif [ "$want" = "pass" ] && [ "$FAILS" -ne 0 ]; then
    echo "ПАДАЕТ НЕ ПО ДЕЛУ: $name"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

DUMP_ANSWER='' expect_fails "пустой дамп" fail
DUMP_ANSWER='Alarm clock\n' expect_fails "оборван по времени" fail
DUMP_ANSWER='no process' expect_fails "нет процесса" fail
DUMP_ANSWER='AXGroup Лента\n AXButton Отправить\n' expect_fails "настоящий дамп без личных путей" pass
DUMP_ANSWER='AXGroup /Users/someone/Documents\n' expect_fails "настоящий дамп с чужим путём" fail

[ "$PROBLEMS" -eq 0 ] && echo "lib.sh: проверки прошли"
exit "$PROBLEMS"
