#!/usr/bin/env bash
# The compose smoke test of spike S4c-2 (KINIG-99, ADR-0020). It brings up the stack of deploy/docker-compose.yml, uses it through the
# reverse proxy with TLS, takes a backup, restores that backup into a SECOND, fresh stack (a new database, an empty blob volume), and checks
# that the second stack answers the old cursor with GenerationChanged and serves every envelope and the blob byte for byte.
#
# Run from anywhere:   deploy/smoke.sh
# Needs docker with compose v2, openssl, curl and cargo (the client is `cargo build --example smoke -p kinige-server --features testing`).
# With a remote Docker daemon (DOCKER_HOST=tcp://host:2376) the stacks' ports are reached on that host; override with SMOKE_HOST.
#
# Everything it creates has the prefix SMOKE_PREFIX (default kinige-s4c2-smoke): two compose projects, one shared volume for the backups,
# one image. All of it is removed on exit, also on failure. Passwords and the server secret are random and used once.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
prefix="${SMOKE_PREFIX:-kinige-s4c2-smoke}"
project_a="${prefix}-a"
project_b="${prefix}-b"
backups_volume="${prefix}-backups"
port_a="${SMOKE_PORT_A:-39443}"
port_b="${SMOKE_PORT_B:-39444}"
image_tag="smoke-${prefix}"
work="$(mktemp -d)"

if [ -n "${SMOKE_HOST:-}" ]; then
  host="$SMOKE_HOST"
elif [ -n "${DOCKER_HOST:-}" ] && [[ "$DOCKER_HOST" == tcp://* ]]; then
  host="${DOCKER_HOST#tcp://}"
  host="${host%%:*}"
else
  host="127.0.0.1"
fi

dc() { # dc <project> <compose arguments...>
  local project="$1"
  shift
  docker compose -p "$project" --env-file "$work/$project.env" -f "$here/docker-compose.yml" "$@"
}

cleanup() {
  status=$?
  set +e
  if [ "$status" -ne 0 ]; then
    echo "--- the smoke test failed: logs of the first stack's server and proxy" >&2
    dc "$project_a" logs --no-color --tail 40 server proxy >&2 2>/dev/null
    echo "--- and of the second" >&2
    dc "$project_b" logs --no-color --tail 40 server proxy >&2 2>/dev/null
  fi
  dc "$project_a" --profile tools down -v --remove-orphans >/dev/null 2>&1
  dc "$project_b" --profile tools down -v --remove-orphans >/dev/null 2>&1
  docker volume rm "$backups_volume" >/dev/null 2>&1
  docker image rm "kinige-server:${image_tag}" "kinige-proxy:${image_tag}" >/dev/null 2>&1
  rm -rf "$work"
  exit "$status"
}
trap cleanup EXIT

step() { echo "== $*"; }

write_env() { # write_env <project> <https port> <http port>
  cat >"$work/$1.env" <<EOF
POSTGRES_PASSWORD=$(openssl rand -hex 24)
KINIGE_SERVER_SECRET=$(openssl rand -hex 32)
KINIGE_DOMAIN=localhost
PROXY_HTTPS_PORT=$2
PROXY_HTTP_PORT=$3
BACKUPS_VOLUME=${backups_volume}
KINIGE_IMAGE_TAG=${image_tag}
COMPOSE_PROJECT_NAME=$1
EOF
}

write_env "$project_a" "$port_a" "$((port_a + 1000))"
write_env "$project_b" "$port_b" "$((port_b + 1000))"

step "building the smoke client"
(cd "$repo" && cargo build --release --example smoke -p kinige-server --features testing)
target="${CARGO_TARGET_DIR:-$repo/target}"
smoke="$target/release/examples/smoke"
[ -x "$smoke" ] || { echo "the smoke client is not at $smoke" >&2; exit 1; }
base_a="https://localhost:${port_a}"
base_b="https://localhost:${port_b}"

step "stack A: up (server, Postgres, blob volume, proxy with TLS)"
dc "$project_a" up -d --build --wait
curl -fsS -k --resolve "localhost:${port_a}:${host}" "${base_a}/healthz" | grep -qx ok
echo "ok: /healthz through the proxy"
test "$(dc "$project_a" exec -T server id -u)" = "10001"
echo "ok: the server runs as a non-root user"

step "stack A: register, push, upload a blob in two parts, pull"
"$smoke" prepare "$work/state.json" "$base_a" "$host"
"$smoke" oversize "$base_a" "$host"

step "stack A: backup (a snapshot of Postgres, the blobs, a manifest)"
dc "$project_a" --profile tools run --rm -T backup
latest="$(dc "$project_a" --profile tools run --rm -T --entrypoint ls backup /backups | grep '^backup-' | sort | tail -n 1)"
test -n "$latest"
echo "ok: backup $latest"
dc "$project_a" --profile tools run --rm -T --entrypoint ls backup "/backups/$latest" | grep -qx manifest.json

step "stack A: SIGTERM stops the server cleanly and quickly"
started="$(date +%s)"
dc "$project_a" stop server
elapsed="$(($(date +%s) - started))"
code="$(docker inspect -f '{{.State.ExitCode}}' "$(dc "$project_a" ps -aq server)")"
test "$code" = "0" || { echo "the server exited with $code after SIGTERM" >&2; exit 1; }
test "$elapsed" -lt 10 || { echo "the server took ${elapsed}s to stop" >&2; exit 1; }
echo "ok: exit code 0 in ${elapsed}s"

step "stack B: a fresh database and an empty blob volume, restored from the backup of A"
dc "$project_b" up -d --wait db
KINIGE_RESTORE_FROM="/backups/$latest" dc "$project_b" --profile tools run --rm -T restore
dc "$project_b" up -d --build --wait
curl -fsS -k --resolve "localhost:${port_b}:${host}" "${base_b}/healthz" | grep -qx ok

step "stack B: the old cursor, the envelopes and the blob"
"$smoke" verify "$work/state.json" "$base_b" "$host"

echo "== the smoke test passed"
