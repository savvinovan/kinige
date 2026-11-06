# KINIG-98: spike S4c-1, the sync server core

Budget: about 10 h to merge (code 5–6 h, guardian and review 4 h). Stop and report at 20 h.

- **Huly:** KINIG-98, under KINIG-20 (slice 11). The second half, S4c-2, is KINIG-99: blobs, backup and restore, `docker compose`.
- **Branch:** `spike/S4c1-server`. The PR title and the first commit start with `KINIG-98: …`.
- **Release:** none, because nothing changes in the app.
- **Sources:**
  - PRD §10.1, §10.2 and §14. The S4c criteria are: no plain text in the server database; the cursor does not skip records under parallel pushes.
  - ADR-0001: §2 (`SKEW_MAX`), §3.5, §6, §8.3–§8.7, §9, §10, appendix §A.3, §A.4, §A.5 and §B item 23.
  - The "S4c" items of ADR-0017 and ADR-0018.

S4a proved the protocol against an in-memory server, and S4b proved the bytes and the keys. **S4c-1 asks two questions. Can the server side of ADR-0001 be built on axum and Postgres with the same rules? Does it keep them under real concurrency?** Where a rule cannot be built as written, fix it and edit ADR-0001 in place, marked "S4c, <date>", in the same PR.

## Scenario
1. `cargo test -p kinige-server` runs the server's pure rules with no database.
2. `cargo test -p kinige-server --features pg-tests`, with `KINIGE_TEST_DATABASE_URL` set, runs the HTTP API against a real Postgres. It covers:
   - registration, login, password change, recovery, reissuing the words and logout over HTTP;
   - pushes and pulls with every answer in §A.3;
   - many pushers and a puller at once, with no `seq` ever skipped by a cursor;
   - compaction and the stale rules.
3. The S4a simulation, with a new feature `real-server` on top of `real-envelope`, runs its properties at a small fixed count. Each device talks to the real server through its HTTP router, so one test checks the protocol, the bytes and the server together.

## Included
- **`server/`, the crate `kinige-server`.** It is a workspace member, production code, Linux first: a lib plus a bin `kinige-server`.
  - It depends on `kinige-crypto` for `envelope::peek`, `Layout::parse` and the server hashes, and on `kinige-domain` for the HLC and the verdicts it already has.
  - Pure decisions are plain Rust functions with unit tests: the push check order, `stale_purge`, the drop-missed rule with generations and lineage, and the clock skew. SQL only stores and locks.
  - Workspace lints apply: never panic, logging through `log` only. Request bodies, tokens, `auth_key` and envelope bytes never reach the log (§A.4 "Logs").
  - Configuration comes from environment variables: the database URL, the listen address, and `server_secret` read from a file. Document every variable in the README.
  - **Library choices are yours.** Pick a Postgres driver (`sqlx` with runtime-checked queries, or `tokio-postgres` with a pool), migrations embedded in the binary, and Postgres 17. Licences go through `cargo-deny`; any `deny.toml` change carries a reason.
- **Accounts and sessions** (§8.3–§8.7, §A.5), ported from `spikes/S4b-keys/src/server.rs`:
  - The server stores hashes only, compared in constant time, including in its idempotency records. Port that model, never a record with raw keys.
  - An unknown login gets a salt derived from `server_secret` and looks exactly like a known one: same answer shape, same errors, same log lines.
  - Registration and recovery are idempotent (§A.5).
  - Credential versioning: a password change compares and swaps on the version it was derived against, and refuses if another change landed first.
  - **The server refuses a downgrade of an account's profile** on a password change and on a recovery.
  - Recovery ends the other sessions. A password change can optionally do the same.
  - Session tokens are random and the server stores their hashes. Add a logout request, and a login request that carries the token it replaces (ADR-0018 "S4c, sessions").
  - Rate limits on login, recovery and the word checks: in memory, per login and per client address, with simple fixed numbers recorded in the README. Real numbers are slice 11.
- **Push** (§A.3, §A.4, server column, in that order):
  - Lengths and the header are checked with `Layout::parse` and `envelope::peek` before the body is read.
  - The request carries `entity`, `id` and `op` next to each envelope, and the server refuses a mismatch with the header.
  - After that come the session, `op_id` deduplication, `too_old`, `clock_skew`, `purged` and `stale_purge`.
  - **`seq` is assigned under the account lock, in the same transaction as the write.** A transaction that took `seq = n` commits before any other transaction can take `n + 1`, so no reader ever sees a gap that fills in later.
  - The ADR-0017 carry-overs:
    - a repeated `purged` gets the accepted snapshot in its answer;
    - a push whose cursor is behind a drop it missed is refused with a `stale` answer, so the client resyncs first. This is a new row in §A.3; add it.
  - The body limit is enforced before buffering. Batch size has a cap.
