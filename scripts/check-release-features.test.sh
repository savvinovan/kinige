#!/usr/bin/env bash
# Controls of scripts/check-release-features.sh: it must pass on the real release graph and fail when the feature is forced on (a
# guard that never fired proves nothing). Run: bash scripts/check-release-features.test.sh (in CI, the rust job).
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
GUARD="$REPO/scripts/check-release-features.sh"

PROBLEMS=0
check() {
  local name="$1" want="$2"
  shift 2
  if "$@" > /dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name (wanted $want, got $got)"
    PROBLEMS=$((PROBLEMS + 1))
  fi
}

check "the release graph of the whole workspace is clean" pass "$GUARD"
check "the release graph of the app is clean" pass "$GUARD" -p kinige
check "forcing the store's feature through the app fails" fail "$GUARD" -p kinige -F kinige-store/test-hooks
check "forcing the app's feature fails" fail "$GUARD" -p kinige -F test-hooks
check "forcing the store's feature in the whole workspace fails" fail "$GUARD" --workspace -F kinige-store/test-hooks
check "forcing the app's feature in the whole workspace fails" fail "$GUARD" --workspace -F kinige/test-hooks
# The control sees dev-dependencies as they are: the graph with them does have the feature (the guard excludes them on purpose).
if cargo tree --locked -e features -p kinige-store 2>/dev/null | grep -qF 'feature "test-hooks"'; then :; else
  echo "FAIL: the graph with dev-dependencies should show the store's test-hooks (the self dev-dependency is gone?)"
  PROBLEMS=$((PROBLEMS + 1))
fi
check "an unknown feature fails (not an empty, clean graph)" fail "$GUARD" -p kinige -F no-such-feature

echo "problems: $PROBLEMS"
[ "$PROBLEMS" -eq 0 ]
