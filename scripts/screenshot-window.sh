#!/usr/bin/env bash
# Снимает окно Kinige (не весь экран) в PNG: screencapture -l <windowid>.
# Использование: scripts/screenshot-window.sh <out.png> [имя процесса, по умолчанию kinige]
# Терминалу нужно разрешение «Запись экрана» (Системные настройки → Конфиденциальность).
set -euo pipefail

OUT="${1:?использование: $0 <out.png> [имя процесса]}"
OWNER="${2:-kinige}"

# Ищем окно по владельцу среди окон обычного слоя (0) на экране; берём самое большое.
WINDOW_ID="$(osascript -l JavaScript - "$OWNER" <<'JXA'
ObjC.import("CoreGraphics");
ObjC.import("Foundation");
function run(argv) {
  const owner = argv[0].toLowerCase();
  const raw = $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, $.kCGNullWindowID);
  const list = ObjC.deepUnwrap(ObjC.castRefToObject(raw));
  const found = list
    .filter((w) => String(w.kCGWindowOwnerName).toLowerCase() === owner && w.kCGWindowLayer === 0)
    .sort((a, b) => b.kCGWindowBounds.Width * b.kCGWindowBounds.Height
                  - a.kCGWindowBounds.Width * a.kCGWindowBounds.Height);
  return found.length ? String(found[0].kCGWindowNumber) : "";
}
JXA
)"

[ -n "$WINDOW_ID" ] || { echo "окно процесса '$OWNER' не найдено (приложение запущено?)" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
screencapture -x -o -l "$WINDOW_ID" "$OUT"
echo "окно $WINDOW_ID -> $OUT"
