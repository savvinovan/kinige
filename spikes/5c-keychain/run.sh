#!/usr/bin/env bash
# The keychain check of slice 5c (KINIG-151, ADR-0021), run by the `keychain` job of ci.yml on a macOS runner: `run.sh <build A> <build B>`.
# Two builds of the same program, signed ad hoc, take turns on one keychain item. A prompt that nobody can answer shows as a timeout
# (status 142, SIGALRM after 20 s); the script never stops on an error, it prints every result, and a human reads the log.
#   1. default access, as `SecItemAdd` gives it: the item trusts the program that made it;
#   2. the item made with `security add-generic-password -A`: any program of this user, as the old ACL says;
#   3. both builds signed with one self-signed certificate instead of ad hoc: is the designated requirement stable across builds?
set -u

run() {
  perl -e 'alarm 20; exec @ARGV' "$@"
  echo "  exit status $?"
}

# sequence <title> <build A> <build B>
sequence() {
  local a="$2" b="$3"
  echo "--- $1"
  run "$a" write
  echo "[B reads what A wrote]"
  run "$b" read
  echo "[A reads it back]"
  run "$a" read
  echo "[B writes over it]"
  run "$b" write
  echo "[A reads again]"
  run "$a" read
  echo "[cleanup]"
  run "$b" delete
  run "$a" delete
}

A="$1"
B="$2"
echo "keychains:"
security list-keychains -d user
security default-keychain

sequence "1. default access (creator only), ad-hoc signed builds, the runner's own keychain" "$A" "$B"

echo "--- 2. the item made by the security tool with -A (any application), then B reads and A reads"
security add-generic-password -a telegram-bot-token -s me.savvinov.kinige.keychain-check -w "0:fake-secret-for-the-keychain-check" -A
echo "[B reads]"
run "$B" read
echo "[A reads]"
run "$A" read
run "$A" delete

W="$(mktemp -d)"
cat > "$W/openssl.cnf" <<'EOF'
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = Kinige keychain check
[ext]
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
basicConstraints = critical, CA:false
EOF
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$W/key.pem" -out "$W/cert.pem" -days 2 -config "$W/openssl.cnf" 2>/dev/null
openssl pkcs12 -export -out "$W/id.p12" -inkey "$W/key.pem" -in "$W/cert.pem" -passout pass:kc
security create-keychain -p kc "$W/sign.keychain-db"
security set-keychain-settings -t 3600 -u "$W/sign.keychain-db"
security unlock-keychain -p kc "$W/sign.keychain-db"
security import "$W/id.p12" -k "$W/sign.keychain-db" -P kc -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k kc "$W/sign.keychain-db" >/dev/null
HASH="$(openssl x509 -in "$W/cert.pem" -noout -fingerprint -sha1 | sed 's/.*=//; s/://g')"
cp "$A" "$W/a.bin"
cp "$B" "$W/b.bin"
codesign --force --sign "$HASH" --keychain "$W/sign.keychain-db" "$W/a.bin"
codesign --force --sign "$HASH" --keychain "$W/sign.keychain-db" "$W/b.bin"
codesign -dv "$W/a.bin" 2>&1 | grep -E "Signature|Authority|TeamIdentifier|CDHash"
codesign -dv "$W/b.bin" 2>&1 | grep -E "CDHash"
codesign -dr - "$W/a.bin" 2>&1
sequence "3. both builds signed with one self-signed certificate" "$W/a.bin" "$W/b.bin"
