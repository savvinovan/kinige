#!/usr/bin/env bash
# Installs dependencies (if needed) and opens Storybook at http://localhost:6006.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/node_modules ] || npm --prefix web ci
exec npm --prefix web run storybook -- --no-open
