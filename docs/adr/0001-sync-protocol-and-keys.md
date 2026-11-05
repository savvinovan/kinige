# ADR-0001: Sync protocol and keys

Date: 2026-10-20. Status: **accepted** on 21.10.2026 (the user's decisions were received and PR #3 was merged by the user).
Revision 8 (S4a, 2026-11-05): fixes from the simulation of the protocol (`spikes/S4a-sync`, ADR-0017) — `vv` in `body` and `placement` and an exact concurrency test (§3.4), "unconfirmed" is a version this device wrote (§3.4), the initiator deletes a chat's messages on their own ack (§3.5), a pulled `purged` sets the tombstone's snapshot (§3.5), the children of a purged chat are hidden for a day and then shown in Inbox (§3.6), "cursor is stale" means something was dropped after the device last heard from the server (§6.3). Each change is marked "S4a, 2026-11-05" in the text. Review rounds, same day: an own envelope met in a pull is merged when a lift left the record without it (§3.5), the epoch of a resync (§6.3), drops stored with their generation, a lineage of generations inside the dump (restore into a clean installation) and "both causes at once" answered by the server (§6.3), the initiator does not show children it is about to purge (§3.6), a quarantined record is not applied live (§6.3), no revision for an unsupported `sv` (§3.4). Revision 7: small items from the sixth `sync-crypto-guardian` review — the lifecycle of `cascade_of` and `held`, lifting a tombstone removes a pending `purged`, a `purged` response to a push without a tombstone. Revision 6: fixes from the fifth `sync-crypto-guardian` review — `purged_of` became a snapshot of the HLCs of all groups (closes a silent loss of a move within the purge window), a cascade of tombstones on binding and on a `generation` change — parent first (`cascade_of`, `held`), a single definition of a chat's children and of returning from quarantine, small items. Revision 5: fixes from the fourth `sync-crypto-guardian` review — chat purge "chat first, then messages", a revision for the device's own unconfirmed version, `raw` as a string, `remote_blob_id` at attachment creation and a new id on re-upload, idempotent registration and recovery, the external import id in `ext`. Revision 4: the user's decisions ("User decisions" below) and three "important" remarks from the third `sync-crypto-guardian` review (derived Inbox, the Argon2 profile on recovery, false `conflict` revisions). Revision 3: the reviews of `sync-crypto-guardian` (two rounds) and `codex-second-opinion` are incorporated.
The group table, byte formats, check order, test plan, schema and sources are moved to [`0001-sync-protocol-and-keys.details.md`](0001-sync-protocol-and-keys.details.md); below, that file is called "the appendix".
Basis: §4 and §10 of the PRD (`docs/plan/02-prd.md`, v0.8), `research/review-fable.md` items 1–7, 13–16, `research/review-codex.md` items 1–13, 16.
The PRD is normative. This ADR makes concrete what the PRD left "for the ADR". Deviations from the PRD are marked **[deviation]**; the user accepted them, and the PRD was brought in line in v0.8.

## Terminology

The Russian original used several words for some of these concepts; this translation uses one term each.

| Term                    | Meaning in this ADR                                                                                                                                 |
|-------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| record                  | any synchronized entity: chat, message, attachment, message_revision. A user's journal entry is a message record                                    |
| field group, group      | a named set of fields of a record (`core`, `body`, `placement`, …) with its own HLC and `sv`; the unit of merge                                     |
| envelope                | one immutable encrypted operation (`upsert` or `purged`) as stored in the outbox and on the server (§5)                                             |
| snapshot                | the full set of a record's groups carried by an `upsert` (§3.1); `purged_of` is a snapshot of HLCs only (§3.5); a held snapshot is `tombstone.held` |
| delete / restore        | moving a record to the trash and back: a write of the `deleted` group (§3.5)                                                                        |
| purge                   | physical deletion of a record from the trash (after 30 days or "delete forever"), announced by a `purged` envelope                                  |
| tombstone               | the local row that remembers a purged record (`purged_of`, `cascade_of`, `held`), kept for at least 180 days                                        |
| cascade                 | the `purged` envelopes and tombstones of a purged record's children (`cascade_of` = id of the parent)                                               |
| quarantine              | a local holding area for records that must not be silently deleted or resurrected; the user decides (slices 3 and 11)                               |
| `malformed`             | quarantine reason for an envelope that passed AEAD but failed parsing (§5.4)                                                                        |
| confirmed / unconfirmed | a version the server has acknowledged (`acked_seq`, `server_known = 1`) or not yet                                                                  |
| binding                 | attaching a local database to an account for the first time (§6.2)                                                                                  |
| full resync             | pulling everything from `seq = 0` after a `generation` change or a stale cursor (§6.3)                                                              |
| compaction              | the server or the outbox dropping envelopes whose groups are all covered by newer ones                                                              |
| recovery                | regaining account access with the 24 recovery words (§8.6)                                                                                          |
| restore from a backup   | replacing the local database with a backup file (§1); not to be confused with recovery                                                              |
| database copy           | another instance of the same database file (a restored backup or a cloned profile), §1                                                              |
| server log, app log     | server log: the server's per-account envelope log, numbered by `seq`; app log: the client's log file (§8.8); neither is the journal                 |

## Context

The local schema of slice 1 depends on this ADR: ids, `hlc_json`, `schema_version`, raw group bytes, the "known to the server" flag. That is why it must be accepted before slice 1, even though the server only appears in slice 11.
Constraints from the PRD: the server does not see content and does not merge it (§10.1). Clients merge, by field groups with HLCs (§10.2). Deletion is an ordinary group. Envelopes are immutable. The login password is separate from the KEK, and the DEK has two wraps (§10.3). Unknown fields are preserved (§10.4).

## Decision

### 1. Identifiers

| What                                                  | Format                                                                                                                                                                         | Created by                                                                                                                            |
|-------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------|
| Records (chat, message, attachment, message_revision) | UUID v4, 36 lowercase characters (§4.1)                                                                                                                                        | the device                                                                                                                            |
| Inbox                                                 | one constant for all accounts; the `core` group is a constant too (decision 2)                                                                                                 | the code                                                                                                                              |
| Revision with `reason='conflict'`                     | deterministic: 16 bytes of `crypto_generichash(K_rev, message_id[16] ‖ group[1] ‖ hlc(P)[56])`, `group`: 1 = `body`, 2 = `placement` (§3.4); version and variant bits as in v4 | any device, identical on all                                                                                                          |
| `remote_blob_id` (attachment)                         | UUID v7, stored in the attachment's encrypted `core`                                                                                                                           | the device when it creates the attachment, in the same transaction (§7)                                                               |
| `op_id`                                               | UUID v4                                                                                                                                                                        | the device when it enqueues into the outbox                                                                                           |
| `device_id`                                           | UUID v4, created on first launch, stored in `sync_state`                                                                                                                       | the device                                                                                                                            |
| `account_id`                                          | UUID v4                                                                                                                                                                        | the **client** at registration. The server checks uniqueness; if the id is taken, the client creates a new one and rebuilds the wraps |

**`device_id` is unique per database instance, not per machine.** Restoring from a backup (the daily backup of §2.1, the "restore" import) always creates a new `device_id`.
How a database copy is detected: a second instance of `device_id` is kept in the OS secret store as a **non-synchronizable** item. If at startup it does not match the database, this is a copy.
Not detected: a copy restored on the same machine, and a clone of the whole profile together with the secret store. This is almost harmless: pull goes first, an HLC tie is resolved deterministically (§3.3), and concurrent edits by clones are caught by item 3 of §3.4 (the `base` test; `supersedes` only covers a clone's own chain). **S4a:** `vv` is per device id, so it cannot tell two clones apart; a clone's concurrent version that reaches a third device through the other clone's chain can look "seen" there and is not revisioned. It is not lost at the clones (each keeps its own until it merges, and merges between the two clones go through item 3), and the cost of the rare case is accepted. The mechanism and `synchronizable` in the secret-store crate — S4a, not verified.

### 2. HLC

**Format** (§10.2): `{wall_ms:013}-{counter:05}-{device_id}`.
- `wall_ms` is 13 decimal digits, `counter` is from 00000 to 99999, `device_id` is 36 lowercase characters.
- The length is 56 characters. The parser is strict: any deviation from the format (including `wall_ms` longer than 13 digits) is a format error (§5.4).

**Comparison** is a byte-wise string comparison. Because of the fixed width it is the same as comparing the tuples `(wall_ms, counter, device_id)`.

**Algorithm** — Kulkarni et al., 2014, fig. 5 (checked against the text of the paper). `pt` is the physical time in ms.
- **Local event:** `l' = l; l = max(l', pt); c = (l == l') ? c + 1 : 0`.
- **Receiving m:** `l' = l; l = max(l', l_m, pt)`. Then:
  - if `l == l' == l_m`, then `c = max(c, c_m) + 1`;
  - else if `l == l'`, then `c = c + 1`;
  - else if `l == l_m`, then `c = c_m + 1`;
  - else `c = 0`.
- **Counter overflow:** if `c` would exceed 99999, then `l = l + 1, c = 0`.

**When the clock ticks:**
- every local edit of group `g` sets `hlc[g] = now()`;
- the clock receives **every** HLC from the header of every envelope that passed AEAD. This includes envelopes that were not applied: own ones by `op_id`, those dropped because of a tombstone, those sent to quarantine. Before AEAD — never.

**Storage and restart.** The pair `(l, c)` is written to `sync_state` in the transaction of the change. At startup:
- `l = max(stored_l, pt)`;
- `c = (l == stored_l) ? stored_c : 0` — the counter continues; otherwise, after receiving a "future" timestamp and restarting, the HLC would repeat.

**Drift and the limit.** `SKEW_MAX = 24 h` is a protocol constant, the same as on the server (decision 5).
- The limit applies to the HLC only. The user may set any `journal_date`, past or future; it is the content of the entry. Sync does not rewrite `created_at`, including when leaving `clock_skew`: a time recorded with a wrong clock is only corrected by hand.
- A timestamp with `l_m − pt > 10 min` is a warning; work is not blocked (§10.2). The timestamp is accepted into the clock, otherwise a restore would lose to a "future" deletion.
- A timestamp with `l_m − pt > SKEW_MAX` is applied to the data but is **not accepted into the clock**. The user sees "the device clock is behind: edits and restores on this device may lose to others until the clock is fixed". This is a deliberate edge: a local restore loses to such a deletion until `pt` catches up with the timestamp.

**Leaving `clock_skew`.** The state is entered when the server rejected a push with a `clock_skew` response, or when at startup `stored_l − pt > SKEW_MAX`. When the clock is fixed:
1. The clock is reset: `l = max(pt, the maximum HLC among groups known to the server or received from it)`.
2. Unconfirmed envelopes whose HLCs are not beyond `server_now + SKEW_MAX` are first resent as is (a `duplicate` response is likely).
3. Only the **unconfirmed** groups from rejected envelopes — the server did not accept their HLCs — get new HLCs. Their envelopes are rebuilt with new `op_id`s. The dependent fields `base` and `supersedes` (§3.4) and the ids of `conflict` revisions are recomputed. Revisions of the device's own losing versions are not restamped separately: if the winner W was accepted by the server, the timestamp of the loser P is less than `hlc(W)` and by the time of leaving is already within `SKEW_MAX` — its envelopes go out as is in step 2.

Confirmed groups are not touched: their timestamps are not beyond `server_now + SKEW_MAX`.

### 3. Records, field groups and merge

**3.1. An `upsert` envelope carries a full snapshot of the record: all groups, each with its own HLC and its own `sv`.**
- The snapshot is self-contained: the record can be recreated from it after `purged` or a full resync.
- Merging a record is an element-wise maximum over groups. It is commutative, associative and idempotent, so the **state of records** converges under any order, duplicates and retries.
- **The set of `conflict` revisions may depend on the delivery order**: extra revisions are possible, but no loss (§3.4).
- Compaction on the server and in the outbox is trivial: an envelope is redundant if each of its groups is covered by envelopes with an HLC that is not smaller.

**3.2. Groups** — the full table is in the appendix, §A.0. In short:
- message: `core` (with the immutable `origin` — who created the record — and `ext` — the external import id), `body` and `placement` (both have the fields `base` and `supersedes`), `flags`, `deleted`;
- chat: `core`, `meta`, `order`, `state`, `privacy` (`ai_excluded` separately), `deleted`;
- attachment: `core` (including `blob_key` and `remote_blob_id`), `order`, `deleted`;
- message_revision: `core` only.

Derived data is not synchronized (§4.1).

**3.3. Merge (`kinige-domain`, no IO).** For each group in the union of groups, the larger HLC wins. If a group is missing on one side, the one that exists is taken.

**A group is stored and resent as raw bytes.**
- A group's value is canonical JSON per RFC 8785 (JCS). Groups contain only strings, booleans, `null`, objects, arrays and integers within ±2⁵³, so the JCS rules reduce to sorting keys by UTF-16 code units, minimal escaping and no whitespace.
- A client writes canonical bytes only for a group it edits itself, and only with a new HLC.
- Someone else's group it stores and resends **verbatim**, with its `sv`. Table columns are the parsed representation of these bytes (§4).

**An equal HLC with different bytes** is an integrity error. It is only possible with a database copy that has the same `device_id` (§1). Then:
- the lexicographically larger bytes win;
- for a message's `body` and `placement`, the losing group goes into a `conflict` revision (§3.4); for other groups and entities there is no revision;
- the event is written to the app log without content.

**3.4. Text and move conflicts, `conflict` revisions.**

The rules below are described for `body`; for `placement` (chat and date) they are the same: the same service fields, the same condition, a `conflict` revision with `group = 2` (decision 9). In the UI, in both cases, there is a "there is another version" mark with a choice of which one to keep (slices 3 and 11).

The `body` group has two service fields:
- `base` — the HLC of the version the edit was made from;
- `supersedes` — the list of HLCs of the device's own **unconfirmed** versions that this edit replaces.

How they are filled:
- Editing a version the server confirmed, or one that came from elsewhere: `base = hlc(current)`, `supersedes = []`.
- Editing the device's own unconfirmed version V: `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`. This way two offline edits look like one edit to others, even if the outbox compacted the intermediate envelope. An in-flight envelope without an ack is covered by the same list.
- The list is limited to 64 items: the oldest are dropped; in the worst case an extra revision appears.

**`vv` (S4a, 2026-11-05).** `body` and `placement` get a third service field, `vv`: a list of HLCs, one per device — for each device, the newest HLC of that device's versions in the ancestry of this version, this version included (at most 16 entries; the oldest drop out). A new version's `vv` is computed from the version it is made from alone: `vv(current)` with the entry of the writing device replaced by the new HLC (an old-format `current` without `vv` gives its own HLC, `base` and `supersedes`, one per device). It is additive, so no `sv` bump; `kinige_domain::sync::next_vv` computes it. `vv` says exactly which versions the author has seen; `base` and `supersedes` only say which one it was made from. The reason is a counterexample found by the simulation (S4a, finding 2, `spikes/S4a-sync/README.md`): device B pushes its edit P and goes away; device A, which had not seen P, edits twice (the second time from its own confirmed first edit v, so `W.base = v`) and the server compacts v; B returns and pulls only W. P is below `W.base`, looks like "a stale version", and is dropped without a revision — a lost version with no trace, although both were confirmed.

When merging `body`, the winner W and the loser P are **concurrent** if the texts differ and P is not in W's history. The test, in this order (`kinige_domain::sync::needs_conflict_revision`):
1. `hlc(P) ∈ W.supersedes` or `hlc(P) = W.base`: W was made from P (S4a: also when the ack for P was lost and P still counts as unconfirmed) — no revision.
2. **P was written by another device than W and W has `vv` (S4a):** P is in W's history exactly when W's `vv` has an entry of P's device that is not below `hlc(P)`; otherwise it is concurrent. This does not depend on which intermediate envelopes the server still has, and it needs no special case for the device's own unconfirmed version.
3. Otherwise (the same device wrote both — a profile clone, §1 — or W is in the old format without `vv`): `hlc(P) > W.base` (the first version has `base = null`, treated as −∞), and the device's own unconfirmed version (below) always makes a revision.

A causal HLC ancestor of W is always covered by W's `vv`, so a stale version produces no revision: a device that is two text versions behind and edited another group does not create false `conflict`s (its snapshot carries the old `body`). Two live copies with one `device_id` (a profile clone, §1) are caught by item 3: a conflict rather than a silent loss.

**The device's own unconfirmed version loses — always a revision** (item 3; where item 2 applies it decides exactly). If P is the device's own unconfirmed version, a `conflict` revision is created for any relation between `hlc(P)` and `W.base` — given the other conditions above (the texts differ, `hlc(P)` is neither in `W.supersedes` nor `W.base`). Otherwise a concurrent P with an HLC less than `W.base` would vanish without a trace when W's intermediate version was compacted on the server (a return after 180+ days, the first binding of a database copy). **"Unconfirmed" is a version this device wrote itself (S4a, 2026-11-05):** the HLC carries this device's id, and the record has `server_known = 0` or an outbox row without `acked_seq` carries that very version. An `upsert` is a snapshot of the whole record, so an unacked row also carries the groups that came from other devices; those are not unconfirmed versions of this device (taking them for such gave `base = null` and a foreign HLC in `supersedes`, and false revisions at later merges). The converse is accepted: a version this device wrote and the server acked, carried again by a later unacked snapshot row, still counts as unconfirmed — harmless, at worst one extra revision. A client that does not support the `sv` of either version makes no revision (`kinige_domain::sync` checks `SUPPORTED_SV`; another device will). The set of revisions depends on the delivery order anyway (§3.1), and the id is deterministic. **A device that joins later makes revisions of its own (S4a):** replaying the log in its order can find concurrent pairs that devices which saw the versions in another order did not. They are real versions with deterministic ids, they propagate, and this is an error on the safe side.

Then any device that sees this pair creates `message_revision(reason='conflict')`, if it supports the `sv` of both versions (otherwise another device will create the revision):
- the id is deterministic (§1);
- `snapshot_json` = `{"g": "body" | "placement", "h": hlc(P), "sv": sv(P), "raw": "<exact bytes of group P>"}` — `raw` is a JSON **string** with the verbatim bytes of the group (UTF-8, escaped per JCS), not a nested object: re-canonicalization by different clients would give different bytes for an equal HLC (§3.3);
- `saved_at` = `wall_ms` from `hlc(P)`;
- the HLC of the revision's `core` group = `hlc(P)`.

The revision's content is identical on all devices, and re-creation is idempotent. **[deviation]** from §10.2 and §4.2 of the PRD ("the losing full snapshot", "all groups"), accepted (decision 9). For `edit` revisions the full snapshot remains.
Without the DEK there is no `K_rev`: `conflict` revisions created before binding (the "merge" import, PRD §16) get a random v4; after binding, another device may create the same revision with a deterministic id — this is an extra revision, not a loss.

Residual risk: a chain of edits through a third device whose intermediate version did not arrive may give an extra revision. This is an error on the safe side. The "there is another version" mark is derived; closing it is slice 3 or 11.
Concurrent `flags` edits are resolved by LWW without a trace, as in the PRD (decision 9). The fields `base`, `supersedes` and (S4a) `vv` in `placement` are part of `sv = 1`; no `sv` bump is needed.

**3.5. Deletion, trash, restore, `purged`.**

- **Delete and restore** are a write of the `deleted` group with a new HLC; the larger one wins (§10.2). Editing a deleted record changes its own group; the record stays in the trash.
- **Purge with an account.** When `deleted_at` is older than 30 days by the local clock, the device enqueues `kind='purged'` with `purged_of` = **a snapshot of the HLCs of all the record's groups**, as the device sees them, including unknown ones from `groups_raw`: `{"<group>":"HLC"}`. A group missing from the snapshot is considered newer than it. The purge pass runs at the end of a sync cycle, after a pull that reached the head (S4a). The ack is handled in one transaction:
  - if no group of the record is newer than the snapshot, the record and its children are deleted and tombstones are written (a message: its revisions; a chat: **the chat only** — its messages are deleted when their own `purged` is acked, §3.6, S4a, 2026-11-05);
  - if at least one is newer (the user managed to restore or edit it), the record together with its children goes to **quarantine** — the server has already deleted it.
- **Purge without an account** (slices 1–10, or the device is not bound) is local only. The record is deleted, a tombstone is still written and kept **until the first binding, but no less than 180 days**: it will be needed at binding (§6.2); otherwise a database copy bound later would resurrect the record.
- **`purged_of` of children** is the same kind of snapshot of all the child's groups, including for revisions (they have `core` only). There are no exceptions from the server-side check.
- **Message cascade.** After the ack of `purged(message)`, the device sends `purged` for its revisions and attachments; their tombstones are marked `cascade_of` = the message id (appendix §C). A recipient of `purged(message)` deletes the children itself and writes tombstones for them with the same `cascade_of`. The lifecycle of `cascade_of` and `held` is in appendix §C.
- **The server orders purges** by the header only (decision 5; the check order is in the appendix):
  - `stale_purge`, if the server knows a group of the entity with an HLC greater than in the `purged_of` snapshot. It takes the maximum HLC per group from the open headers — the same index as for compaction; the snapshot reveals nothing new to the server;
  - after an accepted `purged`, subsequent envelopes of the entity get the `purged` response;
  - a repeated `purged` is a no-op.
- **A `purged` response to a push, or a pull of `purged(X)`.** If X has **any** group known to the device that is newer than the `purged_of` snapshot — the device's own unconfirmed one (in the outbox, or `server_known = 0` at the first binding) or a confirmed one (with an honest server such a purge would have got `stale_purge`) — then X **with its children** leaves the live tables and moves entirely to quarantine, and a tombstone is written. Otherwise X is deleted silently: a copy without edits loses nothing. No "I have it in my feed, others don't" divergence remains. A `purged` response to a push carries no snapshot: if there is no tombstone for X yet (the server accepted someone else's `purged(X)` between my pull and push), X goes to quarantine — the safe side — and the tombstone's snapshot is refined when a pull brings `purged(X)`. **The snapshot of a pulled `purged(X)` always becomes the tombstone's (S4a, 2026-11-05):** for an existing tombstone it replaces, per group, an older HLC with the pulled one. The `purged` in the log is the one the server accepted; the device's own may have been a no-op repeat with an older snapshot (two devices purged the same record, the server took the first). Without this, an envelope that is newer than the device's own snapshot but not newer than the accepted one lifts a purge the server already made, and the record comes back live on one device (S4a, finding 6). (A repeated `purged` is still answered `ok` without the check of the accepted snapshot; carrying it in the answer is for S4c.)
- **Tombstones** are kept for at least 180 days. An envelope for an id from a tombstone:
  - with an HLC of **any** group greater than in the `purged_of` snapshot — the purge was stale: the envelope is applied, the tombstone is **lifted** together with the tombstones of this cascade's children (`cascade_of`, recursively over the tree; only for unconfirmed children), and the children are recreated from the held snapshots (below). A lifted tombstone removes its not-yet-sent `purged` from the outbox. **An envelope of the device's own, met again in a pull (by `op_id`), is an ack and is merged as well when the live record lacks one of its groups (S4a review, round 2, 2026-11-05):** a purge removed the record, and the lift recreated it from another device's envelope, which may be older than this device's own writes; without the merge the device kept the old version while all the others had both. This is protection against a server that reordered the server log: the client applies the same rules itself; the server is not a trusted party;
  - otherwise it is dropped. The exception is a cascade child (a tombstone with `cascade_of`): its groups are merged into the held snapshot `tombstone.held`, so that the child comes back if the parent's tombstone is lifted later (§6.2, §6.3).
- **An attachment or revision that arrives after the `purged` of its message** is deleted locally, and the device enqueues `purged(child)`: otherwise the envelope and the blob would stay on the server forever. A repeat is a no-op. Messages of a purged chat do not fall under this rule — §3.6 applies to them.
- **The cycle is always "pull first, then push".** The purge pass runs at the end of a cycle, after a pull that reached the head (S4a). This is also what keeps a device that was away for 200 days from pushing an old edit of a record whose `purged` the server has already dropped: its pull answers "cursor is stale" first (§6.3). The simulation made this explicit: a push without the preceding pull resurrects (S4a, `spikes/S4a-sync/README.md`); S4c may also make the server refuse a push whose `known_epoch` is behind a drop.

**3.6. Dependencies and Inbox.**
- There are no foreign keys. A record without a parent is kept hidden until the parent arrives (§10.2) — but **only until pull has reached the head of the server log**. After a complete pull, a message with an unresolvable `chat_id` (the chat is neither among live ones nor in tombstones) is shown in Inbox marked "chat unavailable" (derived, as below). Otherwise a late message of a purged chat would vanish without a signal when the chat's tombstone expires, and on a new device after compaction of the server's `purged(chat)` — from the first pull. Attachments and revisions without a message stay hidden.
- Inbox is identified **by id**, not by a flag. Its `core` is a constant, so two Inboxes created before binding merge without a conflict. Merge ignores `deleted` of Inbox.
- **Children of chat C** — one definition for the ack, for quarantine and for the `purged` response (§3.5): messages with `chat_id = C` and a `placement` HLC **less** than the HLC of the chat's deletion (`deleted` in the chat's `purged_of` snapshot), with their attachments and revisions. Messages with a greater `placement` HLC are live (moved into the chat after the deletion): they do not go to quarantine and are not deleted on the ack.
- A message whose `chat_id` points to a chat in tombstones is not deleted. A live message of such a chat **is shown in Inbox marked "chat deleted"**; the chat's children are hidden — their `purged` follows — **for a day (S4a, 2026-11-05):** while pull has not reached the head, or less than 24 h have passed since the device wrote the chat's tombstone (`PURGED_CHILD_GRACE_MS`, `tombstone.purged_at`). After that a child whose `purged` has not come is shown in Inbox with the same mark — **except on the initiator, while the message's own `purged` is still in its outbox unacked** (S4a review): it is about to purge the message and does not show it. Without this a message the initiator had not seen when it purged the chat (written on another device before the chat's deletion, pushed later) was never purged by anyone and stayed invisible for as long as the tombstone lived (S4a, finding 4). This is a derived display: `placement` does not change and no envelopes arise. A synchronized move happens only through an explicit user action.
- **Chat purge** (decision 4) is done by one device, **chat first, then messages**:
  - it sends `purged(chat)` as a separate request; the server-side `stale_purge` check (§3.5) catches a restore of the chat made by another device — then pull and re-evaluate, the messages are not touched;
  - only after `ok(seq)`, in the same transaction as the ack handling, it enqueues into the outbox the `purged` of the chat's children (definition above) — **only the messages (S4a, 2026-11-05): a message and its revisions are deleted when the message's own `purged` is acked, not when the chat's is.** The ack of `purged(chat)` deletes the chat only; its messages stay in the live tables, hidden by the rule above, and each message's ack deletes it and enqueues the `purged` of its revisions (§3.5). The cascade is resumable after a crash, as for a message (§3.5). `stale_purge` for a message (it was moved out of the chat before the deletion, but the initiator did not see that) — pull and re-evaluate: the moved message is alive, with everything this device had of it, and is no longer a child; its purge is not repeated. (The earlier rule deleted the messages and their revisions at the chat's ack. A `stale_purge` for a moved message then left the initiator without the edits only it had — acked, so its own envelopes were skipped on pull — and the lifted tombstone recreated the message from the mover's snapshot: S4a, finding 3.)
  - recipients do not cascade by their own `placement`. A device that restored the chat offline receives `purged(chat)` first and takes the chat **together with its children** into quarantine; the `purged` of the messages arriving next are a no-op against the quarantine. A restored empty chat never happens.
  - **Returning from quarantine** (data; UI is slice 11): a record comes back as a copy with a new id — under the old one the server answers `purged`. A chat — with a new id; its messages — by moving `placement` into the new chat if their ids are not in tombstones, otherwise as a copy with a new id (attachments with new `remote_blob_id` and `blob_key`, §7). This way no duplicate live messages arise.
  - Edge case: if the initiator disappeared between `purged(chat)` and the cascade, or had not seen a message when it purged the chat, the chat's old messages stay alive on the server; they are hidden for a day and then shown in Inbox marked "chat deleted" (above). No data is lost. A device that holds such a message in quarantine (it restored the chat, or edited it) and another that shows it in Inbox do not agree on whether it is live: quarantine is a device's own decision (S4a).