- **Pull** (§6.1, §6.3):
  - Pages carry the declared `entity`, `id` and `op` with each envelope (ADR-0018, finding 2), plus `next`, `head`, `epoch` and `generation`.
  - The stale rule uses drops stored with their generation. `GenerationChanged { stale, epoch }` is answered, and a cursor whose generation is not in the lineage gets the placement of §6.3.
  - In S4c-1 the lineage has one generation, because restore is S4c-2. The table, the lineage logic and its unit tests are built now so that S4c-2 only adds the dump and the restore.
  - Store the compaction time with each drop, for the ADR-0017 branch-cut rule that S4c-2 finishes.
- **Compaction** is the rule of `spikes/S4a-sync/src/server.rs::compact`, in SQL:
  - a `purged` older than 180 days goes, together with its entity's envelopes, and adds a drop with a new epoch;
  - an `upsert` older than 180 days goes when every one of its groups is covered by a newer one.
  - The server takes an injected clock, so tests and the S4a adapter run on virtual time. Production uses the system clock.
- **Proofs:**
  - **No plain text in the database.**
    - A run writes synthetic entries whose texts, chat titles, `journal_date` and tags are known, plus every secret: passwords, `auth_key`, `rc_auth_key`, tokens, the words, the DEK, `KEK_pw` and `KEK_rc`.
    - Dump every table, plus the server log, and scan them in every encoding, as `spikes/S4b-keys/src/leak.rs` does.
    - A positive control: the scanner finds a deliberate leak, and the hashes it should find.
  - **The cursor does not skip records** (§B item 23).
    - Several concurrent pushers on one account, and a puller running at the same time. The puller's collected `seq`s equal the log with no gap, every time.
    - The test has a demonstrated negative control: `seq` from a plain sequence without the account lock makes it fail. The control may live in the test code as a raw-SQL path; nothing like it ships in the server.
    - Two accounts pushing in parallel do not block each other: a timing assertion with a generous bound.
  - **Garbage:** random and truncated bodies on every endpoint give a 4xx and never panic or hit a 5xx (rule 12), as a proptest at a fixed count.
- **S4a with the real server:** a feature `real-server` in `spikes/S4a-sync`, which requires `real-envelope`.
  - Put the simulation's server behind a trait. One implementation is the in-memory server; the other calls the `kinige-server` router in process, through `tower::ServiceExt::oneshot`, so HTTP parsing is exercised.
  - Accounts are registered through the API with random 32-byte keys, so no Argon2id runs.
  - Restore mixes are skipped until S4c-2. Say which ones in the README.
  - CI runs it at a small fixed count against a Postgres service. The default build stays the in-memory server.
- **Postgres for tests:**
  - **CI:** a new `server` job on `ubuntu-latest` with a `postgres:17` service, pinned. It runs clippy and the `pg-tests` suite of `kinige-server`, then the S4a `real-server` run.
  - **Locally:** a throwaway container on ubuntu-server through the remote Docker in CLAUDE.md, named `kinige-s4c-pg-<something>`, with a random password and synthetic data only. Remove it when done. Each test creates and drops its own database, so runs never share state.
  - `cargo test --workspace` without the feature needs no database.
- **Docs:**
  - `server/README.md`: run, configuration, the HTTP shapes and the test map below.
  - A new appendix §A.6 of ADR-0001 with the HTTP shapes: paths, bodies, status codes, and how envelopes are encoded on the wire.
  - ADR-0019 "S4c-1 results", half a page: what was proven, what was found, and what is left for S4c-2 and slice 11.
  - ADR-0001 edits in place, marked "S4c".
  - A PRD changelog line.
  - The CLAUDE.md layout and build lines for `server/` and the `pg-tests` command.

## Not included
- Blobs and their storage (disk and S3), backup and restore, a new generation, the branch-cut rule's restore side, account deletion, retention of backups (Q14), `docker compose`, the Dockerfile. All of these are S4c-2.
- TLS: a reverse proxy terminates it, documented in S4c-2.
- Exposing the server to the internet (Q9, second half), deploying to the homelab, the homelab deployer.
- Any client HTTP code in the app, any UI, the OS secret store: slice 11.
- Changes to `kinige-store`, the schema, the app, or `kinige-crypto`'s formats.
- Pagination limits tuned on real data, and any metrics.

## What "done" looks like
- CI is green on all jobs, including the new `server` job: the `pg-tests` suite and S4a with `real-server`.
- Every row of §A.3, every server step of §A.4, and §B item 23 is listed in the README with the test that covers it.
- The plain-text scan passes, with a positive control. The concurrency test passes, with a demonstrated failing control.
- The README, ADR-0019, §A.6, the ADR-0001 edits, the PRD line and the CLAUDE.md lines are in the PR.
- The local Postgres container on ubuntu-server is removed. `docker ps -a --filter name=kinige-` shows nothing of this spike.

## Touches sensitive areas
Yes: the sync protocol, keys and the server's storage of credentials. `sync-crypto-guardian` reviews before merge:
- the push and pull rules against §3.5, §6 and §A.3–§A.5, line by line;
- the credential flows against §8;
- the plain-text and secret scans.

`slice-reviewer` checks the spec, the evidence and the CI. New dependencies are the web framework, the Postgres driver, tokio, and anything they pull in. Each one goes through `cargo-deny`.
