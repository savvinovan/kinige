# KINIG-102: fix KINIG-102, server follow-ups from the reviews of PR #51 and PR #52

**Budget:** code 7 h, review and verification 10 h (verification has been about 3× the code; most of this is moves, which verify fast). Stop and report at 2×. The code estimate grew by 2 h after the guardian's rulings on this spec (a probe statement and two failure variants, the orphan scan, three more tests).
**Task:** Huly KINIG-102, under KINIG-20 (slice 11). **Branch:** `fix/KINIG-102-server-followups`. The PR title and first commit start with `KINIG-102: …`. No release: nothing changes in the app.

One PR, one commit per item below. The order of the commits: the spec, then the moves (items 1 and 3), the spike's lints (4), the restore lock (2), the orphan scan (6) and the trim of ADR-0020 (5). The guardian reads the commits of items 2, 6 and 5, and the reviewer the rest. Moves come before the fix, so the fix's diff is only the fix.

## Scenario
No user sees this. An operator and a developer do.

**The restore lock (ADR-0020, "What is left"; CLAUDE.md rule 12).**
1. The operator stops the server and runs `kinige-server restore <backup directory>` in place.
2. Mid-restore, the restore's lock connection ends: Postgres restarts, the network drops, or an administrator runs `pg_terminate_backend`. The lock dies with it, and a server starts in the gap. The compose stack's `restart: unless-stopped` is enough to start one.
3. **Today:** the server takes its lock and serves, and the restore goes on and replaces every table under it, on other pool connections. **After the fix:** the restore stops with an error saying it lost its lock and replaced nothing in the database. The objects it put into the store are removed again, and the running server keeps serving the installation as it was.
4. The operator stops the server and runs the restore again. It succeeds.

**The rest.** A developer opens `store/lock.rs` and `backup/journal.rs` and finds constants first, then types, then constructors and methods (code style rule 2). `cargo clippy` in `spikes/S4a-sync` is clean, and CI keeps it clean. `server/tests/restore_rules.rs` is under 400 lines. ADR-0020 reads as decisions, not as a review log.

## Included
1. **Order within a file, moves only.**
   - `store/lock.rs`: `SERVER_LOCK` sits mid-file, next to the functions. The types (`ServerLock`, `Retake`, `Held`) sit below free functions.
   - `backup/journal.rs`: the type `Applied` sits below the `impl Journals` and `extend` sits before the constructors `of_backup`, `of_installation` and `of_newest_neighbour`. Rule 2 wants the type above the `impl` and the method after the constructors.
   - No code changes, and no test changes.
