# Spike S4a: simulation of the sync protocol

Date: 2026-11-05. Task: KINIG-94, spec `docs/slices/S4a-sync-simulation.md`. Result: ADR-0017, ADR-0001 revision 8, PRD v0.22.

The question: does the protocol of ADR-0001, as written, converge and keep the promises of PRD §10.2 under hostile delivery, before any network, server or encryption exists? Answer: not as written. The simulation found seven defects (below); with them fixed, nine properties hold, each with a negative control.

## What it is
- **The rules under test are the real ones**: `kinige-domain::sync` (merge by field groups, `conflict` revisions, `vv`, purge and tombstone verdicts, the binding and resync table, the feed while a parent is missing) and the existing `rewrite_group`, `rewrite_body`, `new_message`, `new_chat`, `Clock`. The group bytes, `groups_raw` and `hlc_json` are the formats the app writes.
- **The simulation** (`src/`): `server.rs` (in-memory: `seq` under one lock, idempotent push by `op_id`, `stale_purge`, `purged` kept 180 days, compaction, a restore from a backup with a new generation), `client/` (records, outbox with compaction, cursor, `server_known`, tombstones with `cascade_of` and `held`, quarantine, binding, full resync), `world.rs` (the network and the virtual clock with skew per device), `oracle.rs` (a record of every write, kept outside the clients), `schedules.rs` (proptest generators), `props.rs` (the properties and the runner), `switches.rs` (the breakages for the negative controls).
- **Network**: a push is dropped, answered into the void (= a restart between "sent" and "ack saved"), delivered twice, cut after n envelopes, or arrives reversed; a pull page is dropped, delivered twice, replayed late (an older answer), or applied in another order; a device restarts; the server is restored from a backup. A device's physical clock is off by up to 3 hours (never beyond `SKEW_MAX`).
- **Envelopes** are plaintext structs with the open header fields of §5.2 and a seal over header and payload that stands for AEAD (the server never changes a header). Encryption is S4b.

## Run
```text
cd spikes/S4a-sync
cargo test                                   # 10 000 cases per property for 2 devices, 2 000 for 3, seed 1: about 25 s
S4A_CASES=1000 S4A_CASES3=200 S4A_CONTROL_CASES=3000 S4A_SEED=1 cargo test --test properties --test negative --test scenarios   # what CI runs
cargo test --test negative -- --nocapture    # the negative controls with their reports
S4A_STATS_N=5000 cargo test --release --test stats -- --nocapture   # how much of the protocol the schedules reach (the table below; default 2000)
cargo run --release --bin measure -- 100000  # the binding measurement
```
A failing property prints the property, the seed, the case count, the reason and the shrunk schedule; running again with the same seed and case count gives the same schedule. `S4A_CASES`, `S4A_CASES3`, `S4A_SEED`, `S4A_CONTROL_CASES` (default 20 000; the controls normally fail within a few hundred).

## Properties and cases
Numbers below: seed 1, `cargo test --release` with `S4A_CASES=30000 S4A_CASES3=6000`, on the last commit of this branch; every row passed at 30 000 cases for 2 devices and 6 000 for 3. The default `cargo test` runs 10 000 and 2 000 and also passes. The device-count rows for the generators other than "general" use the same counts (the restore-epoch generator has 2 devices only).

