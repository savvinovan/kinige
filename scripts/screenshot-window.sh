#!/usr/bin/env bash
# Captures the Kinige window (not the whole screen) to a PNG: screencapture -l <windowid>.
# Usage:
#   scripts/screenshot-window.sh <out.png> [process name, kinige by default] [mode]
# Modes (which window of the process to capture):
#   main     - the largest window of the normal layer 0 (the main window; default);
#   capture  - the quick capture window: no title, above the others (layer above 0), up to 700 px wide;
#   settings - the "Settings" window (slice 4b): normal layer 0, title "Settings" or "Настройки" (the Russian UI title);
#   menu     - the dropdown menu of the menu bar icon (layer above 0, a window without a title, narrower than 400 px and shorter than 400 px);
#   list     - only print the windows of the process (id, layer, size, name) and exit.
# If several instances are running (for example, a working one and a test one), `KINIGE_PID=<pid>` keeps the windows of one process.
# A window with transparent corners is captured with the alpha channel (`-o` removes only the system shadow, the CSS shadow stays).
# Always one window of the process is captured: there is no desktop or other programs in the shot.
# The terminal needs the "Screen Recording" permission (System Settings -> Privacy).
set -euo pipefail

OUT="${1:?usage: $0 <out.png> [process name] [main|capture|settings|menu|list]}"
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
  } else if (mode === "settings") {
    found = mine.filter((w) => w.kCGWindowLayer === 0 && /^(Settings|Настройки)$/.test(String(w.kCGWindowName || "")));
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

[ -n "$WINDOW_ID" ] || { echo "no window of the process '$OWNER' (mode $MODE) found (is the app running, is the window shown?)" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
screencapture -x -o -l "$WINDOW_ID" "$OUT"
echo "window $WINDOW_ID -> $OUT"
