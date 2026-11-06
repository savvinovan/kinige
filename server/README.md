# kinige-server: the sync server core

Spike S4c-1 (KINIG-98), spec `docs/slices/S4c1-server-core.md`, result `docs/adr/0019-s4c1-server-core.md`, HTTP shapes in ADR-0001 appendix §A.6. A workspace member, production code, Linux first: a lib and the binary `kinige-server`. The second half, S4c-2 (blobs, backup and restore, `docker compose`), is KINIG-99 and is not here.

What it does: accounts and sessions (ADR-0001 §8.3-§8.7), push and pull of envelopes with `seq` issued under an account lock (§6, §A.3, §A.4), and compaction (§10.2). It reads the open header of an envelope and nothing inside it.

## Layout
- `src/rules/`: the decisions, as plain functions with unit tests and no database. `push.rs` is the check order of §A.4, `pull.rs` the stale rule with drops stored with their generation and the placement of a generation outside the lineage (§6.3), `compact.rs` the plan of §10.2.
- `src/store/`: Postgres only. It reads the facts, calls a rule, writes. `envelopes.rs` is push and pull, `accounts.rs` the credential flows, `compact.rs` applies a compaction plan, `sessions.rs` the tokens.
- `src/api/`: the axum router and the handlers. `src/wire.rs`: the push body and its reader. `src/dto.rs`: the JSON bodies.
- `migrations/0001_init.sql`: the schema, embedded in the binary and applied at start (also by concurrent starts, under an advisory lock).
- `src/testing.rs` (feature `testing`): a throwaway database, an in-process client of the router, synthetic credentials. The S4a simulation uses it.

## Run
```text
openssl rand -hex 32 > server-secret          # 64 hex characters; keep it, an unknown login's salt is derived from it
KINIGE_DATABASE_URL=postgres://user:password@host:5432/kinige \
KINIGE_SERVER_SECRET_FILE=./server-secret \
KINIGE_LISTEN=127.0.0.1:8080 \
cargo run --release -p kinige-server
```
TLS is not terminated here: a reverse proxy does it (S4c-2). Postgres 17 is the tested version.

## Configuration
Everything comes from environment variables.
- `KINIGE_DATABASE_URL` (required): `postgres://user:password@host:port/database`. No TLS to the database, which sits next to the server.
- `KINIGE_SERVER_SECRET_FILE` (required): a file with 64 hex characters (32 bytes), the `server_secret` that keys the salt returned for an unknown login (§8.4).
- `KINIGE_LISTEN` (default `127.0.0.1:8080`): the address to listen on.
- `KINIGE_COMPACT_EVERY_SECS` (default `3600`): how often every account's log is compacted.
- `KINIGE_CLIENT_ADDR_HEADER` (default none): the name of a header that a trusted reverse proxy sets to the client's address, for the rate limits. Without it the connection's address is used.
- `KINIGE_LOG` (default `info`): `error`, `warn`, `info`, `debug` or `trace`, for the server's own records. Other crates are held at `warn`.
- `KINIGE_TEST_DATABASE_URL` (tests only): a Postgres server where databases may be created and dropped. Only the `pg-tests` suites read it.

