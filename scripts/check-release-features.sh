#!/usr/bin/env bash
# The guard against shipping the tests' hooks in a build for people (KINIG-103): the cargo feature `test-hooks` of `kinige-store`
# and `kinige` (injected panics and faults, probes of the internals) is switched on only by the dev-dependencies of the crates' own
# tests. This script lists the features that a release build compiles (a graph with no dev-dependencies: what
# `cargo build --release -p kinige` and `tauri build` use) and fails if `test-hooks` is on in it.
#   scripts/check-release-features.sh [cargo tree arguments]
#       default: every member of the workspace. The arguments replace `--workspace`, so a control can force the feature on:
#       `scripts/check-release-features.sh -p kinige -F kinige-store/test-hooks` must fail (scripts/check-release-features.test.sh).
# Exit codes: 0 clean, 1 the feature is on, 2 the check could not run (cargo failed, e.g. an unknown feature, or the list of crates is stale).
# The tree is inverted (`-i <crate>`): a forward `cargo tree` does not print a feature that only the command line turns on, while the
# inverted one lists every feature that is on for the crate, with whatever asked for it.
set -euo pipefail

cd "$(dirname "$0")/.."
FEATURE="test-hooks"
# The crates that define the feature. A new one must be added here (the check below fails until it is).
CRATES=(kinige-store kinige)
[ $# -gt 0 ] || set -- --workspace

# Every manifest that defines the feature is in CRATES, otherwise the guard would not look at it.
DEFINING="$(grep -rlE "^$FEATURE[[:space:]]*=" --include=Cargo.toml crates src-tauri server | sort | tr '\n' ' ')"
EXPECTED="crates/kinige-store/Cargo.toml src-tauri/Cargo.toml "
if [ "$DEFINING" != "$EXPECTED" ]; then
  echo "the crates that define the feature $FEATURE changed: $DEFINING(expected: $EXPECTED); update CRATES in $0" >&2
  exit 2
fi

PROBLEMS=0
for crate in "${CRATES[@]}"; do
  # Captured first: a failing `cargo tree` (a typo in the arguments, a broken lockfile) must not look like an empty, clean graph.
  # `--target=all`: without it `cargo tree` resolves the host's target only, and a `[target.'cfg(windows)'.dependencies]` entry that turns the
  # feature on would pass on a Mac or Linux runner and ship in the Windows installer.
  GRAPH="$(cargo tree --locked -e no-dev,features -i "$crate" --target=all "$@")" || { echo "cargo tree failed for $crate" >&2; exit 2; }
  # A sanity check on the output itself: the crate's own default feature is always there.
  # (A here-string, not a pipe: `grep -q` closing the pipe early would fail the pipeline under pipefail.)
  grep -qF "$crate feature \"default\"" <<< "$GRAPH" || { echo "cargo tree shows no features of $crate; the check cannot see anything" >&2; exit 2; }
  FOUND="$(grep -F "$crate feature \"$FEATURE\"" <<< "$GRAPH" || true)"
  if [ -n "$FOUND" ]; then
    echo "the feature $FEATURE of $crate is on in the release dependency graph (cargo tree $*):" >&2
    sort -u <<< "$FOUND" >&2
    PROBLEMS=$((PROBLEMS + 1))
  fi
done
if [ "$PROBLEMS" -gt 0 ]; then
  echo "It is for tests only: a dev-dependency may turn it on, nothing else." >&2
  exit 1
fi
echo "the release dependency graph does not enable $FEATURE (cargo tree $*)"
