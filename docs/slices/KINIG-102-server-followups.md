# KINIG-102: fix KINIG-102, server follow-ups from the reviews of PR #51 and PR #52

**Budget:** code 5 h, review and verification 10 h (verification has been about 3× the code; most of this is moves, which verify fast). Stop and report at 2×.
**Task:** Huly KINIG-102, under KINIG-20 (slice 11). **Branch:** `fix/KINIG-102-server-followups`. The PR title and first commit start with `KINIG-102: …`. No release: nothing changes in the app.

One PR, one commit per item below, in this order, so that the guardian reads commits 1 and 2 and 5 and the reviewer the rest. Moves come before the fix, so the fix's diff is only the fix.

## Scenario
No user sees this. An operator and a developer do.

**The restore lock (ADR-0020, "What is left"; CLAUDE.md rule 12).**
1. The operator stops the server and runs `kinige-server restore <backup directory>` in place.
2. Mid-restore, the restore's lock connection ends: Postgres restarts, the network drops, or an administrator runs `pg_terminate_backend`. The lock dies with it, and a server starts in the gap. The compose stack's `restart: unless-stopped` is enough to start one.
3. **Today:** the server takes its lock and serves, and the restore goes on and replaces every table under it, on other pool connections. **After the fix:** the restore stops with an error saying it lost its lock and changed nothing in the database. The objects it put into the store are removed again, and the running server keeps serving the installation as it was.
4. The operator stops the server and runs the restore again. It succeeds.

**The rest.** A developer opens `store/lock.rs` and `backup/journal.rs` and finds constants first, then types, then constructors and methods (code style rule 2). `cargo clippy` in `spikes/S4a-sync` is clean, and CI keeps it clean. Every file under `server/tests/` is under 400 lines. ADR-0020 reads as decisions, not as a review log.

## Included
1. **Order within a file, moves only.**
   - `store/lock.rs`: `SERVER_LOCK` sits mid-file, next to the functions. The types (`ServerLock`, `Retake`, `Held`) sit below free functions.
   - `backup/journal.rs`: the type `Applied` comes after the `impl Journals`, and `extend` comes before the constructors `of_backup`, `of_installation` and `of_newest_neighbour`.
   - No code changes, and no test changes.
2. **The restore holds its lock on the connection that writes.**
   - `backup::restore` takes one pool connection and its `pg_try_advisory_lock(SERVER_LOCK)` on it, as today. The load transaction then runs on **that same connection**.
   - If the connection ends, the lock ends and the transaction rolls back together. "Lock held" and "TRUNCATE committed" share one fate.
   - The error is a new `BackupError` variant. Its message: the restore lost its lock, changed nothing in the database, and the operator should check that no server runs and try again. The existing cleanup of copied objects runs for it.
   - Regression test in `server/tests/server_lock.rs`:
     - The test holds `BACKUP_LOCK` exclusively on its own connection, so the restore waits at the first statement of its transaction (after the blob copy, before the `TRUNCATE`). This needs no new hook.
     - The test seeds an installation that differs from the backup, including one backed-up blob that the store lacks. It starts the restore and waits until the restore's backend is waiting on `BACKUP_LOCK` (`pg_locks`, not granted).
     - It terminates the backend that holds `SERVER_LOCK`, takes a `ServerLock` (the server starts in the gap), and releases `BACKUP_LOCK`.
     - Expected: the restore returns the new error; the tables equal the installation from before; the object the restore copied is gone; and `ServerLock::lost` does not resolve.
     - Then the test drops the `ServerLock` and runs the restore again. It succeeds.
   - Update ADR-0020's "What is left" line about this. Also say what remains open, if the guardian accepts a residue (see Decisions).
3. **Test files.**
   - Move the credential tests out of `server/tests/restore_rules.rs` (686 lines) into the existing `restore_credentials.rs`: `a_password_changed_after_a_backup_is_not_the_old_one_again` and `new_recovery_words_after_a_backup_are_not_the_old_words_again`.
   - Move the three branch-cut and lineage tests into a new `restore_branch_cut.rs`: `a_branch_cut_from_…`, `a_branch_that_was_itself_restored_…` and `the_lineage_is_capped_…`.
   - `restore_rules.rs` keeps the deletion tests and `a_restore_refuses_to_run_under_a_server_…`.
   - Merge the two copies of `copy_dir` (`restore_rules.rs`, `restore_garbage.rs`) into one in `tests/common/backup.rs`.
   - Test names and bodies don't change.
   - Update every pointer to these tests: the test map in `server/README.md`, the `//!` headers of `backup_restore.rs`, `garbage_blobs.rs` (it already points to the wrong file) and `restore_credentials.rs`.
4. **The S4a spike's clippy lints.**
   - Fix them without a behaviour change. The properties, negative controls and scenarios pass at CI's seed with no edit to an expected result.
   - An `#[allow]` only with a reason.
   - Add CI steps so the lints don't come back:
     - `cargo clippy --all-targets -- -D warnings` in the `s4a` job (default features);
     - `cargo clippy --features real-server --all-targets -- -D warnings` in the `server` job, which already builds `real-server`.
   - The PR lists the count before and after.
