#!/usr/bin/env bash
# Релиз Kinige (ADR-0012). Два шага, оба явные; на main ничего не делается само.
#
#   scripts/release.sh X.Y.Z          поднять версию, ветка release/vX.Y.Z, коммит «Release vX.Y.Z», push и PR
#   scripts/release.sh --tag X.Y.Z    после мержа PR и зелёного ci.yml на main: аннотированный тег vX.Y.Z на origin/main и его push
#
# Тег запускает .github/workflows/release.yml: сборка на macOS-раннере и GitHub Release с .dmg, .zip и SHA256SUMS.
# Ключи: --dry-run (с первым шагом: всё, кроме push и PR; показывает коммит, worktree и ветка удаляются).
# Первый шаг работает во временном git worktree от origin/main: текущая ветка и неотправленные правки не затрагиваются.
set -euo pipefail

cd "$(dirname "$0")/.."

SEMVER='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'

die() {
  echo "release.sh: $*" >&2
  exit 1
}

usage() {
  sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

# Версия из [workspace.package] в тексте Cargo.toml на stdin.
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") }
    in_pkg && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]*"/)) print substr($0, RSTART + 1, RLENGTH - 2)
      exit
    }'
}

# 0, если $1 строго больше $2 (оба x.y.z, сравнение по числам).
version_greater() {
  local a b i
  IFS=. read -r -a a <<<"$1"
  IFS=. read -r -a b <<<"$2"
  for i in 0 1 2; do
    [ "${a[$i]}" -gt "${b[$i]}" ] && return 0
    [ "${a[$i]}" -lt "${b[$i]}" ] && return 1
  done
  return 1
}

check_semver() {
  [[ "$1" =~ $SEMVER ]] || die "версия «$1» не x.y.z (семвер без приставок, без ведущих нулей)"
}

# --- шаг 2: тег ------------------------------------------------------------------------------------------------------
tag_release() {
  local version="$1" tag="v$1" main_version sha
  check_semver "$version"
  git fetch --quiet origin main --tags
  main_version="$(git show origin/main:Cargo.toml | workspace_version)"
  [ "$main_version" = "$version" ] ||
    die "в origin/main версия $main_version, а не $version: сначала вмержите PR «Release ${tag}»"
  git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null && die "тег $tag уже есть локально"
  [ -z "$(git ls-remote --tags origin "refs/tags/$tag")" ] || die "тег $tag уже есть в origin"
  sha="$(git rev-parse origin/main)"
  require_green_ci "$sha"
  git tag -a "$tag" -m "Kinige $version" "$sha"
  git push origin "refs/tags/$tag"
  echo "OK: тег $tag на $(git log -1 --format='%h %s' "$sha"); сборку ведёт workflow Release:"
  echo "    gh run list --workflow release.yml --limit 1"
}

# Тег ставится только на коммит, где последний запуск ci.yml завершился успешно (запуск по push в main после мержа).
require_green_ci() {
  local sha="$1" state
  state="$(gh run list --workflow ci.yml --commit "$sha" --limit 1 --json status,conclusion \
    --jq '.[0] | "\(.status) \(.conclusion)"')" || die "не удалось спросить GitHub о CI (gh run list)"
  case "$state" in
    "completed success") ;;
    "null null") die "для коммита ${sha:0:7} нет запуска ci.yml: дождитесь CI на main или запустите его (gh workflow run ci.yml --ref main)" ;;
    "completed "*) die "CI на коммите ${sha:0:7} завершился как «${state#completed }»: тег не ставится" ;;
    *) die "CI на коммите ${sha:0:7} ещё не завершён (${state%% *}): подождите и повторите" ;;
  esac
}