Fixed limits (`src/config.rs`; real numbers are slice 11's):
- Push body: 16 MiB and at most 1000 envelopes per request; an envelope is at most `MAX_ENVELOPE`.
- Account JSON bodies: 16 KiB.
- Pull page: 200 envelopes by default, at most 1000, and the page stops at 8 MiB (the first envelope always goes).
- Login attempts: 10 a minute per login and 60 a minute per client address.
- Word checks (the recovery fetch and the recovery share one budget): 5 a minute per login and 30 a minute per client address.

The rate limits are fixed windows in memory. Every attempt counts, a known and an unknown login alike, and an excess is a `429` with `Retry-After`. A restart empties them. Behind a reverse proxy without `KINIGE_CLIENT_ADDR_HEADER` every client has the proxy's address, so only the per-login budget separates them.

## Logs
`log` only, to stderr, one line per record: event kinds, ids and error categories. No request body, token, key, login or envelope byte is ever formatted into a record (ADR-0001 §8.8). Dependencies are held at `warn` at every level, because `tokio-postgres` logs every statement with its bound parameters at `debug`; the plain-text scan found it (ADR-0019).

## Tests
```text
cargo test -p kinige-server                          # the pure rules, the wire reader, the config: no database
KINIGE_TEST_DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/postgres \
  cargo test -p kinige-server --features pg-tests    # the HTTP API on a real Postgres, a database per test
cd spikes/S4a-sync && KINIGE_TEST_DATABASE_URL=... cargo test --features real-server --test real_server   # the S4a properties against this server
```
`cargo test --workspace` needs no database. Each `pg-tests` test creates its own database and drops it, also when it panics. Case counts: `GARBAGE_CASES` (300) and `GARBAGE_SEED` (1) for the garbage suite; `S4A_REAL_CASES` (30) and `S4A_REAL_CASES3` (8) for the S4a run.

### What covers each row of §A.3
Files are in `server/tests/`; unit tests are in the module they test.
- `ok(seq)`: `push_pull::ok_assigns_consecutive_seqs_and_a_pull_returns_the_bytes_with_the_declared_names`.
- `duplicate(seq)` and `op_conflict`: `push_pull::duplicate_for_the_same_bytes_and_op_conflict_for_other_bytes` (the nonce is random, so a rebuilt envelope has other bytes).
- `purged` and `stale_purge`: `push_pull::purged_stale_purge_and_a_repeated_purged_with_the_accepted_snapshot` (a newer group, a group missing from the snapshot, a snapshot that covers everything, and a repeated `purged` answered with the accepted snapshot, ADR-0017).
- `clock_skew`: `push_pull::clock_skew_beyond_24_hours_and_nothing_is_stored`, `rules::push::tests::the_skew_limit_is_exclusive_and_covers_a_purged_snapshot`.
- `too_old`: `push_pull::too_old_below_the_minimum_protocol_but_a_duplicate_still_gets_its_answer`.
- `stale` (the new row): `push_pull::stale_when_the_pushers_cursor_is_behind_a_drop_or_in_another_generation`.

### What covers each server step of §A.4
- 1, lengths and the header: `wire::tests::*` (the reader, chunk by chunk), `push_pull::lengths_and_headers_are_judged_before_the_session_and_the_bodies_before_they_are_buffered`, `push_pull::the_batch_has_a_cap_and_every_envelope_of_a_batch_is_answered_in_order`.
- 2, the session: the same lengths test (a 400 before a 401), `accounts::logout_ends_one_session_and_a_login_can_name_the_token_it_replaces`.
- 3, `op_id` before the purge rules: `rules::push::tests::a_repeat_is_a_duplicate_or_an_op_conflict_before_every_other_rule`, and in `push_pull` a duplicate after `too_old` and after a purge.
- 4, 4a, 5, 6, 7: `rules::push::tests::the_checks_come_in_the_order_of_the_appendix`, `rules::push::tests::stale_purge_when_a_known_group_is_newer_or_missing_from_the_snapshot`.
- 8, `seq` under the account lock: `concurrency::parallel_pushers_and_a_puller_never_skip_a_seq`, `concurrency::two_accounts_do_not_block_each_other_and_one_account_does_wait_for_its_lock`.
- Pull and the stale rule: `rules::pull::tests::*`, `push_pull::a_pull_says_stale_or_generation_changed_by_the_rule_of_the_lineage`, `push_pull::a_pull_refuses_what_it_cannot_read_with_a_4xx`.
- Compaction: `rules::compact::tests::*`, `compaction::*`.

### Accounts and sessions (§8.3-§8.7, §A.5)
`accounts.rs` covers registration and its idempotency, what the protocol refuses, an unknown login that looks like a known one (the answer and the log lines), the password change on a credential version (exactly one of six concurrent changes wins), the refusal of a lower profile on a change and on a recovery, `revoke_others`, recovery (the word check, one atomic replacement, every session ended, a repeat is a success, another package is a clash), new words, logout and the login that names the token it replaces, and the rate limits.

### The proofs of the spec
- **Appendix §B item 23 and PRD S4c, "the cursor does not skip records":** `concurrency::parallel_pushers_and_a_puller_never_skip_a_seq` runs 8 pushers (160 envelopes) and a puller with pages of 7 the whole time, 3 rounds; the `seq`s the puller collected equal the log. Negative control: `concurrency::control_seq_from_a_plain_sequence_without_the_lock_does_skip_records` runs the same race with `seq` from a plain Postgres sequence and no account lock, and the reader's cursor passes seqs that commit later. The control is raw SQL in the test; nothing like it is in the server.
- **PRD S4c, "no plain text in the server database":** `leak::nothing_the_user_wrote_and_no_secret_is_in_any_table_or_in_the_log`. Positive controls: `leak::the_scanner_finds_a_deliberate_leak_in_every_encoding`, and in the main test the scan finds the hashes, the salt, the login, a token hash and envelope bytes in the dump, finds a text planted in a table nobody listed, and finds a key appended to the log.
- **Garbage:** `garbage::garbage_on_every_endpoint_is_a_4xx_and_never_a_5xx` sends random bytes, random JSON, the protocol's own field names with wrong values, and a real push body cut, flipped and extended, to every route, with and without a session.

## What is left
S4c-2: blobs and their storage, backup and restore (a new generation and the lineage's dump, the branch-cut rule of ADR-0017), account deletion, `docker compose`, the Dockerfile, TLS at a reverse proxy. Slice 11: the client side, real rate-limit numbers, metrics. See ADR-0019.
