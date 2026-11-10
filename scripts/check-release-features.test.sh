#!/usr/bin/env bash
# Controls of scripts/check-release-features.sh: it must pass on the real release graph and fail, with the right exit code, when the
# feature is forced on (a guard that never fired proves nothing). Run: bash scripts/check-release-features.test.sh (in CI, the rust job).
# Exit codes of the guard: 0 clean, 1 the feature is on, 2 the check could not run. Each control names the exact code it wants, so a guard
# that fails for another reason (cargo itself failing, say) does not count as having detected anything.
#
# Not tested: `--workspace -F <crate>/test-hooks`. On cargo 1.94 `cargo tree` panics on it (`missing dep graph connection for CLI feature`),
# so `-F` is combined with `-p kinige`, whose graph contains `kinige-store`.
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
GUARD="$REPO/scripts/check-release-features.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PROBLEMS=0
check() {
  local name="$1" want="$2" got
  shift 2
  "$@" > /dev/null 2>&1
  got=$?
  if [ "$got" -ne "$want" ]; then
    echo "FAIL: $name (wanted exit $want, got $got)"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

check "the release graph of the whole workspace is clean" 0 "$GUARD"
check "the release graph of the app is clean" 0 "$GUARD" -p kinige
check "forcing the store's feature through the app is found" 1 "$GUARD" -p kinige -F kinige-store/test-hooks
check "forcing the app's feature is found" 1 "$GUARD" -p kinige -F test-hooks
check "an unknown feature is an error of the check, not a clean graph" 2 "$GUARD" -p kinige -F no-such-feature

# The control sees dev-dependencies as they are: the graph with them does have the feature (the guard excludes them on purpose).
if cargo tree --locked -e features -p kinige-store 2>/dev/null | grep -qF 'feature "test-hooks"'; then :; else
  echo "FAIL: the graph with dev-dependencies should show the store's test-hooks (the self dev-dependency is gone?)"
  PROBLEMS=$((PROBLEMS + 1))
fi

# A target-specific dependency that turns the feature on, in a throwaway copy of the manifests and sources (the repository itself is never
# touched). `cfg(target_os = "redox")` is false on every host, so only `--target=all` in the guard can see it.
COPY="$WORK/copy"
mkdir -p "$COPY"
(cd "$REPO" && git ls-files -z -- Cargo.toml Cargo.lock rust-toolchain.toml crates server src-tauri scripts | tar --null -T - -cf -) | tar -xf - -C "$COPY"
check "the copy of the repository is clean" 0 bash "$COPY/scripts/check-release-features.sh"
cat >> "$COPY/src-tauri/Cargo.toml" <<'EOF'

[target.'cfg(target_os = "redox")'.dependencies]
kinige-store = { path = "../crates/kinige-store", features = ["test-hooks"] }
EOF
check "the feature turned on for another target is found" 1 bash "$COPY/scripts/check-release-features.sh"
check "the same, asking for the app only" 1 bash "$COPY/scripts/check-release-features.sh" -p kinige

echo "problems: $PROBLEMS"
[ "$PROBLEMS" -eq 0 ]
