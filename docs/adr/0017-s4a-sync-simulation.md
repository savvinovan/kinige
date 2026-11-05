# ADR-0017: S4a results, the sync protocol under hostile delivery

Date: 2026-11-05. Status: **accepted** (KINIG-94; spec `docs/slices/S4a-sync-simulation.md`, code `spikes/S4a-sync` and `kinige-domain::sync`, numbers and counterexamples in `spikes/S4a-sync/README.md`). Changes ADR-0001 (revision 8, each change marked "S4a, 2026-11-05") and PRD §10.2; no schema, migration or app change.

## What was proven
The protocol of ADR-0001, with the fixes below, converges under random delay, duplication, reordering, dropped and cut requests, delayed pages, restarts, device clocks off by up to 3 hours, a device away for 200 days, a restored server and a first binding. The real merge, conflict, purge, resync and feed rules (`kinige-domain::sync`) run against an in-memory server and 2 and 3 clients. Nine properties (the eight of PRD §10.2 and "no false conflicts") hold on 10 000 cases for 2 devices and 2 000 for 3 each (more in the README), and each has a negative control: a switch that breaks one rule makes it fail.

## What was found
Seven things in ADR-0001 were wrong or missing; each has a shrunk counterexample with its seed in the README.
1. **"Cursor older than 180 days" is not a test.** By the age of the last envelope seen, an account idle for 181 days is stale forever; a resync cursor in the middle of its pages was called stale again. Now: stale means a drop (epoch) above the cursor since the device last heard from the server (§6.3). The first version of the epoch had a hole (review, guardian): a restored server counted epochs from its backup, so a later drop could carry a number a device already held and was not seen. Epochs are now unique across generations, the server answers "both causes at once" itself, and a device forgets its epoch when the generation changes.
2. **`base` and `supersedes` do not tell concurrent from old.** A confirmed version below the winner's `base` was lost with no revision after a compaction. Now `vv`, one HLC per device (§3.4).
3. **Deleting a chat's messages at the chat's ack loses data** when a message was moved by someone else: the initiator's own acked edits are gone and the lift recreates the message without them. Now each message is deleted at its own ack (§3.5, §3.6).
4. **A message the initiator had not seen is hidden for as long as the chat's tombstone lives.** Now hidden for a day, then Inbox (§3.6).
5. **"Unconfirmed" was per record**, so a snapshot row made foreign groups look the device's own; and the device's own version that is the winner's `base` made a false revision (§3.4).
6. **A tombstone kept the device's own snapshot** when the server had taken another device's `purged`; a later envelope lifted a purge the server had made (§3.5).
7. **"Pull first, then push" is load-bearing.** A push that skips the pull resurrects a purged record after the server drops its `purged`. The simulation runs the cycle as the ADR says; S4c may enforce it.

The spike adds one test-only dependency tree, `proptest` 1.12 (MIT OR Apache-2.0) with default features off, which brings `rand` 0.10, `rand_core`, `rand_xorshift`, `chacha20`, `getrandom`, `unarray`, `regex-syntax`, `bitflags`, `num-traits`, `core_detect`, `cfg-if` and `libc`; it is outside the workspace and `cargo deny` runs over it in CI with the project's `deny.toml`.

## What is left
- **S4b:** the keyed hash of the `conflict` revision id (the simulation uses a stand-in); the AEAD bytes (the simulation seals header and payload with a hash).
- **S4c:** the server answers a repeated `purged` with the accepted snapshot, and a push with a `known_epoch` behind a drop is refused; `drops` and `epoch` are server state, `epoch` outside the backup; `drops` grows with every compaction that drops a `purged` and nothing trims it (a drop older than every device's `known_epoch` can go, which needs the server to know its devices); the `seq` lock is real SQL.
- **Slice 11:** write `vv` next to `base` and `supersedes` in `rewrite_group` (the app does not write it yet; old versions without it fall back to the old rule); `sync_state.known_epoch`; an outbox column for "send after the parent's ack"; the purge pass after a pull; `tombstone.purged_at` already exists.
- **Not modelled:** attachments and blobs (a message's revisions stand in for children), `edit` revisions, `clock_skew` beyond 24 h and its exit, `pending_unknown`, `malformed`, the first binding of a database copy with the same record ids, two live copies of one `device_id` (domain unit tests only), unknown groups and `sv` from the future (domain unit tests only).
- **Limits of the claim:** quarantine is a device's own decision, so a message in quarantine on one device may be live on another; the set of `conflict` revisions depends on delivery order and a new device adds some.