# --- шаг 1: версия, ветка, PR ----------------------------------------------------------------------------------------
# root, work, branch, pushed и dry_run — глобальные: их читает ловушка EXIT, когда функция уже вернула управление.
prepare_release() {
  local version="$1" tag="v$1" current tmp log
  branch="release/v$1"
  pushed=0
  dry_run="$2"
  check_semver "$version"
  root="$PWD"

  git fetch --quiet origin main --tags
  current="$(git show origin/main:Cargo.toml | workspace_version)"
  [[ "$current" =~ $SEMVER ]] || die "в origin/main не нашлась версия в Cargo.toml"
  version_greater "$version" "$current" || die "версия $version не больше текущей $current"
  git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null && die "тег $tag уже есть"
  git show-ref --quiet --verify "refs/heads/$branch" && die "ветка $branch уже есть локально"
  [ -z "$(git ls-remote --heads origin "$branch")" ] || die "ветка $branch уже есть в origin"

  work="$(mktemp -d "${TMPDIR:-/tmp}/kinige-release.XXXXXX")"
  git worktree add --quiet -b "$branch" "$work" origin/main
  trap cleanup_worktree EXIT
  cd "$work"

  # Cargo.toml: меняется одна строка в [workspace.package]; файл пишется во временный и переносится атомарно.
  tmp="$(mktemp "${TMPDIR:-/tmp}/kinige-cargo.XXXXXX")"
  awk -v v="$version" '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") }
    in_pkg && !done && /^version[[:space:]]*=/ { print "version = \"" v "\""; done = 1; next }
    { print }
    END { if (!done) exit 1 }' Cargo.toml >"$tmp" || { rm -f "$tmp"; die "в Cargo.toml нет version в [workspace.package]"; }
  mv "$tmp" Cargo.toml

  # Cargo.lock: только крейты рабочей области, без компиляции и без сети.
  log="$(mktemp "${TMPDIR:-/tmp}/kinige-cargo-log.XXXXXX")"
  cargo update --workspace --offline >"$log" 2>&1 || { cat "$log" >&2; rm -f "$log"; die "cargo update -w --offline не удался"; }
  rm -f "$log"
  # package.json и package-lock.json: npm правит обе строки версии, тег и коммит не создаёт.
  npm --prefix web version "$version" --no-git-tag-version --allow-same-version >/dev/null

  verify_diff "$current" "$version"
  run_version_test

  git add Cargo.toml Cargo.lock web/package.json web/package-lock.json
  # Релизный коммит и PR идут без номера задачи Huly (исключение из правила 15 CLAUDE.md, ADR-0012).
  git commit --quiet -m "Release $tag"

  if [ "$dry_run" = "1" ]; then
    echo "dry-run: коммит в ветке $branch подготовлен; push и PR не делались, рабочее дерево и ветка удаляются"
    git --no-pager show --stat --format='%h %s' HEAD
    return
  fi
  git push --quiet -u origin "$branch"
  pushed=1
  gh pr create --base main --head "$branch" --title "Release $tag" --body "$(pr_body "$version" "$current")"
  echo "Дальше: дождаться CI, вмержить PR, затем: scripts/release.sh --tag $version"
}

# Тест версий в временном worktree. Зависимости: node_modules основного дерева (ссылкой), а если их нет — `npm ci` здесь
# (временный каталог удаляется вместе с worktree). Молча пропускать тест нельзя: он и ловит забытый файл версии.
run_version_test() {
  if [ -d "$root/web/node_modules" ]; then
    ln -s "$root/web/node_modules" web/node_modules
  else
    echo "release.sh: в $root/web нет node_modules, ставлю зависимости во временном worktree (npm ci)" >&2
    npm --prefix web ci --ignore-scripts --no-audit --no-fund >/dev/null || die "npm ci не удался"
  fi
  (cd web && npx vitest run src/version.test.ts) || die "web/src/version.test.ts не прошёл"
  [ ! -L web/node_modules ] || rm web/node_modules
}

# Всегда: worktree удаляется (и после --dry-run, и при ошибке), локальная ветка — пока она не отправлена в origin.
cleanup_worktree() {
  cd "$root" || true
  git worktree remove --force "$work" >/dev/null 2>&1 || true
  if [ "$pushed" = "0" ]; then
    git branch -D "$branch" >/dev/null 2>&1 || true
  fi
}

# В дифф должны попасть ровно четыре файла, а в Cargo.lock — только строки version крейтов рабочей области.
verify_diff() {
  local old="$1" new="$2" files lock
  files="$(git diff --name-only | sort | tr '\n' ' ')"
  [ "$files" = "Cargo.lock Cargo.toml web/package-lock.json web/package.json " ] ||
    die "изменились лишние файлы: $files"
  lock="$(git diff -U0 Cargo.lock | grep -E '^[+-][^+-]' || true)"
  [ -n "$lock" ] || die "Cargo.lock не изменился"
  if printf '%s\n' "$lock" | grep -Ev "^[+-]version = \"($old|$new)\"$" >/dev/null; then
    die "в Cargo.lock изменилось не только version крейтов рабочей области"
  fi
}

pr_body() {
  cat <<EOF
Версия $2 -> $1 (Cargo.toml, Cargo.lock, web/package.json, web/package-lock.json). Схема версий и порядок выпуска — ADR-0012.

После мержа: \`scripts/release.sh --tag $1\` ставит тег v$1 на main; workflow Release соберёт приложение и создаст GitHub Release.
EOF
}

# --- разбор аргументов -----------------------------------------------------------------------------------------------
mode="prepare"
dry_run=0
version=""
for arg in "$@"; do
  case "$arg" in
    --tag) mode="tag" ;;
    --dry-run) dry_run=1 ;;
    -h | --help) usage ;;
    -*) usage ;;
    *) [ -z "$version" ] || usage; version="$arg" ;;
  esac
done
[ -n "$version" ] || usage

if [ "$mode" = "tag" ]; then
  [ "$dry_run" = "0" ] || die "--dry-run относится только к первому шагу"
  tag_release "$version"
else
  prepare_release "$version" "$dry_run"
fi
