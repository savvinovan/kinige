#!/usr/bin/env bash
# Замер холодного старта (цель < 1,5 с, §15 ТЗ). Методика — в README.
# Использование: scripts/measure-cold-start.sh [число прогонов, по умолчанию 10]
#
# Один прогон = новый процесс release-сборки; время от запуска процесса до события
# «страница загружена» (Rust печатает метку при KINIGE_TIMING=1). Это не «первый
# отрисованный кадр», а близкая к нему верхняя граница: React рисует после загрузки.
set -euo pipefail

cd "$(dirname "$0")/.."
RUNS="${1:-10}"
# Замер идёт на чистых данных (свой временный каталог): настоящую базу и Markdown-копию пользователя он не открывает и не мигрирует.
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export KINIGE_DATA_DIR="${KINIGE_DATA_DIR:-$TMP/data}" KINIGE_MIRROR_DIR="${KINIGE_MIRROR_DIR:-$TMP/mirror}"
BIN="target/release/kinige"
[ -x "$BIN" ] || { echo "нет $BIN: сначала scripts/build-app.sh" >&2; exit 1; }

echo "машина: $(sysctl -n machdep.cpu.brand_string), $(sysctl -n hw.model), macOS $(sw_vers -productVersion)"
echo "сборка: release ($BIN), прогонов: $RUNS"

python3 - "$BIN" "$RUNS" <<'PY'
import os, statistics, subprocess, sys, time

binary, runs = sys.argv[1], int(sys.argv[2])
times = []
for i in range(runs):
    started = time.perf_counter()
    proc = subprocess.Popen(
        [binary], env={**os.environ, "KINIGE_TIMING": "1"},
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    elapsed = None
    for line in proc.stderr:
        if line.startswith("kinige-timing: page-loaded"):
            elapsed = (time.perf_counter() - started) * 1000
            break
    proc.kill()
    proc.wait()
    if elapsed is None:
        sys.exit("приложение закрылось, не сообщив о загрузке страницы")
    times.append(elapsed)
    print(f"  прогон {i + 1:2d}: {elapsed:7.1f} мс")
    time.sleep(1)  # дать системе освободить окно перед следующим запуском

print(f"первый: {times[0]:.0f} мс; min {min(times):.0f}, медиана {statistics.median(times):.0f}, max {max(times):.0f} мс")
print("цель §15: < 1500 мс —", "выполнена" if statistics.median(times) < 1500 else "НЕ выполнена")
PY
