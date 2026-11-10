# KINIG-103: tech debt E1, the Rust half

**Budget:** about 4 h to merge. Stop and report at 8 h.

**Task:** Huly KINIG-103, under KINIG-49. Branch `chore/E1-rust-debt`. The PR title and the first commit start with `KINIG-103: …`. No release: nothing changes for the user.

Slice 11 is about to add synced tables and a client to this code. Before that, four things need to stop being a trap. **The rule for all of them: the app behaves exactly as before.** A test that has to change its expected result is a finding to report, not an edit to make.

## Included

### 1. Migration tests start from a real old database
- Many tests build an "old" database by taking the current schema and running `DROP TABLE`. That proves nothing about what an old database really looks like.
- Add `db_at_version(n)`, which applies the first `n` real entries of `MIGRATIONS` with `after_migration`, exactly as an upgrade does, and leaves `user_version = n`.
- Classify every `DROP TABLE` in `crates/kinige-store/tests/` and `src-tauri/tests/`.
  - A test that imitates an older schema moves to `db_at_version(n)`.
  - A test that drops a table to force a store error (fault injection) stays as it is.
  - List the classification in the PR body.
- **Pin the old migrations.** A test holds the SHA-256 of each entry of `MIGRATIONS`. Editing a shipped migration fails it, and appending a new one only adds a line.
  - If git history shows that a shipped migration was already edited after its release, stop and report: real databases may differ from the frozen text.
- Add a test for the whole upgrade path: every `n` from 1 to `LATEST_VERSION` upgrades to a schema equal to a fresh database's (`sqlite_master`, compared as data).

### 2. One search builder
- Search is written twice: `crates/kinige-store/src/search_query.rs` and `agent_read.rs` (`search_fts MATCH`). Healing the index and the read wrapping around it are repeated too.
- Build one query builder and one healing and read wrapper, used by both. The UI and MCP keep their own result shapes and filters.
- `agent_read.rs` goes below 400 lines.
- The existing tests of both paths stay as they are and pass.

### 3. Calendar arithmetic through `chrono` (code style rule 6)
- Replace the hand-written `civil_from_days`, the leap-year check in `crates/kinige-domain/src/journal.rs`, and the time-of-day arithmetic in `crates/kinige-store/src/time.rs` (`hhmmss`, `local_hhmm`).
- **`journal_date` is stored data and goes into the sync payload, so its output must be the same bytes.**
  - Keep the old algorithm in a test module as an oracle.
  - A property test compares old and new on random timestamps, offsets and boundaries, plus the edges: negative times, the DST days, year 1000 and 9999, month and leap-day edges.
- Outside the range `chrono` can represent (the old code printed a date for any `i64`), it must not panic.
  - Find out whether such a value can reach this function at all, and write down what it returns. Keep the old output there if that is the simplest honest answer.
- `kinige-domain` keeps building for `wasm32-unknown-unknown`.

### 4. Test hooks behind a feature
- `#[doc(hidden)] pub` hooks in `kinige-store` and `src-tauri` (injected panics and faults, among others) ship in the release build today.
- Put them behind a cargo feature `test-hooks`, off by default, enabled only through dev-dependencies. `kinige-domain`, `kinige-store`, `kinige-mcp` and `kinige` keep compiling without it.
- A CI check, in the style of `scripts/check-test-hooks.sh`, proves that the release dependency graph does not enable the feature (`cargo tree -e features --no-dev` or better).
- If a `#[doc(hidden)]` item is real API used across crates in production, leave it and say so in the PR.

### 5. One line in ADR-0001 (guardian note on PR #14)
In appendix §A.4, client step 5, add if it is not said yet: the payload is parsed group by group (`RawValue`, as `parse_groups_raw` does), and the nesting limit of 127 counts from the value of a group, not from the whole payload. Otherwise a slice 11 reader that parses the whole payload as one `Value` would quarantine an honest group of depth 127.

## Not included
- Any change of schema, of a migration's SQL, or of what a user sees or what the log says.
- `MockApi` and the shared fixtures: that is E2 (KINIG-104), in parallel. Do not touch `web/`.
- The Dependabot PRs, including the `rusqlite` bump.
- The items of KINIG-102 (server follow-ups).
- Rewriting test files beyond what the items above need.

## What "done" looks like
- CI is green on all jobs, with no change to expected results in existing tests other than the `db_at_version` moves, each explained in the PR body.
- The old-versus-new `journal_date` property test passes at a fixed seed, and its negative control is shown once (a deliberately off-by-one boundary fails).
- The release graph has no `test-hooks`, and the check fails when the feature is forced on.
- `agent_read.rs` is under 400 lines.

## Touches sensitive areas
Yes: migration tests, and `journal_date`, which is stored and synced. `sync-crypto-guardian` reviews before merge. `slice-reviewer` checks the spec, the evidence and the CI.