### 4. Versions and format evolution

There are three version numbers.
- `format_version` (u8) is the first byte of the envelope. An old client stores an unknown envelope raw in `pending_unknown` and moves the cursor.
- `sv` (u32) — **per group**, in the header (in the AD) and in `hlc_json`. A record's `schema_version` is the maximum over its groups.
- The HTTP protocol version — the server answers with a clear "update the client" error (§10.4).

`sv` rules (the initial value is 1):
1. **Additive, without an `sv` bump:** a new group or a new optional field that **does not change the meaning of the other fields of its group**.
   - An unknown group is stored raw in `groups_raw` together with its HLC and `sv`, merged by HLC and included in the header's `groups` when resent — otherwise compaction would consider the old envelope not covered.
   - Unknown fields of a known group are preserved when the client rewrites the group with a new HLC.
2. **An `sv` bump of the group is mandatory** if a new field changes the interpretation of existing fields of its group (for example, `body_format` next to `body_md`) or a field's meaning changes. A dependency between groups means a new group or an `sv` bump of both.
3. **Mixed versions.**
   - Merge goes by HLC, regardless of `sv`.
   - A new client **does not migrate or restamp** a group with an old `sv`: the raw bytes and `sv` stay as they arrived and are resent verbatim (§3.3).
   - Migration happens only on read: columns and the cache are built by the group's migration function.
   - A group's `sv` is bumped only together with a new HLC, when the group is edited.
   - So an old client sees neither false "equal HLC, different bytes" nor a spreading "read-only" mode.
