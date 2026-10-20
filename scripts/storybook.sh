#!/usr/bin/env bash
# Ставит зависимости (если нужно) и открывает Storybook на http://localhost:6006.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/node_modules ] || npm --prefix web ci
exec npm --prefix web run storybook -- --no-open