| # | Property                                                                                           | Generator                                          | Negative control (switch)                                                                                                           |
|---|----------------------------------------------------------------------------------------------------|----------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------|
| 1 | 2 and 3 devices converge to the same records; a fresh device rebuilds the same from the log        | general; chat purges                               | no `device_id` tiebreak; `seq` without the lock; cascade at the parent's ack                                                        |
| 2 | a concurrent edit or move keeps both versions (winner, a `conflict` revision, or its history)      | edits and moves; compacted chains                  | no revisions; no `vv`                                                                                                               |
| 3 | a restore with a newer HLC beats the older delete, on skewed clocks                                | delete and restore                                 | delete always wins; the clock does not receive                                                                                      |
| 4 | a device back after 200 days does not resurrect a purged record                                    | one device away, others purge, compact             | no stale check; (scenario) a tombstone that keeps its own snapshot                                                                  |
| 5 | a repeated push is idempotent (same state, no new `seq`; the whole log replayed twice)             | general with many faults                           | no `op_id` deduplication                                                                                                            |
| 6 | edits of different groups are both kept (the newest write of every group is what the devices hold) | general                                            | whole-record last-writer-wins                                                                                                       |
| 7 | no orphan is shown; after the head nothing is hidden for good                                      | chats and moves; chat purges                       | orphans shown at once; children hidden for as long as the tombstone lives; (scenario) the initiator shows what it is about to purge |
| 8 | after a server restore nothing acked is lost; the honest limit is asserted and no more             | backups and restores; drop epochs across a restore | the new generation is ignored; the epoch counter restored with the backup                                                           |
| 9 | no false conflicts when one device writes (beyond the eight)                                       | one writer                                         | the winner's own base counted as concurrent (finding 5)                                                                             |

Every property has a negative control that fails (`cargo test --test negative`, 15 controls, reports in the PR description); findings 2 and 6 and the review's blocker and feed items are also named scenarios that fail with the old rule and pass with the new one (`tests/scenarios.rs`).

## Coverage
`cargo test --test stats` over 5000 schedules per mix (seed 7): the interesting paths are reached.

| mix                     | steps   | purges accepted | quarantined | cascade children | stale-cursor resyncs | new-generation resyncs | first bindings | conflict revisions | lifted tombstones |
|-------------------------|---------|-----------------|-------------|------------------|----------------------|------------------------|----------------|--------------------|-------------------|
| general, 2 devices      | 193 167 | 175             | 65          | 5                | 0                    | 0                      | 2 441          | 378                | 0                 |
| conflicts, 2 devices    | 190 753 | 0               | 0           | 0                | 0                    | 0                      | 0              | 1 291              | 0                 |
| chats, 3 devices        | 243 020 | 334             | 174         | 18               | 0                    | 0                      | 2 526          | 588                | 1                 |
| server restore, 2       | 192 798 | 115             | 59          | 3                | 0                    | 3 920 (1 with both)    | 0              | 535                | 0                 |
| device away 200 days, 2 | 178 035 | 3 804           | 2 578       | 0                | 3 149                | 0                      | 0              | 60                 | 0                 |
| compacted chain, 2      | 89 940  | 0               | 0           | 0                | 0                    | 0                      | 0              | 4 256              | 0                 |
| chat purges, 3          | 200 841 | 2 521           | 1 364       | 64               | 0                    | 0                      | 0              | 727                | 34                |
| single writer, 2        | 193 489 | 175             | 74          | 0                | 0                    | 0                      | 0              | 0                  | 0                 |

Counts are over 5000 schedules per mix; revisions are those on device 0 at the end. The general mix reaches the cascade rarely (5 children in 5000 schedules); the chat-purge mix was added for that.

## Findings
Each is a shrunk schedule that broke a property on the rules as ADR-0001 revision 7 stated them, with the fix. Seeds are for `S4A_SEED`, counts are the cases it took.