4. An old client stores a group with an `sv` higher than it supports raw. A record with such a group is read-only, with an offer to update (§10.4).
   - The old client creates no `upsert` envelopes for such a record.
   - It may create `purged` if the `deleted` group is in a supported `sv`.
   - The children of such a record are ordinary records of their own versions.
5. A client writes an edited group in its own `sv`. Local database migrations are done with a backup, without rollback (§10.4). A format change is a new ADR.

### 5. Operation envelope (`format_version = 1`)

**5.1.** The byte layout, the AD with field widths, and the limits are in the appendix, §A.1. In short:
- `crypto_aead_xchacha20poly1305_ietf` with the key `K_env[key_id]`;
- the nonce is 24 random bytes per envelope (the libsodium documentation allows a random nonce for XChaCha20);
- AD = domain label ‖ `account_id` ‖ **all open bytes before the nonce, as they are**. Authenticity is checked over the received bytes, so a canonical JSON of the header is not needed, and no open field can be substituted;
- the payload is padded to 1024 bytes with `sodium_pad`;
- the AD covers all fields the PRD requires (account, type, id, `op_id`, groups with HLCs, `purged`, `schema_version`), plus `format_version` and `key_id`.

**5.2. Header (visible to the server):**

```json
{"entity":"message","id":"…","op":"…","kind":"upsert",
 "groups":{"core":{"h":"HLC","sv":1},"body":{"h":"HLC","sv":1},"flags":{"h":"HLC","sv":1},"placement":{"h":"HLC","sv":1},"deleted":{"h":"HLC","sv":1}}}
{"entity":"attachment","id":"…","op":"…","kind":"purged","purged_of":{"core":"HLC","order":"HLC","deleted":"HLC"},"blob":"<remote_blob_id>"}
```

