#!/usr/bin/env bash
# The keychain check of slice 5c (KINIG-151, ADR-0021), run by the `keychain` job of ci.yml on a macOS runner: `run.sh <build A> <build B>`.
# Two builds of the same program, signed ad hoc, take turns on one keychain item. A prompt that nobody can answer shows as a timeout
# (status 142, SIGALRM after 20 s); the script never stops on an error, it prints every result, and a human reads the log.
#   1. default access, as `SecItemAdd` gives it: the item trusts the program that made it;
#   2. the item made with `security add-generic-password -A`: any program of this user may read it without a prompt.
set -u
A="$1"
B="$2"

run() {
  perl -e 'alarm 20; exec @ARGV' "$@"
  echo "  exit status $?"
}

sequence() {
  echo "--- $1"
  run "$A" write
  echo "[B reads what A wrote]"
  run "$B" read
  echo "[A reads it back]"
  run "$A" read
  echo "[B writes over it]"
  run "$B" write
  echo "[A reads again]"
  run "$A" read
  echo "[cleanup]"
  run "$B" delete
  run "$A" delete
}

echo "keychains:"
security list-keychains -d user
security default-keychain

sequence "1. default access (creator only), the runner's own keychain"

echo "--- 2. the item made by the security tool with -A (any application), then B reads and A reads"
security add-generic-password -a telegram-bot-token -s me.savvinov.kinige.keychain-check -w "0:fake-secret-for-the-keychain-check" -A
echo "[B reads]"
run "$B" read
echo "[A reads]"
run "$A" read
run "$A" delete
