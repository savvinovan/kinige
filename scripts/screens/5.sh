#!/usr/bin/env bash
# Снимки слайса 5 («В этот день» и ответ «N лет спустя») в настоящем окне на macOS-раннере: scripts/screens/5.sh <собранный Kinige.app>.
# Данные — синтетические (seed_demo) во временном каталоге; настоящая база не трогается. Результат — $OUT (docs/screens/5).
# Ввод — только действия доступности (ax.js): ни мыши, ни клавиатуры, ни глобальных сочетаний. Текста с клавиатуры нет, поэтому
# недописанный ответ кладёт в черновик сам сценарий через sql (как это сделал бы другой процесс до перезапуска приложения),
# а «запись ушла в корзину» делается правкой базы снаружи, после чего лента открывается заново.
# Язык интерфейса — русский (`AppleLanguages` в домене приложения), в конце английский; подписи кнопок берутся по языку,
# который показывает окно (`UI`), поэтому сценарий не ломается, если системе не удалось подставить язык.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
K="${K:-${RUNNER_TEMP:-/tmp}/kinige-screens}"
APP="$K/Kinige.app"
OUT="${OUT:-$REPO/docs/screens/5}"
SOURCE_APP="${1:?использование: $0 <путь к собранному Kinige.app>}"
# shellcheck source=lib.sh
. "$REPO/scripts/screens/lib.sh"

# Обход дерева окна с лентой, карточкой и сайдбаром занимает больше минуты (каждое свойство элемента — отдельный запрос к
# System Events), поэтому нажатия и поиск ждут долго; проверка личных путей в дампе (вся она по синтетическим данным
# раннера) ограничена коротко, иначе каждый снимок тянул бы минуты.
export AX_TIMEOUT=150
dump() { AX_TIMEOUT=15 ax dump "$(app_pid)" > "$K/debug/$1.txt" 2>&1; }

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

# Правка settings.json «снаружи» (как в 4b.sh): временный файл и замена. Код меняет словарь d.
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

# Дата «31 октября 2023» / «October 31, 2023» из YYYY-MM-DD на языке окна.
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

# Подписи окна на двух языках.
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
  # По названию чата Inbox в сайдбаре: он стоит в начале дерева, поиск быстрый (карточки может и не быть, и тогда поиск идёт по всему дереву).
  if has "Входящие"; then UI=ru; else UI=en; fi
  labels
  echo "язык окна: $UI"
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
[ -n "${OLD_ID}" ] || fail "в базе нет записи трёхлетней давности"
echo "сегодня по журналу: ${TODAY}; старая запись: ${OLD_DATE}"

# ---- 1. Inbox с карточкой ----
set_language ru
launch_window
wait_for_window
sleep 3
detect_language
OLD_LABEL="$(long_date "${OLD_DATE}")"
has "${L_HIDE}" || fail "в окне нет карточки «В этот день» (кнопки «${L_HIDE}»)"
# Карточка высокая: лента открыта у нижнего края, заголовок карточки с первым годом прокручивается в видимое место.
scroll_to "${L_HIDE}"
shoot 01-inbox-card

# ---- 2. «Ответить» у записи трёхлетней давности: строка над полем ----
press "${L_REPLY} ${OLD_LABEL}"
sleep 2
[ "$(sqlw "select reply_to_id from draft where key = 'chat:$INBOX'")" = "${OLD_ID}" ] || fail "черновик с ответом не сохранён в базе"
shoot 02-reply-line

# ---- 3. Текст ответа кладёт в черновик сценарий; kill -9 и запуск: текст и строка на месте ----
sqlw "update draft set body = 'До сих пор так думаю' where key = 'chat:$INBOX'"
restart_app
detect_language
shoot 03-draft-after-restart

