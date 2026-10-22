#!/usr/bin/env bash
# Снимает окно Kinige (не весь экран) в PNG: screencapture -l <windowid>.
# Использование:
#   scripts/screenshot-window.sh <out.png> [имя процесса, по умолчанию kinige] [режим]
# Режимы (какое окно процесса снимать):
#   main     — самое большое окно обычного слоя 0 (главное окно; по умолчанию);
#   capture  — окно быстрого захвата: без заголовка, поверх остальных (слой выше 0), шириной до 700 px;
#   menu     — выпавшее меню значка в menu bar (слой выше 0, окно без заголовка уже 400 px и ниже 400 px);
#   list     — только напечатать окна процесса (id, слой, размеры, имя) и выйти.
# Если запущено несколько экземпляров (например, рабочий и проверочный), `KINIGE_PID=<pid>` оставляет окна одного процесса.
# Окно с прозрачными углами снимается с альфа-каналом (`-o` убирает только системную тень, своя тень в CSS остаётся).
# Снимается всегда одно окно процесса: ни рабочего стола, ни других программ на снимке нет.
# Терминалу нужно разрешение «Запись экрана» (Системные настройки → Конфиденциальность).
set -euo pipefail

OUT="${1:?использование: $0 <out.png> [имя процесса] [main|capture|menu|list]}"
OWNER="${2:-kinige}"
MODE="${3:-main}"

WINDOW_ID="$(osascript -l JavaScript - "$OWNER" "$MODE" "${KINIGE_PID:-}" <<'JXA'
ObjC.import("CoreGraphics");
ObjC.import("Foundation");
function run(argv) {
  const owner = argv[0].toLowerCase();
  const mode = argv[1];
  const raw = $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, $.kCGNullWindowID);
  const list = ObjC.deepUnwrap(ObjC.castRefToObject(raw));
  const pid = argv[2] ? Number(argv[2]) : null;
  const mine = list.filter(
    (w) => String(w.kCGWindowOwnerName).toLowerCase() === owner && (pid === null || w.kCGWindowOwnerPID === pid),
  );
  const area = (w) => w.kCGWindowBounds.Width * w.kCGWindowBounds.Height;
  if (mode === "list") {
    return mine
      .map((w) => `${w.kCGWindowNumber}\tlayer=${w.kCGWindowLayer}\t${w.kCGWindowBounds.Width}x${w.kCGWindowBounds.Height}\tx=${w.kCGWindowBounds.X} y=${w.kCGWindowBounds.Y}\t${w.kCGWindowName || ""}`)
      .join("\n");
  }
  let found;
  if (mode === "capture") {
    found = mine.filter((w) => w.kCGWindowLayer > 0 && w.kCGWindowBounds.Width <= 700 && w.kCGWindowBounds.Height >= 90);
  } else if (mode === "menu") {
    found = mine.filter((w) => w.kCGWindowLayer > 0 && w.kCGWindowBounds.Width < 400 && w.kCGWindowBounds.Height < 400 && w.kCGWindowBounds.Height > 40);
  } else {
    found = mine.filter((w) => w.kCGWindowLayer === 0);
  }
  found.sort((a, b) => area(b) - area(a));
  return found.length ? String(found[0].kCGWindowNumber) : "";
}
JXA
)"

if [ "$MODE" = "list" ]; then
  echo "$WINDOW_ID"
  exit 0
fi

[ -n "$WINDOW_ID" ] || { echo "окно процесса '$OWNER' (режим $MODE) не найдено (приложение запущено, окно показано?)" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
screencapture -x -o -l "$WINDOW_ID" "$OUT"
echo "окно $WINDOW_ID -> $OUT"
