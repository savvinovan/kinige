#!/usr/bin/env bash
# Kinige release (ADR-0012). Two steps, both explicit; nothing happens on main by itself.
#
#   scripts/release.sh X.Y.Z          bump the version, branch release/vX.Y.Z, commit "Release vX.Y.Z", push and PR
#   scripts/release.sh --tag X.Y.Z    after the PR is merged and ci.yml on main is green: an annotated tag vX.Y.Z on origin/main and its push
#
# The tag starts .github/workflows/release.yml: builds on a macOS and a Windows runner and a GitHub Release with the .dmg, .zip, the Windows installer .exe and one SHA256SUMS.
# Flags: --dry-run (with the first step: everything except push and PR; shows the commit, the worktree and the branch are removed).
# The first step works in a temporary git worktree from origin/main: the current branch and unpushed changes are not touched.
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

# The version from [workspace.package] in the Cargo.toml text on stdin.
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") }
    in_pkg && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]*"/)) print substr($0, RSTART + 1, RLENGTH - 2)
      exit
    }'
}

# 0 if $1 is strictly greater than $2 (both x.y.z, compared numerically).
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
  [[ "$1" =~ $SEMVER ]] || die "version '$1' is not x.y.z (semver without suffixes, without leading zeros)"
}

# --- step 2: the tag -------------------------------------------------------------------------------------------------
tag_release() {
  local version="$1" tag="v$1" main_version sha
  check_semver "$version"
  git fetch --quiet origin main --tags
  main_version="$(git show origin/main:Cargo.toml | workspace_version)"
  [ "$main_version" = "$version" ] ||
    die "origin/main has version $main_version, not $version: merge the PR 'Release ${tag}' first"
  git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists locally"
  [ -z "$(git ls-remote --tags origin "refs/tags/$tag")" ] || die "tag $tag already exists in origin"
  sha="$(git rev-parse origin/main)"
  require_green_ci "$sha"
  git tag -a "$tag" -m "Kinige $version" "$sha"
  git push origin "refs/tags/$tag"
  echo "OK: tag $tag on $(git log -1 --format='%h %s' "$sha"); the build is run by the Release workflow:"
  echo "    gh run list --workflow release.yml --limit 1"
}

# The tag is put only on a commit where the last ci.yml run succeeded (the run on push to main after the merge).
require_green_ci() {
  local sha="$1" state
  state="$(gh run list --workflow ci.yml --commit "$sha" --limit 1 --json status,conclusion \
    --jq '.[0] | "\(.status) \(.conclusion)"')" || die "could not ask GitHub about CI (gh run list)"
  case "$state" in
    "completed success") ;;
    "null null") die "commit ${sha:0:7} has no ci.yml run: wait for CI on main or start it (gh workflow run ci.yml --ref main)" ;;
    "completed "*) die "CI on commit ${sha:0:7} finished as '${state#completed }': the tag is not put" ;;
    *) die "CI on commit ${sha:0:7} has not finished yet (${state%% *}): wait and retry" ;;
  esac
}