**5.3. Payload (encrypted):** UTF-8 JSON `{"groups":{"<g>":<canonical JSON of the group>}}`. The group's value is inserted as raw bytes (§3.3; in Rust — `serde_json::value::RawValue`, feature `raw_value`). Unknown groups and fields live there too.
- HLC and `sv` are not repeated in the payload: their only source is the header.
- The set of groups in the payload must match `header.groups`; duplicate JSON keys are forbidden.
- For `purged` the payload is empty, but padded and encrypted: a `purged` cannot be forged.
- The limit is 1 MiB before padding.

**5.4. Checks on pull** (appendix, §A.4): length limits before allocating memory → `format_version` and `key_id` (unknown ones go to `pending_unknown` together with `entity` and `id` from the transport) → AEAD → parsing and matching `entity`, `op`, `id` against the transport.
- **An AEAD failure** stops applying the page: the cursor does not move, the user sees an error.
- **A format error after successful decryption** (HLC, `sodium_unpad`, JSON, group mismatch) — the envelope is authentic but was built by a faulty client. It goes to quarantine as `malformed`, the cursor moves, the user sees an error.

### 6. Push, pull, cursor, binding

**6.1. The normal cycle.**
- The envelope is built in the transaction of the change. After the ack, the outbox row gets `acked_seq` and the record gets the flag **`server_known = 1`**. The same flag is set on a record received by pull. The outbox row is deleted when the cursor passes its `seq`; on pull such envelopes are skipped.
- An unsent outbox envelope may be deleted if newer ones cover all its groups.
- The cursor is `(generation, seq)`. `seq` is issued under an account lock (§10.2). A pull page and the cursor are saved in one transaction.
- Push responses are in the appendix, §A.3.

