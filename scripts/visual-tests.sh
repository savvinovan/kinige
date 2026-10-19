#!/usr/bin/env bash
# Визуальные тесты Storybook в Linux-контейнере (как в CI).
#   scripts/visual-tests.sh           — сравнить с эталонами
#   scripts/visual-tests.sh --update  — перезаписать эталоны и скопировать их в репозиторий
# Docker удалённый (ubuntu-server): файлы идут через build context и `docker cp`, не bind-mount.
set -euo pipefail

cd "$(dirname "$0")/.."
IMAGE="kinige-visual:local"
NAME="kinige-visual-$$"
SNAPSHOTS="tests/visual/stories.spec.ts-snapshots"

docker build -q -f docker/visual.Dockerfile -t "$IMAGE" .
trap 'docker rm -f "$NAME" >/dev/null 2>&1 || true' EXIT

if [ "${1:-}" = "--update" ]; then
  docker run --name "$NAME" "$IMAGE" npx playwright test --update-snapshots
  rm -rf "web/$SNAPSHOTS"
  docker cp "$NAME:/app/web/$SNAPSHOTS" "web/$SNAPSHOTS"
  echo "эталоны обновлены: web/$SNAPSHOTS"
else
  docker run --name "$NAME" "$IMAGE"
fi