# ---- 4. «Отправить»: ответ в ленте с цитатой ----
press "${L_SEND}"
sleep 2
[ "$(sqlw "select reply_to_id from message where body_md = 'До сих пор так думаю'")" = "${OLD_ID}" ] || fail "в базе у ответа нет reply_to_id"
[ -z "$(sqlw "select 1 from draft where key = 'chat:$INBOX'")" ] || fail "черновик не очищен отправкой"
shoot 04-reply-sent
# Markdown-копия: под заголовком времени ответа строка со ссылкой на файл дня старой записи.
sleep 3
MIRROR_FILE="$K/mirror/Inbox/${TODAY%%-*}/${TODAY}.md"
if [ -f "$MIRROR_FILE" ]; then
  cp "$MIRROR_FILE" "$OUT/mirror-reply.md"
  grep -q '↩ \[' "$OUT/mirror-reply.md" || fail "в файле дня нет строки ↩"
else
  fail "нет файла дня $MIRROR_FILE"
fi

# ---- 5. Щелчок по цитате: старая запись в контексте ----
press "${L_QUOTE} ${OLD_LABEL}"
sleep 2
shoot 05-old-entry-from-quote

# ---- 6. Старая запись ушла в корзину (правка базы снаружи): цитата «Запись в корзине» ----
sqlw "update message set deleted_at = $(python3 -c 'import time; print(int(time.time()*1000))') where id = '${OLD_ID}'"
press "${L_BOOKS}"
press "${L_INBOX}"
sleep 2
shoot 06-quote-in-trash
sqlw "update message set deleted_at = NULL where id = '${OLD_ID}'"

# ---- 7. Крестик карточки ----
press "${L_HIDE}"
sleep 2
[ "$(sqlw "select value from app_setting where key = 'on_this_day_dismissed'")" = "${TODAY}" ] || fail "скрытие не записано на сегодняшний день журнала"
shoot 07-card-dismissed

# ---- 8. kill -9: карточки нет, ответ с цитатой на месте ----
restart_app
detect_language
[ "$(sqlw "select reply_to_id from message where body_md = 'До сих пор так думаю'")" = "${OLD_ID}" ] || fail "после kill -9 у ответа нет reply_to_id"
shoot 08-after-kill9

# ---- 9. Чат «Книги»: карточки нет ----
press "${L_BOOKS}"
shoot 09-books-chat

# ---- 11. Крупный текст (скрытие снято, возврат в Inbox читает карточку заново) ----
sqlw "delete from app_setting where key = 'on_this_day_dismissed'"
edit_settings 'd["text_size"] = "large"'
press "${L_INBOX}"
sleep 2
has "${L_HIDE}" || fail "после снятия скрытия карточки нет"
# Карточка высокая: лента открыта у нижнего края, заголовок карточки с первым годом прокручивается в видимое место.
scroll_to "${L_HIDE}"
shoot 11-large-text
edit_settings 'd["text_size"] = "normal"'

# ---- 12–13. Импортированный личный чат Telegram: цитаты без подписи «N лет спустя» ----
press "${L_ANNA}"
sleep 2
shoot 12-telegram-chat-quotes
press "${L_PLAIN_QUOTE} $(long_date 2023-03-14)"
sleep 2
shoot 13-telegram-quote-opened

# ---- 10. Английский ----
press "${L_INBOX}"
sleep 1
sqlw "delete from app_setting where key = 'on_this_day_dismissed'"
set_language en
restart_app
detect_language
has "${L_HIDE}" || fail "в английском окне нет карточки"
# Карточка высокая: лента открыта у нижнего края, заголовок карточки с первым годом прокручивается в видимое место.
scroll_to "${L_HIDE}"
shoot 10-en

# ---- Замеры на 100 000 синтетических записей: карточка в import_bench, холодный старт приложения ----
kill -9 "$(app_pid)"
sleep 2
cargo run --release -q -p kinige-store --example import_bench -- "$K/bench" synthetic 100000 > "$OUT/bench.txt" 2>&1 || fail "замер import_bench"
{
  KINIGE_DATA_DIR="$K/bench" KINIGE_MIRROR_DIR="$K/bench/mirror" KINIGE_BIN="$APP/Contents/MacOS/kinige" \
    limit 600 "$REPO/scripts/measure-cold-start.sh" 10
} > "$OUT/cold-start.txt" 2>&1 || fail "замер холодного старта"
cat "$OUT/bench.txt" "$OUT/cold-start.txt"

finish