**6.2. The first binding of a database to an account** is a separate scenario. Before binding there is no `account_id` and no DEK, so the outbox is not written, and all records have `server_known = 0`.
1. Log in or register (§8), then a full pull from `seq = 0` and an ordinary merge. Inbox is merged by its constant id (§3.6).
2. For **every** local record that is newer than the server's in at least one group, or is absent from the server and has `server_known = 0`, an envelope is created **regardless of the HLC's age**. These are records that never left the device. Order: chats, messages, revisions, attachments (blobs before their envelopes, §7).
3. Local tombstones go out as `purged` with their `purged_of` snapshot, **parent first**: tombstones with `cascade_of` only after the parent's `ok(seq)`, as in the live path (§3.5, §3.6). If the server answers `stale_purge` to the parent — pull: the parent's new envelope lifts its tombstone together with the children's tombstones, and the children are recreated from `held` (§3.5) — their envelopes came through in the full pull of step 1. Otherwise a child's purge would delete an attachment or message of a record restored on another device.
4. Generation goes in batches with progress. A Telegram import is on the order of 100k envelopes. **Measured (S4a, 2026-11-05, `spikes/S4a-sync/README.md`):** a database of 100 000 entries (about 490 bytes of text each) gives 100 041 envelopes, one per record, of 1.1 KB of plaintext JSON on average, 233 MB on the wire after the header, the nonce, the tag and the padding to a multiple of 1 KiB (2.3 KB each, 4.2 times the text: an `upsert` is a snapshot of five groups, and padding rounds 1.1 KB up to 2 KiB). Building them from the database takes 0.23 s and the server taking them in order 0.07 s in the model (Apple M3 Pro, release, no encryption, no network, no SQLite); 101 requests of 1000 or 1001 of 100. The cost of binding is the bytes and the encryption, not the protocol.
5. **Binding is resumable.** The "bound" flag is set only after a complete pull. After a restart, envelopes are created for records with `server_known = 0` that have no outbox row yet: no duplicates and no gaps.
6. A `purged` that arrived in step 1 is handled by the rule of §3.5: if the record was edited on the database copy, it goes to quarantine.

**6.3. Full resync** — on a `generation` change or a "cursor is stale" response. **"Stale" (S4a, 2026-11-05) is not "older than 180 days":** the cursor is stale when, since the device last heard from the server, a compaction dropped something the device cannot do without — a `purged` envelope and the `upsert`s of the entity it purged — with a `seq` above the cursor. Every page carries the server's drop `epoch`; the pull request carries the epoch of the device's last page (`known_epoch`); the server remembers `(epoch, highest dropped seq)` for each such compaction and answers "stale" when one with a greater epoch has a `seq` above the cursor. A pull from `seq = 0` is never stale. **Generations and drops (S4a review, 2026-11-05, rounds 1 and 2):** a restore is a new branch of history, and `seq` and `epoch` mean something only inside one generation: a restored server issues `seq` again from the head of its backup and counts epochs from the backup's. So a drop is stored with the generation it was made in, `(generation, epoch, highest dropped seq)`, and a cursor `(generation, seq)` plus `known_epoch` is compared only with the drops of its own generation. The server judges a drop **missed** by a device when it was made in a generation *after* the cursor's, or in the cursor's generation with a greater epoch and a `seq` above the cursor; drops of earlier generations are never missed (the device pulled everything from 0 when it entered its generation). A device drops its old `known_epoch` when it starts a resync because the `generation` changed and takes the server's current `epoch` from the `GenerationChanged` answer: drops up to then are covered by the answer's `stale`, and a page of the resync with a greater epoch means a compaction dropped a `purged` while the device was pulling from 0 (a pull from 0 is never stale), so the cause becomes "both" and what the page no longer has goes to quarantine instead of being pushed back (found by the round 2 restore schedules). Comparing the two numbers across generations was the second blocker of the S4a review: an old cursor that had gone above the backup's head hid a low-`seq` drop of the new generation, and the purged record came back live. **How generations are named, and a restore into a clean installation (PRD §10.1):** a `generation` is an opaque random 64-bit identifier chosen by the server at restore time (if it is already in the lineage, the next one is taken). The server keeps the **lineage**: the list of its generations, oldest first, ending with the current one. The lineage, the drops with their generations and the epoch counter are all part of the dump, together with the log, so nothing lives outside the backup: a restore into the same installation and into a clean one read the same dump and produce the same state, plus a fresh identifier appended to the lineage. (A counter kept outside the backup, which round 1 of this revision relied on, has nowhere to live in a clean installation, and a number ordering generations would need exactly that.) A cursor whose generation is not in the lineage (it belonged to a branch that an earlier restore abandoned) is placed between the generation of the backup and the current one: it only misses the drops of the current generation, because a device on an abandoned branch had pulled from 0 when it entered the branch, so it already holds every drop that was in the backup its branch was cut from. A restore from a backup *newer* than that one (restores from an older backup, then from a newer one, with a device that lived on the first branch) can therefore push back a record the branch had seen alive but the newer backup had already purged and compacted: the device is told "not stale". This is a known limitation, pinned by a scenario (`tests/scenarios.rs`, `review_round_3_*`); the exact rule that closes it is in ADR-0017, for S4c. **Both causes at once:** a pull whose `generation` is old gets `GenerationChanged` with a `stale` flag computed by the rule above; the device does not guess from its clock (the clock rule quarantined everything a restore had taken from an idle device, Inbox included). **Quarantine after a restore that undoes a purge:** a record the user purged, which a restore brought back to the server, is, on a device that is stale, a visible state in "Found old entries" that the user decides about; slice 11 may offer "take the server's copy". A message in quarantine does not get a live `conflict` revision (a revision would be a child without a live parent); the version stays on the server and in the merged quarantined copy. Two simulated failures led here: the age of the last envelope the cursor has seen made an account that was idle for 181 days stale forever (the new cursor is just as old), and a resync cursor in the middle of its pages was called stale again (finding 1). Covered `upsert`s do not count; a later one says the same. The server's retention period (180 days) is now only the minimum age before a compaction may drop a `purged`. Everything is fetched from `seq = 0` and merged. If a local record is newer than the server's in any group, an envelope is created. What follows depends on the cause:

