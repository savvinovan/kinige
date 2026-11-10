---
name: server-tests
description: Running the kinige-server suites against a real Postgres and an S3-compatible store, the S4a simulation against the real server, and the deploy/ compose smoke test, locally through the remote Docker on ubuntu-server or in CI. Use when a change touches server/, deploy/ or spikes/S4a-sync with the real-server feature.
---

# Server tests (S4c-1 and S4c-2, ADR-0019 and ADR-0020)

`server/README.md` has the configuration, the HTTP shapes and the test map. Without the feature and the variable, `cargo test --workspace` runs only the pure rules, and needs no database.

## Postgres

- **Run:** `KINIGE_TEST_DATABASE_URL=postgres://user:password@host:port/postgres cargo test -p kinige-server --features pg-tests`.
- Each test makes and drops its own database.
- **In CI,** the `server` job has a pinned `postgres:17` service.
- **Locally,** the database is a throwaway container on ubuntu-server, reached through the remote Docker (`DOCKER_HOST` is already set). Name it `kinige-s4c-pg-<something>`, give it a random password, use synthetic data only, and remove it afterwards.
  - Ports are reached at `192.168.1.61:<port>`.
  - Bind mounts of Mac paths don't work.

## S3 blobs

- CI's `server` job has a pinned SeaweedFS S3 service for the blob suites.
- The variables are `KINIGE_TEST_S3_URL`, `KINIGE_TEST_S3_BUCKET`, `KINIGE_TEST_S3_ACCESS_KEY` and `KINIGE_TEST_S3_SECRET_KEY`.
- **Locally,** use a throwaway `kinige-s4c2-…` container on ubuntu-server.

## The S4a simulation against the server

- **Run:** `cd spikes/S4a-sync && cargo test --features real-server --test real_server`, with the same `KINIGE_TEST_DATABASE_URL`.

## The compose smoke test

- **In CI,** the `compose` job runs `deploy/smoke.sh`.
- **Locally,** it works against the remote Docker. Its projects, volumes and images are prefixed `kinige-s4c2-smoke` and removed on exit.
- Never run it against a Docker daemon you don't mean to build images on.

## Clean up

When you're done, both of these must show nothing of yours:
- `docker ps -a --filter name=kinige-`
- `docker volume ls --filter name=kinige-`

The guard blocks `docker … prune` and removal of objects not named `kinige-…`: other projects' containers live on that host.