# --- step 1: the version, the branch, the PR -------------------------------------------------------------------------
# root, work, branch, pushed and dry_run are global: the EXIT trap reads them when the function has already returned.
prepare_release() {
  local version="$1" tag="v$1" current tmp log
  branch="release/v$1"
  pushed=0
  dry_run="$2"
  check_semver "$version"
  root="$PWD"

  git fetch --quiet origin main --tags
  current="$(git show origin/main:Cargo.toml | workspace_version)"
  [[ "$current" =~ $SEMVER ]] || die "no version found in Cargo.toml in origin/main"
  version_greater "$version" "$current" || die "version $version is not greater than the current $current"
  git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists"
  git show-ref --quiet --verify "refs/heads/$branch" && die "branch $branch already exists locally"
  [ -z "$(git ls-remote --heads origin "$branch")" ] || die "branch $branch already exists in origin"

  work="$(mktemp -d "${TMPDIR:-/tmp}/kinige-release.XXXXXX")"
  git worktree add --quiet -b "$branch" "$work" origin/main
  trap cleanup_worktree EXIT
  cd "$work"

  # Cargo.toml: one line in [workspace.package] is changed; the file is written to a temporary one and moved atomically.
  tmp="$(mktemp "${TMPDIR:-/tmp}/kinige-cargo.XXXXXX")"
  awk -v v="$version" '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") }
    in_pkg && !done && /^version[[:space:]]*=/ { print "version = \"" v "\""; done = 1; next }
    { print }
    END { if (!done) exit 1 }' Cargo.toml >"$tmp" || { rm -f "$tmp"; die "Cargo.toml has no version in [workspace.package]"; }
  mv "$tmp" Cargo.toml

  # Cargo.lock: only the workspace crates, without compiling and without the network.
  log="$(mktemp "${TMPDIR:-/tmp}/kinige-cargo-log.XXXXXX")"
  cargo update --workspace --offline >"$log" 2>&1 || { cat "$log" >&2; rm -f "$log"; die "cargo update -w --offline failed"; }
  rm -f "$log"
  # package.json and package-lock.json: npm edits both version lines, creates no tag and no commit.
  npm --prefix web version "$version" --no-git-tag-version --allow-same-version >/dev/null

  verify_diff "$current" "$version"
  run_version_test

  git add Cargo.toml Cargo.lock web/package.json web/package-lock.json
  # The release commit and PR go without a Huly task number (an exception to rule 15 of CLAUDE.md, ADR-0012).
  git commit --quiet -m "Release $tag"

  if [ "$dry_run" = "1" ]; then
    echo "dry-run: the commit on branch $branch is prepared; no push and no PR, the worktree and the branch are removed"
    git --no-pager show --stat --format='%h %s' HEAD
    return
  fi
  git push --quiet -u origin "$branch"
  pushed=1
  gh pr create --base main --head "$branch" --title "Release $tag" --body "$(pr_body "$version" "$current")"
  echo "Next: wait for CI, merge the PR, then: scripts/release.sh --tag $version"
}

# The version test in the temporary worktree. Dependencies: node_modules of the main tree (by a link), and if absent, `npm ci` here
# (the temporary directory is removed together with the worktree). The test must not be skipped silently: it catches a forgotten version file.
run_version_test() {
  if [ -d "$root/web/node_modules" ]; then
    ln -s "$root/web/node_modules" web/node_modules
  else
    echo "release.sh: $root/web has no node_modules, installing dependencies in the temporary worktree (npm ci)" >&2
    npm --prefix web ci --ignore-scripts --no-audit --no-fund >/dev/null || die "npm ci failed"
  fi
  (cd web && npx vitest run src/version.test.ts) || die "web/src/version.test.ts failed"
  [ ! -L web/node_modules ] || rm web/node_modules
}

# Always: the worktree is removed (after --dry-run and on error), the local branch too unless it has been pushed to origin.
cleanup_worktree() {
  cd "$root" || true
  git worktree remove --force "$work" >/dev/null 2>&1 || true
  if [ "$pushed" = "0" ]; then
    git branch -D "$branch" >/dev/null 2>&1 || true
  fi
}

# The diff must contain exactly four files, and in Cargo.lock only the version lines of the workspace crates.
verify_diff() {
  local old="$1" new="$2" files lock
  files="$(git diff --name-only | sort | tr '\n' ' ')"
  [ "$files" = "Cargo.lock Cargo.toml web/package-lock.json web/package.json " ] ||
    die "extra files changed: $files"
  lock="$(git diff -U0 Cargo.lock | grep -E '^[+-][^+-]' || true)"
  [ -n "$lock" ] || die "Cargo.lock did not change"
  if printf '%s\n' "$lock" | grep -Ev "^[+-]version = \"($old|$new)\"$" >/dev/null; then
    die "Cargo.lock changed in more than the version of the workspace crates"
  fi
}

pr_body() {
  cat <<EOF
Version $2 -> $1 (Cargo.toml, Cargo.lock, web/package.json, web/package-lock.json). The version scheme and the release procedure are in ADR-0012.

After the merge: \`scripts/release.sh --tag $1\` puts the tag v$1 on main; the Release workflow builds the app and creates a GitHub Release.
EOF
}

# --- argument parsing ------------------------------------------------------------------------------------------------
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
  [ "$dry_run" = "0" ] || die "--dry-run applies only to the first step"
  tag_release "$version"
else
  prepare_release "$version" "$dry_run"
fi