| The local record is not on the server, and…   | "cursor is stale"                                                                                                                                  | `generation` changed (the server was restored from a backup) |
|-----------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------|
| `server_known = 0` (it never left the device) | push at any age                                                                                                                                    | push                                                         |
| `server_known = 1` (the server knew it)       | **quarantine**: it was deleted and the tombstone has expired — this is exactly the protection against resurrection after 180+ days **[deviation]** | push: the server lost it in the restore                      |

- **Both causes at once** — the "cursor is stale" column applies: quarantine is safer. The server says whether the old cursor was stale (above).
- **A record in quarantine that the server sends again** (S4a review): an envelope for a record this device holds in quarantine is merged into the quarantined copy and stays there; it is not applied live, so a record is never in the feed and in quarantine at once. The user decides (slices 3 and 11). Tombstone rules (§3.5) run first.
- **"Is on the server"** also takes `pending_unknown` into account: a record whose envelopes arrived in an unknown `format_version` does not go to quarantine.
- **On a `generation` change:**
  - `acked_seq` is reset, and everything unconfirmed is sent again (the server is idempotent by `op_id`);
  - local tombstones are resent as `purged` in the order of §6.2 step 3 (parent first);
  - a purge made after the backup was taken, on devices without a tombstone, may bring records back to the trash — an honest limitation of restoring.

Compaction of the server log follows §10.2: only envelopes older than the tombstone retention period, covered in all groups.

### 7. Attachments