1. **"A cursor older than 180 days is stale" loops.** Property 1, seed 1, 200 cases, reason "the network did not quiesce": `CreateChat, Push(DropResponse), CreateChat x2, Advance(181 days), CreateChat x3`. The cursor's age was the age of the last envelope it had seen, so an idle account was stale forever (the new cursor is as old as the old one); a second schedule showed a resync cursor in the middle of its pages called stale again. Fix: stale means a compaction dropped a `purged` (and its entity's `upsert`s) above the cursor since the device last heard (a drop epoch carried by every page and sent back); a pull from 0 is never stale. ADR-0001 §6.3. **Review round:** the first epoch had a hole (guardian): a restored server counted epochs from its backup, so a drop after the restore could carry a number a device already held, the device was not told it was stale, and the records it had not seen purged came back live (`tests/scenarios.rs`, `review_epoch_*`; the old rule is the control `control_8_the_epoch_counter_restored_*`, random schedules of the right shape find it in a few hundred cases; the general `RESTORE` mix did not, because its `compact` and `long_gap` weights were 0). Fix: the epoch counter is not part of the backup and only grows (a drop never gets a number a device holds), the device forgets `known_epoch` when the generation changes, and "both causes at once" is answered by the server (`GenerationChanged { stale }`): the restored log is a prefix of the old one, so the restored drops are held against the old cursor. The first answer to "both causes" was the device's own clock (last page older than 180 days); it quarantined everything a restore had taken from an idle device, Inbox included, and was dropped.
2. **A confirmed version is lost without a revision.** `tests/scenarios.rs`, `finding_2_*`; found by property 2 with the compaction generator (`chain_schedule`, seed 1). B pushes its edit P (acked) and goes away; A, which had not seen P, edits twice, the second time from its own confirmed first edit v (`W.base = v`); the server compacts v; B returns, pulls only W, and P is below `W.base`: "stale", no revision. The ADR's own-unconfirmed rule covers only the device's unacked version. Fix: `vv`, the newest HLC per device in a version's ancestry; a loser of another device is in the winner's history exactly when `vv` says so. Exact, no special case, and `supersedes` stops mattering (ADR-0001 §3.4, `kinige_domain::sync::next_vv`). Control: `control_2_without_vv_*`.
3. **Deleting the chat's messages at the chat's ack loses data.** Property 1 on the chat-purge generator, seed 1, 3000 cases, `control_1_cascade_*` (the report is in the PR): device 1 edits message M, pushes (ack lost), and purges the chat; device 0 has moved M out of the chat. At the chat's ack device 1 deletes M with the edit only it had; the server answers `stale_purge` for M; the lift recreates M from device 0's snapshot, and device 1's own acked envelope is skipped on pull. Devices differ for good. Fix: the chat's ack deletes the chat only; each message is deleted at its own ack (ADR-0001 §3.5, §3.6).
4. **A child the initiator never saw is hidden for 180 days.** `control_7_without_the_grace_*`: a message written in a chat on one device and pushed after the other device purged the chat; nobody purges it and the rule "children are hidden, their `purged` follows" hides it for as long as the tombstone lives. Fix: hidden while pull is short of the head or for a day (`PURGED_CHILD_GRACE_MS`, `tombstone.purged_at`), then Inbox "chat deleted" (§3.6).
5. **"Unconfirmed" per record made false conflicts and wrong lineages.** Property 2 (shrunk: `CreateMessage@1, CreateChat@0, Pull@1, Push@1 Reorder, Pull@1 x2, Pull@0, Edit@0, Move@0 x2, Move@1`, seed 1, 3000 cases) and property 9 (shrunk: `Pull@1, Push@1 DropResponse, CreateMessage@1, Pull@1, Push@1 DropResponse, Pull@0, Edit@0, Favorite@1`). An `upsert` is a snapshot, so an unacked row carries foreign groups; the device took them for its own unconfirmed versions, wrote `base = null` and a foreign HLC into `supersedes`, and made a false revision when the winner's `base` was its own version whose ack was lost. Fix: unconfirmed is a version this device wrote (its id is in the HLC) and the server has not acked; `W.base = P` is never a conflict (§3.4).
6. **A tombstone kept the device's own snapshot.** `tests/scenarios.rs`, `finding_6_*` (random schedules reach it after about 30 000 cases). Two devices purge the same message; the server takes the first `purged` (its snapshot includes a later flag change) and answers the second `ok`. The second device's tombstone keeps its older snapshot, and the flag change, pulled after the `purged`, lifts a purge the server made: the message is live again on that device only. Fix: a pulled `purged` sets the tombstone's snapshot to the accepted one (§3.5). Left for S4c: the answer to a repeated `purged` should carry it.
7. **"Pull first, then push" is load-bearing.** Property 4, seed 1: a device that pushed without pulling after the server had compacted away the `purged` resurrected the record (`Advance, Compact, Push(1)`). The simulation therefore only pushes right after a pull that reached the head (a cycle lasts a minute), and the ADR says so (§3.5); S4c may make the server refuse a push whose epoch is behind a drop.

