#!/usr/bin/env bash
# Cold start measurement (target < 1.5 s, PRD §15). The method is in the README.
# Usage: scripts/measure-cold-start.sh [number of runs, 10 by default]
#
# One run = a new process of the release build; the time from the process start to the
# "page loaded" event (Rust writes the mark to stderr via `log` when KINIGE_TIMING=1). This is not the "first
# painted frame" but an upper bound close to it: React paints after the load.
set -euo pipefail

cd "$(dirname "$0")/.."
RUNS="${1:-10}"
# The measurement runs on clean data (its own temporary directory): it neither opens nor migrates the user's real database and Markdown copy.
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export KINIGE_DATA_DIR="${KINIGE_DATA_DIR:-$TMP/data}" KINIGE_MIRROR_DIR="${KINIGE_MIRROR_DIR:-$TMP/mirror}"
BIN="${KINIGE_BIN:-${CARGO_TARGET_DIR:-target}/release/kinige}"
[ -x "$BIN" ] || { echo "no $BIN: run scripts/build-app.sh first" >&2; exit 1; }

echo "machine: $(sysctl -n machdep.cpu.brand_string), $(sysctl -n hw.model), macOS $(sw_vers -productVersion)"
echo "build: release ($BIN), runs: $RUNS"

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
        # Log line: `[date][time][INFO][kinige-timing] page-loaded` (see src-tauri/src/logging.rs).
        if "[kinige-timing]" in line and line.rstrip().endswith("page-loaded"):
            elapsed = (time.perf_counter() - started) * 1000
            break
    proc.kill()
    proc.wait()
    if elapsed is None:
        sys.exit("the app exited without reporting the page load")
    times.append(elapsed)
    print(f"  run {i + 1:2d}: {elapsed:7.1f} ms")
    time.sleep(1)  # let the system release the window before the next launch

print(f"first: {times[0]:.0f} ms; min {min(times):.0f}, median {statistics.median(times):.0f}, max {max(times):.0f} ms")
print("target §15: < 1500 ms -", "met" if statistics.median(times) < 1500 else "NOT met")
PY