- **Key.** Each attachment has its own random `blob_key` (`crypto_secretstream_xchacha20poly1305_keygen`) in the encrypted `core` group. We do not use convergent encryption.
- **Format** (§10.2, layout and read rules — appendix §A.2): `sodium_pad` of the whole file to a multiple of 1024, then `secretstream` in 1 MiB chunks, the last one with `TAG_FINAL`. An end before `TAG_FINAL` and bytes after it are an error. A mismatch of `sha256` or `byte_size` with `core` is the `missing` state.
- **Re-upload after `missing`** (by a device that has the file) — under a **new** `remote_blob_id` and with a new `blob_key`: `core` is rewritten with a new HLC, and the old blob is deleted on the server after the ack (the request to delete a blob of one's own account — S4c). Under the old id the server would answer "already exists", and a damaged blob could not be replaced. With two concurrent re-uploads, the device whose `core` rewrite lost by HLC deletes its own blob (it knows the id): no orphans, no garbage collector needed.
- **Upload.**
  - The file is encrypted **once** into a temporary ciphertext that is kept until publication; resuming goes by offset in this file. If the file is lost, the upload starts over: the server allows overwriting an unfinished upload.
  - After publication the blob is immutable. Publishing the same `remote_blob_id` again gets the "already exists" response and counts as success — the bytes are not compared, because the stream header is random.
  - The attachment's envelope goes out after the blob is published and does not hold back other envelopes.
  - The blob store is keyed by the pair `(account_id, remote_blob_id)`: a `purged` with someone else's id affects only its own account.
  - **One blob — one attachment.** An import "as a copy" (PRD §16) generates `remote_blob_id` and `blob_key` anew; otherwise a purge of the copy would delete the original's blob.
- **Revisions do not reference attachments.** Attachments are not versioned. A purge of an attachment deletes the blob, and no revision can reference it (decision 9). Delaying blob deletion until server backups rotate (§10.1) — S4c.
- **`remote_blob_id`** is a UUID v7 (`Uuid::now_v7()`, crate `uuid`, feature `v7`), created together with `blob_key` when the attachment is created, in the same transaction: the attachment's envelope carries a full snapshot of `core` (§6.1). It is stored in the encrypted `core` group (decision 3). The time inside v7 matches the HLC of the `core` group, which the server sees anyway. The HMAC of a hash from the PRD is not used: without deduplication determinism gives nothing, integrity is checked by `sha256`, and a random id makes it possible to replace a damaged blob (above).

### 8. Keys

**8.1. Hierarchy.** The byte layouts of the wraps and the AD are in the appendix, §A.1.

```text
password ─crypto_pwhash(ARGON2ID13, salt 16, KDF profile, out 32)─► pw_root
pw_root ─crypto_kdf_derive_from_key(32, id, "kinigePW")─► 1: auth_key (to the server) │ 2: KEK_pw
24 words ─BIP-39: checksum verification → entropy[32] (NOT seed/PBKDF2)─► rc_root
rc_root ─crypto_kdf_derive_from_key(32, id, "kinigeRC")─► 1: rc_auth_key (to the server) │ 2: KEK_rc
DEK[key_id] (32, randombytes_buf) ─crypto_kdf_derive_from_key(32, id, "kinigeDK")─► 1: K_env │ 2: reserved │ 3: K_rev
wrap_pw = AEAD(KEK_pw, DEK)   wrap_rc = AEAD(KEK_rc, DEK)   — XChaCha20-Poly1305-IETF, AD per §A.1
```

- **Words.** The input is normalized: ASCII lowercase, single spaces. Then `bip39::Mnemonic::parse_in_normalized(Language::English, …)` and `to_entropy()`; the methods exist in docs.rs `bip39` 3.0.0. `to_seed` is not used.
- **`K_rev` depends on `key_id`.** DEK rotation (P2) will change the ids of `conflict` revisions; this is settled in its ADR.

**8.2. Argon2id profiles** are a closed set, protocol constants (table — appendix §D): profile 1 = `MODERATE` (3 / 256 MiB, about 0.7 s on a 2.8 GHz Core i7 according to the documentation), profile 2 = `SENSITIVE` (4 / 1 GiB).

- The server sends the profile number. An unknown number is a refusal: this is the upper memory bound.
- The lower bound is profile 1. It can be lowered only by a new client version.
- **An account's profile only grows.** On a password change and on recovery the client takes `max(kdf_profile_seen, the one sent by the server, the default)` and does not offer a downgrade; after its own change, `kdf_profile_seen :=` the chosen profile. The sent profile comes from an untrusted party, so `max(sent, default)` alone is not enough: a recovery on a device that has seen profile 2, with 1 sent, would downgrade the account and force the other devices to recover too. There is no legitimate downgrade.
- If a higher profile has already been seen on the device and the server sends a lower one, the client **refuses**, does not derive the key, and explains why. The way out is recovery with the words (§8.6): it does not depend on Argon2. The option "confirmation instead of refusal" was rejected (decision 8): confirmations get clicked through, and a malicious server gets an `auth_key` that is cheap to brute-force.
- libsodium always computes with one lane (verified in `pwhash_argon2id.c`).

**8.3. Registration.** The client creates `account_id`, `salt`, `DEK[1]` and 24 words, derives the keys and sends the login, `account_id`, `salt`, profile, `auth_key`, `rc_auth_key`, `wrap_pw`, `wrap_rc`.

The server stores `crypto_generichash(auth_key)` and `crypto_generichash(rc_auth_key)` (32 bytes each) and compares them with `sodium_memcmp`. A fast hash is enough: the input is 256 uniform bits, not a password. The server does not receive the password or the KEK (§10.3).
**The login** is ASCII only, `[a-z0-9._@+-]`, lowercased and trimmed. At registration the words are shown the same way as on recovery (§8.6 item 3): a "Save" button, no typed-back check; the account is created on the server only after saving. The registration request is **idempotent by `account_id`** and is repeated with the same prepared package until there is a definite answer; if the login is taken, the words are not regenerated (appendix §A.5). Onboarding — Q12.

**8.4. Login and a new device** (MVP+ — by password only):
1. The client requests the salt and profile by login. For an unknown login: the salt is `crypto_generichash(outlen 16, key = server_secret (32 bytes), login)`, the profile is the default (1). Limitation: an account with profile 2 is distinguishable from a non-existent one — accepted.
2. The client sends `auth_key`.
3. The server issues a session token (32 random bytes, it stores the hash), `account_id` (it is needed in `AD_wrap`) and `wrap_pw`.
4. The client unwraps the DEK and performs the first binding (§6.2).

The rate of login attempts and of step 1 of recovery (§8.6) is limited per account and per IP.

**8.5. Password change** — with the session token and the `auth_key` of the old password. The new `salt`, profile, `auth_key` and `wrap_pw` are replaced atomically. Data is not re-encrypted (§10.3). The option "log out on other devices" is for the case of a leaked password.

**8.6. Recovery** is one atomic operation, authorized by `rc_auth_key`, without the old password:
1. Client: words → `rc_root` → `rc_auth_key` → the server returns `wrap_rc` (without a session) → DEK. The old `rc_auth_key` stays in memory until step 4.
2. The client creates a new password (new `salt`, `auth_key`, `wrap_pw`) **and new 24 words** (new `rc_auth_key`, `wrap_rc`).
3. **The new words are shown with a "Save" button** (PDF or print, like 1Password's Emergency Kit). There is no typed-back check (decision 7). One cannot proceed until "Save" is pressed. A large warning: without the words and the password, access to the entries on the server is lost forever, and nobody can restore it, including the developers. The replacement is requested only after saving: if the client crashes earlier, nothing has changed on the server and the old words still work.
4. The server replaces everything in one transaction and revokes the other sessions (decision 7). The request is **idempotent by the new `rc_auth_key`**: repeating the same package after a lost response is a success. If the outcome is uncertain after a restart, the client tries to log in with the new password: success means the replacement went through (appendix §A.5).

After this the old words are invalid. The responses "no such login" and "wrong key" are indistinguishable.
**Reissuing the words** is a separate operation with the session and the password: new `rc_auth_key` and `wrap_rc`; showing and saving as in item 3.
**Honestly:** if the old words or the password were exposed, the DEK may have been unwrapped, and neither a password change nor new words undo that. Only DEK rotation (P2) fixes it.

**8.7. Revoking a device** deletes the session token. A copied DEK cannot be taken back (§10.3); DEK rotation is P2, and `key_id` is reserved for it.

**8.8. Storage on the device.**
- The DEK and the token are in the OS secret store. On Linux without Secret Service — a file with mode 0600 and a warning (as in §12.3). The database is open anyway (§4.1), so protection is not weakened.
- The password, `pw_root`, `rc_root` and the KEK are zeroed after use (`sodium_memzero`); on recovery "after use" means after step 4 of §8.6.
- **Logs:** the request bodies of registration, login, recovery and password change are logged neither by the server nor by the client; tokens only as hashes. HLCs and ids in logs are allowed; content and keys are not.

**8.9. Account deletion.**
- The server deletes the account, the wraps, the hashes, the sessions, the envelopes and the blobs: the data must be deleted, not just the keys.
- The promise to the user is verbatim §10.3: "backups are kept for N days and then rotated; until rotation, the data in them is protected by your password". The owner of a backup can guess the password offline at the cost of the Argon2id profile.
- N — Q14. Crypto-shredding of backups — P2. An account that comes back when an old backup is restored — operations, S4c.
- Local backups, the Markdown copy and exports are not affected.

### 9. Authentication separate from encryption

| Secret                                          | Server sees it                | What it gives the server                                    |
|-------------------------------------------------|-------------------------------|-------------------------------------------------------------|
| password, `pw_root`, `rc_root`, KEK, DEK, `K_*` | no                            | —                                                           |
| `auth_key`, `rc_auth_key`                       | yes (over TLS), stores a hash | logging in and getting its own wrap                         |
| `wrap_pw`, `wrap_rc`, salt, profile             | yes                           | offline password guessing — unavoidable with password login |
| session token                                   | a hash                        | push and pull for its own account                           |

**A leak of the server database** by itself allows neither logging in nor reading: it holds hashes and ciphertext. But after guessing a weak password offline, an attacker gets both login and the DEK. That is why password requirements are part of Q12.
Code substitution is a risk of the web client; the desktop app is covered by update signing (§10.3, §12.3).

### 10. What the server sees and what it can do (refining §10.3)

**It sees:**
- the account, the login; devices (`device_id` in the HLC, sessions, IP, time);
- the type and id of records, `op_id`; **which group changed and when**, and the groups' `sv` — open HLCs are needed for compaction;
- the fact of a deletion, a restore and a purge; the fact that a record is Inbox;
- sizes to within 1 KB; sizes and upload times of blobs;
- **links by time and batches:** "attachment ↔ blob" (the envelope goes out right after the blob is published, and on purge the link is explicit), "message ↔ children" and "chat ↔ messages" (the `purged` cascade right after `purged(chat)`).

**It does not see:** text, titles, `journal_date`, the value of the `created_at` field, tags, flag values, file contents. The record's creation time is still visible through the HLC of the `core` group (except for imports and a manual edit of the time).

**It can** (there is no protection, see "Not settled"): hide envelopes, serve them in a different order, replay old authentic envelopes, show different devices a different history. Replays are harmless because merge is idempotent, but freshness and completeness are not guaranteed. The client checks the server's purge rules itself (§3.5).
**It cannot:** forge an envelope, change an open field of it, or bind it to another record or account — AEAD prevents that.

### 11. Primitives and implementation

libsodium only: `crypto_aead_xchacha20poly1305_ietf_*`, `crypto_secretstream_xchacha20poly1305_*`, `crypto_pwhash` (Argon2id13), `crypto_kdf_derive_from_key`, `crypto_generichash`, `sodium_pad`/`sodium_unpad`, `randombytes_buf`, `sodium_memcmp`, `sodium_memzero`. The only things of our own here are the byte format and the HLC; there is no cryptography of our own. Constants verified against the headers are in the appendix, §D.
The Rust binding is `libsodium-rs`; the fallback is `libsodium-sys-stable` with a thin wrapper (decision 1). Test vectors and the test plan are in the appendix, §B.

## Consequences for the §4.2 schema

The full list is in the appendix, §C. The main points:
- every synchronized table has `hlc_json` with `sv` per group, `server_known` and `groups_raw` (raw group bytes; replaces `unknown_json`);
- new tables `tombstone`, `quarantine`, `pending_unknown`; a local index `message_ext`;
- `change_outbox.acked_seq`;
- the fields `blob_key`, `base`, `supersedes`;
- `message_revision` has `hlc_json` and `schema_version`.

All of this is needed from the very first write of slice 1.

## User decisions (21.10.2026)

The PRD was brought in line in v0.8.

1. **libsodium binding for Rust** — `libsodium-rs` (wrappers by the libsodium author on top of `libsodium-sys-stable` 1.24.3, 0.3.0 of 2026-10-14); the fallback is `libsodium-sys-stable` directly with a thin wrapper. RustCrypto was rejected: STREAM instead of `secretstream` and HKDF instead of `crypto_kdf` — a PRD change with no benefit. Check the build on three OSes at the start of S4b. The web client will use libsodium.js (`libsodium-sys-stable` has WASM only via WASI) and will be checked against the test vectors.
2. **Inbox is one constant**, including `core`; it is identified by id (§3.6).
3. **`remote_blob_id` is a UUID v7** in the encrypted `core` (§7). The contradiction in the PRD ("HMAC of a hash", "no deduplication", "deleted with the attachment") is resolved.
4. **Chat deletion** (§3.6): `deleted` only on the chat, the messages are hidden together with it; the purge is done by one device, first `purged(chat)`, after its ack — `purged` of the messages and children; late messages go to Inbox as derived state; recipients do not cascade.
   - **Export** (PRD §16): the full export (`data/*.jsonl`) includes records from the trash with `deleted_at`, otherwise a migration loses the 30 days for restoring; the readable Markdown of the export and the Markdown copy exclude the trash; purged records are nowhere.
   - A key per chat — P2 ("Not settled").
5. **Server checks by the header** — ordering purges and refusing when `wall_ms > server_now + SKEW_MAX` (24 h). This is not protection against a malicious server: the client applies the same rules itself (§3.5) and leaves `clock_skew` (§2). The limit applies only to the HLC, not to `journal_date` and not to `created_at` (§2).
6. **Recovery words** — the English BIP-39 list, entropy, not the seed. The UI language does not affect the list.
7. **Recovery** (§8.6) revokes the other sessions and reissues the words. The words (at registration, recovery and reissue) are shown with a "Save" button (PDF / print) and a warning about losing access; **there is no typed-back check**; the new words go to the server only after saving. On a password change, "log out on other devices" is an option.
8. **Argon2id** — profiles 1 and 2, the lower bound is profile 1, **an account's profile only grows** (§8.2); a downgrade is a refusal with an explanation, and the way out is recovery with the words. Measure in S4b on the weakest machine: if more than 2 s, discuss. The WASM measurement — before the web client.
9. **Narrowing revisions [deviation from §10.2 and §4.2]**, accepted:
   - a `conflict` revision stores only the losing group (`body` or `placement`) with its `sv` and raw bytes — otherwise the content of a revision with the same id would differ between devices;
   - a `placement` conflict (a move into different chats or to different dates) gives a revision and the "there is another version" mark, like text (§3.4); `flags` — LWW without a trace;
   - revisions do not reference attachments, so "the blob is deleted taking revisions into account" degenerates.
10. **The quarantine criterion on full resync [deviation from §10.2]** — by `server_known` and the cause (the §6.3 table), not "HLC older than the horizon": the literal PRD rule sent the whole history to quarantine on the first binding and missed the resurrection of an edited record. In the UI, quarantine is «Найденные старые записи» ("Found old entries"): the user decides whether to bring them back or delete them (slices 3 and 11).

11. **Our own protocol rather than a ready-made engine** — per the research `docs/plan/research/sync-build-vs-buy.md`: no candidate gives E2EE, our own server, Rust and version compatibility at the same time; a ready-made engine would cover only the merge (S4a), while keys, recovery, the server, purge and quarantine remain ours with any choice. Reference implementations to compare against during implementation: Actual Budget and Notesnook.
12. **If S4a goes over twice its budget (10 days)** — stop and revise the schedule (§12 item 7); do not simplify the protocol: LWW of a whole record instead of groups loses concurrent edits of different fields.
13. **Metadata:** the server sees when and how much is written (the HLC in the header, size to within 1 KB), but not the content (§10) — acceptable for a journal. Hiding metadata (noise, scheduled batches) is not planned.
14. **A reminder about a long-ago sync** (slice 11): the status shows "last sync N days ago"; if there are unsent edits and there has been no sync for more than 7 days, the client honestly warns that concurrent edits on other devices will produce conflicting versions. No data is lost in that case (§3.4, §3.5).

Also: **`origin`** in a message's `core` — who created the record: `user` (the UI and quick capture), `agent` (MCP, PRD §9.3), `import_telegram` (import). Immutable; a new value is additive, without an `sv` bump (§4); the client shows an unknown value as "other". Needed by PRD §9.3 ("added by an agent" in the feed) and by the import.

**The external import id** (PRD §2.1, Telegram deduplication) does not go through the record id. The record id is an ordinary random v4; the external reference is an optional immutable field `ext` in a message's `core`: `{"src":"telegram","chat":"<chat id>","msg":"<message id>"}`, values as strings (Telegram ids are 64-bit, while canonical JSON is limited to ±2⁵³); `origin = 'import_telegram'` ⇔ `ext.src = 'telegram'`. Deduplication uses a local non-synchronized index `message_ext(src, ext_chat, ext_msg) → message_id`, derived from `core` and recomputed on pull, so it also works on a second device. A match on re-import means the record is skipped (the user's edits matter more than a re-export), and the report says "skipped N". A key from a hash of the external id does not work: at import time (slice 4) there is no DEK yet, and a local key is not synchronized. Limitation: importing the same export on two devices at the same time before they sync gives duplicates — a warning in the UI. The pair `(chat, msg)` is needed because Telegram message ids are, from memory, not global (not verified on a real `result.json`).

## Not settled in this ADR

- Onboarding and password requirements — Q12, S4b. The HTTP API (except the codes in the appendix), transport, rate limits, credential versioning and compare-and-swap on password change — S4c.
- DEK rotation (and revision ids after it), key transfer via QR, a key per chat, SQLCipher, crypto-shredding of backups — P2; the period N — Q14.
  - A key per chat is not for now: an open `key_id` per chat would reveal to the server the "message ↔ chat" link, which the encrypted `placement` currently hides. Turning it on later means re-encrypting that chat's records in the background; the groundwork is the `key_id` in the envelope.
- Protection against hiding, reordering and rollback (hash chains, signed device sequence numbers) — not planned.
- Storage and the crypto library of the web client — before the web client. Settings sync; embeddings are not synchronized in P1 (§9.2).
- The UI for conflicts, quarantine, and the "there is another version" mark — slices 3 and 11. Attachment deduplication and a garbage collector (§4.3).

## Not verified

- `libsodium-rs` 0.3.0: the names of the modules and functions `utils::{pad, unpad, memcmp, memzero}` are verified; signatures, wrapper quality and audit are not. The libsodium version inside `libsodium-sys-stable` 1.24.3 (the `stable` branch is 1.0.22) and the CI build on three OSes are not.
- Byte compatibility with libsodium.js — the S4b test vectors will show. The time of profile 1 on target machines and in WASM has not been measured.
- `synchronizable` for a secret-store item in the chosen crate — S4a. The account lock for `seq` in Postgres — S4c.
- Convergence as a whole — only the property tests of S4a and S4b (appendix, §B).

## Sources

The full list with links is in the appendix, §E: libsodium documentation, headers and sources (`stable`, 1.0.22), docs.rs and crates.io, RFC 9106, RFC 8785, BIP-39, Kulkarni et al. 2014.