Found in the review round (all with a scenario in `tests/scenarios.rs`, the old behaviour behind a switch): the initiator of a chat purge showed the chat's messages in Inbox after the day while their `purged` sat in its own outbox (now hidden while the message's own `purged` is unacked); a record in quarantine that the server sent again was applied live, so it was in the feed and in quarantine (now merged into the quarantined copy); the revision rule ignored `sv` (a client that does not support the `sv` of either version makes no revision; unit tests with an `sv` from the future and the hostile-input test).

Also seen, not defects: the set of `conflict` revisions depends on the order in which a device saw the versions, so a device that joins later makes revisions of its own, with deterministic ids, which then spread (ADR-0001 §3.1 says so; §3.4 now says it again). Quarantine is a device's own decision: a message that one device holds in quarantine may be live on another (the property compares everything else).

## Model assumptions (what a reader should not take for more)
- The purge pass runs at the end of a sync cycle, after a pull to the head (`Step::Purge` pulls first). A purge on a half-pulled view was seen to destroy a record whose newer version was in the next page, in one case that is not reachable otherwise (a purge without an account of a copy); not claimed as a finding.
- A device pushes only after a pull that reached the head within the last minute of virtual time (`CYCLE_MS`).
- The interface cannot delete what is in the trash or restore what is not (no write, no new HLC).
- `edit` revisions are not modelled: a version that was edited into a kept version counts as history, not as lost. Attachments are not modelled; a message's revisions stand in for children in the cascade.
- Revision ids are a deterministic stand-in for `crypto_generichash(K_rev, ...)`; the envelope seal is a hash. Both are S4b.
- `clock_skew` beyond 24 h and leaving it, `pending_unknown`, `malformed`, a database copy with the same record ids bound later (ADR-0001 §6.2 item 6) and two live copies of one `device_id` are not exercised by the simulation. The last, and unknown groups and an `sv` from the future, have unit tests in `kinige-domain::sync` and a hostile-input test.
- Cases are random schedules of at most 90 steps over a handful of records; a bug that needs more than that or a rarer shape stays unseen. Finding 6 needed about 30 000 cases.

## The binding measurement (ADR-0001 §6.2 item 4)
`cargo run --release --bin measure -- 100000`. Machine: Apple M3 Pro, 18 GB, macOS 26.5.2, Rust 1.94, release. A synthetic database of 100 000 entries in 40 chats (300-700 bytes of text, 490 on average, 5 years of days), all `server_known = 0`, no account; the first binding against an empty server.

| what                                                                            | result                                    |
|---------------------------------------------------------------------------------|-------------------------------------------|
| envelopes made                                                                  | 100 041 (one per record)                  |
| building them from the database (real group code, plaintext)                    | 0.23 s (2.3 us each)                      |
| the in-memory server taking them in order                                       | 0.07 s (0.6 us each)                      |
| payload, plaintext JSON of the groups                                           | 110.1 MB (1.1 KB each)                    |
| wire size with header, nonce, tag, padded to a multiple of 1 KiB (appendix A.1) | 233.1 MB, 2 330 bytes each, at most 2 636 |
| requests at 100, 500, 1000 envelopes                                            | 1 001, 201, 101                           |
| memory of the process (database, outbox and server together)                    | about 750 MB                              |

The protocol's share is small. What costs is the bytes: 4.2 times the text, because an `upsert` is a snapshot of five groups (and `vv`) and padding rounds 1.1 KB up to 2 KiB. Encryption (about 100 000 XChaCha20-Poly1305 operations on 2 KB), the network (233 MB) and SQLite are not in these numbers; S4b measures the first, slice 11 the rest.

## Not verified
- That a real SQLite outbox and the real server give the same results: the model is in memory, one thread; the `seq` lock is a function call.
- That 30 000 random cases are enough: see Model assumptions.
- The `vv` rule on data written by slices 1-7 (old groups without `vv` fall back to the old rule; unit-tested, not simulated with mixed devices).
