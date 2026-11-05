# KINIG-94: spike S4a, sync protocol simulation

Budget: ~8 h to merge (code ~4 h, guardian and review ~3–4 h); stop and report at 16 h (the PRD's 5 days are calendar days for a person). Huly: KINIG-94 (under KINIG-20, slice 11); branch `spike/S4a-sync`, PR and first commit `KINIG-94: …`. Release: none (no app change visible to the user). Source: PRD §10.2 (criteria S4a), §14, ADR-0001 §3–§6 and its appendix.

The spike answers one question: **does the protocol of ADR-0001, as written, converge and keep the promises of §10.2 under hostile delivery?** Before any network, server or encryption exists. Where it does not, the spike finds the counterexample, fixes the rule, and the ADR and PRD are edited in the same PR.

## Scenario
1. Running `cargo test` in `spikes/S4a-sync/` builds 2 or 3 simulated clients and one in-memory server. Thousands of random schedules run over them on a virtual clock: creates, edits of `body`, `flags` and `placement`, moves, deletes, restores from the trash, purges after 30 days, chats deleted with their messages, a device offline for 200 days, the binding of a database that has entries made before an account existed. Delivery randomly delays, duplicates and reorders, connections drop mid-push and mid-pull, and a client restarts between "envelope sent" and "ack saved".
2. After every schedule the network quiesces (everyone pushes and pulls to the head). The properties below are checked. A failure prints a shrunk schedule and its seed, and the same seed reproduces it.
3. The merge itself is not spike code. It is the real `kinige-domain` code that slice 11 will call, working on the formats the app already writes (`groups_raw`, `hlc_json`, `sv`).

## Included
- **`kinige-domain::sync` (production code, pure, no IO, builds for wasm32):**
  - merging a record: the element-wise maximum over groups, with the HLC tie broken by `device_id` (§3.1, §3.3);
  - which losing `body` / `placement` becomes a `conflict` revision, with `base` and `supersedes` (§3.4);
  - the `deleted` group and restore (§3.5);
  - `purged` with its `purged_of` snapshot: whether the server or a client must refuse it, quarantine it, or apply it (§3.5, §3.6);
  - children waiting for a parent, and "first the chat, then its messages" (§3.6);
  - the decision table of §6.2 and §6.3 (push, quarantine or drop on binding and on full resync);
  - unknown groups and a newer `sv` kept as raw bytes (§4).
  Only what the simulation needs, and only rules ADR-0001 already states. Code for each rule cites its ADR section. Use the existing types and modules (`hlc`, `groups`, `jcs`); don't duplicate them.
- **`spikes/S4a-sync/` (a separate cargo project like `spikes/S5-search`, not a workspace member; path dependency on `kinige-domain`):**
  - **Server model:** `seq` issued in order under one lock, the cursor `(generation, seq)`, idempotent push by `op_id` (the same `op_id` with other bytes is an error), `stale_purge`, keeping `purged` ≥ 180 days, "cursor is stale" past that, compaction of covered envelopes past retention, a restore from a backup (new generation).
  - **Client model:** records, the outbox, the cursor, `server_known`, quarantine, `held` children. A pull page and its cursor are applied as one step.
  - **Network:** delay, duplicate, reorder, drop, crash/restart between steps.
  - **Virtual clock with skew per device:** an HLC ahead by more than 10 minutes produces the warning event, never a block.
  - Envelopes are plaintext structs with the same open header fields as §5. Encryption is S4b; the AEAD binding is modelled only as "the server cannot change the header" (the header is compared, not decrypted).
  - **Properties (proptest)**, from §10.2 S4a:
    1. 2 and 3 clients converge to the same record state after quiescence, under any schedule;
    2. a concurrent text edit keeps both versions (one wins, the other is a `conflict` revision on every device that saw both: none is lost);
    3. a restore from the trash with a newer HLC reaches the other device and beats the older delete;
    4. a device back after 200 days does not resurrect what was deleted and purged (it gets "cursor is stale", does a full resync, and the record goes to quarantine, not back to the journal);
    5. a repeated push is idempotent (the same state, no new `seq`);
    6. concurrent edits of different groups are both kept;
    7. no orphan is shown once pull reaches the head;
    8. after a server restore (new generation), nothing acked after the backup is lost once clients resync (where ADR-0001 says it is an honest limitation, the test asserts that limitation and no more).
  - **A negative control for each property:** a test-only switch that breaks the rule (no `device_id` tiebreak, no conflict revision, delete always wins, no cursor-age check, `seq` handed out without the lock). The property must fail on it. A property that passes on broken code proves nothing.
  - **Measurement:** the time and the number of envelopes to bind a database of 100k entries, the size of the Telegram import (ADR-0001 §6.2 item 4 says "to be measured in S4a"). Put it in the README with the machine and the command.
- **`spikes/S4a-sync/README.md`:** how to run it, the case counts, the seeds, what was found, and what each finding changed.
- **Findings change the documents in the same PR:**
  - ADR-0001 rules that turn out wrong or underspecified: edit the ADR in place, marked with the date and "S4a";
  - a short ADR-0017 "S4a results", half a page: what was proven, what was found, what is left for S4b, S4c and slice 11;
  - the PRD changelog gets a line.

## Not included
- Network, HTTP, axum, Postgres, the real `seq` lock in SQL (S4c).
- Encryption, keys, Argon2, envelope bytes on the wire (S4b).
- Changes to `kinige-store`, the SQLite schema or migrations, and anything in the app or UI. If the spike shows the local schema is missing something, write it down in ADR-0017 for slice 11; don't change it here.
- Blobs and the resumable upload (only the "blob before its envelope" order is modelled, as an event).
- Performance tuning beyond the one 100k binding measurement.

## What "done" looks like
- `cargo test` in `spikes/S4a-sync/` passes:
  - at least 10 000 cases per property for 2 clients and 2 000 for 3;
  - in CI, a smaller fixed count with fixed seeds so the job stays short, plus the full count in the PR evidence.
- Every negative control fails, and its output is in the PR.
- The `kinige-domain` checks pass: `cargo test -p kinige-domain`, clippy `-D warnings`, the wasm32 build. The new rules have their own unit tests, including garbage input: a corrupt `hlc_json`, an unknown group, an `sv` from the future, an HLC with a bad width or device id.
- The README, ADR-0017, any ADR-0001 edits and the PRD line are in the PR.
- CI is green. The spike gets a CI step (an existing job or its own small job), so it does not rot.

## Touches sensitive areas
Yes: the sync protocol (CLAUDE.md "Sensitive areas"). `sync-crypto-guardian` reviews before merge and checks the merge rules against ADR-0001 line by line. `slice-reviewer` checks the spec and the evidence. No schema, migration, key or app change. One new dev dependency is expected: `proptest`, MIT/Apache, checked by `cargo-deny`.