2. **The restore holds its lock on the connection that writes.** The guardian confirmed the design (ruling 1) and ran prototypes against Postgres 17.
   - `backup::restore` takes one pool connection and its `pg_try_advisory_lock(SERVER_LOCK)` on it, as today. The load transaction then runs on **that same connection**. The migration (`store::migrate`) stays on the pool: it is idempotent and has its own lock.
   - If the connection ends, the lock ends and the transaction rolls back together. "Lock held" and "TRUNCATE committed" share one fate.
   - **Detection is a probe statement, not `Client::is_closed()`.** After `pg_terminate_backend`, `is_closed()` was still false in 38 of 40 runs, while SQLSTATE `57P01` came back in 40 of 40. "Lost" means that a trivial statement (`SELECT 1`) on the lock connection fails.
   - **The outcomes:**

     | Failure                   | Lock connection | Result                        | Copied objects |
     |---------------------------|-----------------|-------------------------------|----------------|
     | before `COMMIT`           | usable          | today's error                 | removed        |
     | before `COMMIT`           | lost            | new "lost its lock" variant   | removed        |
     | at `COMMIT`               | usable          | today's error (rolled back)   | removed        |
     | at `COMMIT`               | lost            | new "outcome unknown" variant | kept           |
     | after `COMMIT`, in a scan | lost            | `Ok`, `orphan_scan_failed`    | not applicable |

   - Kept objects are safe: a rerun finds them by hash, and the sweep adopts the unreferenced ones.
   - The messages of the two new `BackupError` variants say "replaced nothing", not "changed nothing": `migrate` may have created the schema. The first tells the operator to check that no server runs and to try again.
   - Tests in `server/tests/server_lock.rs`. Each seeds an installation that differs from the backup, including one backed-up blob that the store lacks.
     - **Lost after the `TRUNCATE`.** The test installs, in its own database, a trigger `BEFORE INSERT ON lineage` that calls `pg_advisory_xact_lock(<test key>)`, and holds that key. The restore then waits after the `TRUNCATE`; a pause at `BACKUP_LOCK` would also pass on the rejected design that checks the lock before the `TRUNCATE` (the guardian's variant B). The test waits until the restore's backend waits on the key (`pg_locks`, not granted) and checks that it is the backend that holds `SERVER_LOCK`. It terminates that backend, takes a `ServerLock` (the server starts in the gap), and releases the key. Expected: the new "lost its lock" error; the tables equal the installation from before; the object the restore copied is gone; and a session from before the restore still gets its pre-restore head over HTTP. Then the test drops the `ServerLock` and runs the restore again with the retry loop of `a_restore_refuses_to_run_under_a_server_…`, because a dropped `ServerLock` is released asynchronously. It succeeds.
     - **At `COMMIT`, connection usable.** A deferred constraint trigger raises an error. Expected: today's error, the tables unchanged, the copied object removed.
     - **The backend dies during `COMMIT`.** A deferred constraint trigger terminates its own backend. Expected: the "outcome unknown" variant, and the copied object kept.
   - Update `server/README.md` (the restore paragraph and the test map) and the header of `restore.rs`. Update the "What is left" line of ADR-0020.
3. **Test files.**
   - Move the credential tests out of `server/tests/restore_rules.rs` (686 lines) into the existing `restore_credentials.rs`: `a_password_changed_after_a_backup_is_not_the_old_one_again` and `new_recovery_words_after_a_backup_are_not_the_old_words_again`.
   - Move the three branch-cut and lineage tests into a new `restore_branch_cut.rs`: `a_branch_cut_from_…`, `a_branch_that_was_itself_restored_…` and `the_lineage_is_capped_…`.
   - `restore_rules.rs` keeps the deletion tests and `a_restore_refuses_to_run_under_a_server_…`.
   - Merge the two copies of `copy_dir` (`restore_rules.rs`, `restore_garbage.rs`) into one in `tests/common/backup.rs`.
   - Test names and bodies don't change.
   - Update every pointer to these tests: the test map in `server/README.md`, the `//!` headers of `backup_restore.rs`, `garbage_blobs.rs` (it already points to the wrong file) and `restore_credentials.rs`.
   - Other files over 400 lines under `server/tests/` (`accounts.rs`, `blobs.rs`, `restore_garbage.rs`, `backup_restore.rs`, `leak.rs`) were over it before and are not in scope.
4. **The S4a spike's clippy lints.**
   - Fix them without a behaviour change. The properties, negative controls and scenarios pass at CI's seed with no edit to an expected result.
   - An `#[allow]` only with a reason.
   - Add CI steps so the lints don't come back:
     - `cargo clippy --all-targets -- -D warnings` in the `s4a` job (default features);
     - `cargo clippy --features real-envelope --all-targets -- -D warnings` in the same job, which already builds `real-envelope`;
     - `cargo clippy --features real-server --all-targets -- -D warnings` in the `server` job, which already builds `real-server`.
   - The PR lists the count before and after.
5. **Trim ADR-0020** from about 3,150 words to at most about 1,300 (the size of ADR-0019). If the word target and a decision conflict, the decision wins.
   - Keep every decision, every known limit and every "not verified" item.
   - Fold the findings of review rounds 1 and 2 into the rule each one produced, without the round story. Keep a one-line pointer to the review history in the PR and the commit that holds the full text: 27 code comments cite "round N".
   - The PR maps each of the 10 findings, the 9 round-1 items, the 6 round-2 items, the known limits and the "not verified" lines to the sentence that now carries it.
   - Test names stay in `server/README.md`, which already holds them.
   - The status line says it was trimmed under KINIG-102, and names the commit with the full text.
   - Links into it from ADR-0001, the PRD and `server/README.md` still hold.
   - **The copy residue (accepted; the guardian's ruling 9) is stated precisely in the known limits:**
     - until its first database statement after the copy, the restore puts objects into the store and then removes them;
     - it touches only keys whose object was absent or did not match the manifest's hash;
     - it can un-repair a blob whose object was already missing or damaged;
     - it can delete the object of a blob published in that window under a key that is also in the backup, which needs a reused blob id, or the same account id registered again on a clean installation.
   - **The server-side gap** stays as a known limit: the supervisor covers a restore that still holds the lock when the server reconnects; a gap longer than the whole restore is not covered.
6. **The orphan scan runs on the lock connection** (guardian's ruling 5), as its own commit after the lock fix.
   - `adopt_orphans`, `blobs::all` and `blobs::adopt_orphan` take `&impl GenericClient`. The restore passes the lock connection, the sweep passes a pooled client. A lost lock then stops the scan at its next statement, and the existing `orphan_scan_failed` path reports it.
   - Test: a trigger on `blobs` (`WHEN (NEW.published_ms = 0 AND NEW.deleted_ms IS NOT NULL)`) pauses the scan. The test terminates the lock's backend, takes a `ServerLock`, and releases the key. Expected: `Ok`, `orphan_scan_failed`, and no adopted row. Control: the pool-based scan adopts it (`orphans: 1`).

## Not included
- Everything ADR-0019 and ADR-0020 give to slice 11:
  - the pool wait timeout and the 503;
  - per-account budgets and rate limits for registration, push and pull;
  - session expiry;
  - a restore of schema N into N+1 through the migration list;
  - re-uploading blobs after `GenerationChanged`, and the client's handling of `401` after a restore;
  - metrics, and exposing the server to the internet.
- The server side of the lock gap, beyond what the supervisor of ADR-0020 round 2 covers: it covers a restore that still holds the lock when the server reconnects. A gap longer than the whole restore is a known limit in ADR-0020.
- Other known limits: one server process, `ops` and `drops` never trimmed, compaction memory, the blob of a never-pushed `purged`.
- Closing the copy residue (holding the transaction open across the whole copy).
- Splitting or reordering any other file. Changes to schema, wire, envelope or log text, other than the new error messages.
- Tech debt E1 and E2 (KINIG-103, KINIG-104).

## Reference (UI work)
None: no UI.

## Decisions left to the implementer
Ruled by `sync-crypto-guardian` before code:
- **Session lock on the transaction's connection, not a check before the `TRUNCATE`.** Confirmed. A check is one more statement on another connection, and that connection can end between the check and the commit.
- **Detection by a probe statement** on the lock connection, not `Client::is_closed()` (see item 2).
- **The orphan scan on the lock connection, in this PR** (item 6).
- **The residue in the blob copy: accepted** and named in ADR-0020's known limits (item 5).

Left to the implementer, and written under "Decisions for the guardian" in the PR: the names of the two error variants, and anything else beyond the rulings.

## What "done" looks like
No window. The new error's text comes from `kinige-server restore`. `cargo test -p kinige-server --features pg-tests` has four more tests than before (three for the lock, one for the scan), with the same names as before in the other files. The S4a spike is clippy-clean in CI. ADR-0020 fits on about two screens.

## Checks
| Scenario step / rule                   | Test or evidence that proves it                                                          |
|----------------------------------------|------------------------------------------------------------------------------------------|
| Step 3: the lost lock replaces nothing | the `server_lock.rs` test of item 2 passes (pause after the TRUNCATE)                    |
| Negative control                       | the same test fails on `main`'s restore, tables replaced: shown once in the PR           |
| Rejected design                        | the same test fails on the guardian's variant B (check before the TRUNCATE)              |
| Step 4: the second restore succeeds    | the same test: after the server stops, the restore succeeds                              |
| Outcomes at `COMMIT`                   | the two other `server_lock.rs` tests of item 2 (usable; backend dies)                    |
| Orphan scan on the lock connection     | the `server_lock.rs` test of item 6; control: the pool-based scan adopts                 |
| Restores without a loss are unchanged  | every `pg-tests` suite and S4a `real_server` pass with no edit                           |
| Order within a file                    | `git diff --color-moved`: moves only; tests pass with no edit                            |
| Spike lints                            | `cargo clippy --all-targets` (three feature sets): 0, CI steps green                     |
| Test split                             | `-- --list` gives the same test names before and after; `restore_rules.rs` < 400         |
| One `copy_dir`                         | one definition, in `tests/common/backup.rs`                                              |
| ADR-0020 trimmed                       | word count before and after; the PR's map of findings; guardian: nothing decided is lost |
| The branch                             | `scripts/pre-pr-check.py` passes; CI green on every job                                  |

Data surviving a crash: the restore is one transaction on the lock's connection, and the tests kill that connection mid-restore and during the commit. No screenshots: no UI.

## Touches sensitive areas
Yes.
- **What the server stores and its concurrency:** the restore against a server starting in the gap, the cleanup of store objects, and the orphan scan.
- **ADR-0020:** the record of the backup and restore decisions.

No change to the server schema, the wire format, the envelope or the app's SQLite. `sync-crypto-guardian` reviews this spec before code and the PR before merge: commits for items 2, 6 and 5, and the commit for item 1 to confirm it is moves only.

## Open questions
None.