5. **Trim ADR-0020** from about 3,150 words to at most about 1,300 (the size of ADR-0019).
   - Keep every decision, every known limit and every "not verified" item.
   - Fold the findings of review rounds 1 and 2 into the rule each one produced, without the round story.
   - Test names stay in `server/README.md`, which already holds them.
   - The status line says it was trimmed under KINIG-102, and names the commit with the full text.
   - Links into it from ADR-0001, the PRD and `server/README.md` still hold.

## Not included
- Everything ADR-0019 and ADR-0020 give to slice 11:
  - the pool wait timeout and the 503;
  - per-account budgets and rate limits for registration, push and pull;
  - session expiry;
  - a restore of schema N into N+1 through the migration list;
  - re-uploading blobs after `GenerationChanged`, and the client's handling of `401` after a restore;
  - metrics, and exposing the server to the internet.
- The server side of the lock gap: a running server whose own lock connection is down for longer than a restore takes. The supervisor of ADR-0020 round 2 already covers it.
- Other known limits: one server process, `ops` and `drops` never trimmed, compaction memory, the blob of a never-pushed `purged`.
- Splitting or reordering any other file. Changes to schema, wire, envelope or log text, other than the new error message.
- Tech debt E1 and E2 (KINIG-103, KINIG-104).

## Reference (UI work)
None: no UI.

## Decisions left to the implementer
- **Session lock on the transaction's connection, not a check before the `TRUNCATE`** (sync, server storage: the guardian rules on it before code). Recommended, because a check is one more statement on another connection, and that connection can end between the check and the commit. The task names both options; this is the one without a race.
- **The orphan scan after the commit** (sync, server storage: guardian). `adopt_orphans` runs with an age of 0 on the assumption that nothing publishes meanwhile, and it uses the pool. If the lock connection ends after the commit, a server that starts could publish a blob that the scan then adopts as deleted. Recommended: run its reads and writes on the lock connection (`&impl GenericClient` in place of the pool; the sweep passes a pooled client). A lost lock then stops it at its next statement, and the existing `orphan_scan_failed` path reports it. Without a pause hook there is no deterministic test for this, so the evidence is the code path plus a test that `adopt_orphans` on a terminated connection writes nothing. The guardian decides whether it is in this PR.
- **The residue in the blob copy** (server storage: guardian). If the connection ends during the copy that runs before the transaction, the restore has already put objects into a store that a fresh server may serve, and its cleanup removes them. They are only keys whose object was absent or did not match the manifest's hash. Recommended: accept it and name it in ADR-0020's known limits. Closing it would mean holding the transaction open across the whole copy.
- **How the new error is detected**: the client reports it closed (for example `Client::is_closed` of `tokio-postgres`; not verified for the version in `Cargo.lock`), or the failing statement. The implementer picks one; the test pins the variant.

## What "done" looks like
No window. The new error's text comes from `kinige-server restore`. `cargo test -p kinige-server --features pg-tests` has one more test, with the same names as before in the other files. The S4a spike is clippy-clean in CI. ADR-0020 fits on about two screens.

## Checks
| Scenario step / rule                  | Test or evidence that proves it                                     |
|---------------------------------------|---------------------------------------------------------------------|
| Step 3: the lost lock changes nothing | the new `server_lock.rs` test of item 1 passes                      |
| Negative control                      | the same test fails on `main`'s restore: shown once in the PR       |
| Step 4: the second restore succeeds   | the same test: after the server stops, the restore succeeds         |
| Restores without a loss are unchanged | every `pg-tests` suite and S4a `real_server` pass with no edit      |
| Order within a file                   | `git diff --color-moved`: moves only; tests pass with no edit       |
| Spike lints                           | `cargo clippy --all-targets` (both feature sets): 0, CI step green  |
| Test split                            | `-- --list` gives the same test names before and after; files < 400 |
| One `copy_dir`                        | one definition, in `tests/common/backup.rs`                         |
| ADR-0020 trimmed                      | word count before and after; guardian: nothing decided is lost      |
| The branch                            | `scripts/pre-pr-check.py` passes; CI green on every job             |

Data surviving a crash: the restore is one transaction on the lock's connection, and the test kills that connection mid-restore. No screenshots: no UI.

## Touches sensitive areas
Yes.
- **What the server stores and its concurrency:** the restore against a server starting in the gap, the cleanup of store objects, and, if the guardian takes it, the orphan scan.
- **ADR-0020:** the record of the backup and restore decisions.

No change to the server schema, the wire format, the envelope or the app's SQLite. `sync-crypto-guardian` reviews this spec before code and the PR before merge: commits 2 and 5, and commit 1 to confirm it is moves only.

## Open questions
None.
