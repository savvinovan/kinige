#!/usr/bin/env bash
# ВРЕМЕННО (KINIG-40/38): проверка окна захвата и главного окна на macOS-раннере. Удаляется следующим коммитом.
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
K="$RUNNER_TEMP/kinige-canvas"
SRC_APP="$REPO/target/release/bundle/macos/Kinige.app"
APP="$K/Kinige.app"
OUT="$REPO/canvas-out"
mkdir -p "$OUT" "$K"
limit() { local s="$1"; shift; perl -e 'alarm shift; exec @ARGV' "$s" "$@"; }
app_pid() { pgrep -f "$APP/Contents/MacOS/kinige( |\$)" | head -1; }

rm -rf "$K/data" "$K/mirror" "$APP"
mkdir -p "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_demo -- "$K/data" "$K/mirror"
cargo run --release -q -p kinige-store --example seed_code -- "$K/data" "$K/mirror"
cp -R "$SRC_APP" "$APP"
plutil -insert LSEnvironment -json "{\"KINIGE_DATA_DIR\":\"$K/data\",\"KINIGE_MIRROR_DIR\":\"$K/mirror\",\"KINIGE_TMP_SHOW_CAPTURE\":\"1\"}" "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"

python3 -m venv "$K/venv" && "$K/venv/bin/pip" install -q pillow

echo "AppleShowScrollBars=$(defaults read -g AppleShowScrollBars 2>&1) AppleInterfaceStyle=$(defaults read -g AppleInterfaceStyle 2>&1)"

shoot_run() {
  local tag="$1"
  open -a "$APP"
  for _ in $(seq 1 40); do
    KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null | grep -q 'layer=0' && break
    sleep 1
  done
  sleep 4
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $(app_pid)) to true" || true
  sleep 1
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" "$OUT/$tag-main.png" kinige main || echo "нет снимка main"
  # Окно захвата показывает временный код приложения (KINIGE_TMP_SHOW_CAPTURE, через 12 с после запуска): меню значка по AX и
  # щелчок по нему на раннере меню не открыли (прогон 40861738106), а глобальное сочетание не используется.
  PID="$(app_pid)"
  for _ in $(seq 1 40); do
    KINIGE_PID="$PID" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list 2>/dev/null \
      | awk -F'\t' '$2 != "layer=0" && $2 != "layer=25" { found = 1 } END { exit !found }' && break
    sleep 1
  done
  sleep 2
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" /dev/null kinige list > "$OUT/$tag-windows.txt" 2>&1
  cat "$OUT/$tag-windows.txt"
  KINIGE_PID="$(app_pid)" limit 30 "$REPO/scripts/screenshot-window.sh" "$OUT/$tag-capture.png" kinige capture || echo "нет снимка capture"
  kill "$(app_pid)" 2>/dev/null; sleep 2
}

{
  echo "== light =="
  osascript -e 'tell application "System Events" to tell appearance preferences to set dark mode to false'; sleep 3
  shoot_run light
  echo "== dark =="
  osascript -e 'tell application "System Events" to tell appearance preferences to set dark mode to true'; sleep 3
  shoot_run dark
} 2>&1 | tee "$OUT/log.txt"

"$K/venv/bin/python" - "$OUT" <<'PY' | tee "$OUT/pixels.txt"
import sys, os
from PIL import Image
out = sys.argv[1]
for name in sorted(os.listdir(out)):
    if not name.endswith(".png"):
        continue
    im = Image.open(os.path.join(out, name)).convert("RGBA")
    w, h = im.size
    px = im.load()
    print(f"{name}: {w}x{h}")
    pts = {"(0,0)": (0, 0), "(w-1,0)": (w - 1, 0), "(0,h-1)": (0, h - 1), "(w-1,h-1)": (w - 1, h - 1),
           "(4,h/2)": (4, h // 2), "(w/2,2)": (w // 2, 2)}
    for label, p in pts.items():
        print(f"  {label:10} RGBA={px[p]}")
    if "capture" in name:
        a = [px[x, y][3] for x in range(8) for y in range(8)]
        print(f"  corner 8x8 alpha: min={min(a)} max={max(a)}")
        edge = [px[x, y] for y in range(h) for x in (0, w - 1)] + [px[x, y] for x in range(w) for y in (0, h - 1)]
        opaque_dark = [p for p in edge if p[3] > 200 and sum(p[:3]) < 200]
        print(f"  edge pixels: {len(edge)}, opaque dark: {len(opaque_dark)}")
PY
