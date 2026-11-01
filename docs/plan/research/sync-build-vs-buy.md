# Sync: build our own or buy off the shelf

Date: 2026-10-21. Question: build our own protocol from ADR-0001 (HLC, LWW field groups, conflict revisions, tombstones, quarantine, XChaCha20-Poly1305 envelopes on libsodium) or take a ready-made engine.
Normative: §4, §10, §12.3 and §15 of the PRD (`docs/plan/02-prd.md`). ADR-0001 was read in the branch `adr/0001-protocol-keys`, revision 3.
Previous comparison: `research/tauri-stack.md` §7 and `01-review.md`; back then the decision was "our own op-log, don't take ready-made engines". This document re-checks that decision, with E2EE at the center and on fresh data.

**How this was checked.** Dates, versions and licenses come from GitHub (API, `releases.atom`, LICENSE), crates.io, npm and official documentation as of 2026-10-21. Every claim has a link. I re-checked the key facts myself: the status of cr-sqlite (issue #444), the schema rule in SQLite Sync, `%ELO` in the Loro Protocol README. What I could not check is marked "not verified".

## In short

- **There is no ready-made engine that covers §10 of the PRD.** Every candidate falls into one of three groups:
  - the server reads the data: PowerSync, ElectricSQL, Turso, Ditto, Zero, Jazz 2;
  - E2EE exists, but only in JS or Go and with its own data model: Evolu, Jazz Classic, any-sync;
  - E2EE is marked experimental or pre-alpha: Loro `%ELO`, Keyhive/Subduction for Automerge.
- **Ready-made solutions cover the cheapest part of ADR-0001 and leave the most dangerous part to us.** The cheapest part is merging by HLC. The most dangerous: keys, recovery words, the server, purge and the tombstone horizon, generations after a backup, version compatibility. Even the best candidates say outright that keys are "on the application": Loro, "Keys and key agreement are application‑provided and out of scope"; secsync, key exchange out of scope.
- **Recommendation: our own, as in ADR-0001, without someone else's engine.** It is worth checking against existing open implementations of the same scheme (Actual Budget, Notesnook, Loro Protocol, secsync); details in the "Recommendation" section. Loro can be plugged in inside the `body` envelope later, if automatic text merging is ever needed.

## What the solution must provide (from the PRD)

| #  | Requirement                                                                                                                  | Where in the PRD |
|----|------------------------------------------------------------------------------------------------------------------------------|------------------|
| R1 | The server does not see text, titles, `journal_date`, tags, attachments. It may see change times and sizes; this is accepted | §10.1, §10.3     |
| R2 | Immutable envelopes, push is idempotent by `op_id`, AEAD covers the HLC, the purge flag and the type                         | §10.2, §10.3     |
| R3 | Rust owns SQLite (single writer), FTS5 stays, the schema is ours                                                             | §3.2, §4.2       |
| R4 | Trash for 30 days, a restore beats an older delete, `purged`, tombstones for 180 days, quarantine without resurrection       | §10.2            |
| R5 | Old and new clients work together, unknown fields are preserved                                                              | §10.4            |
| R6 | Own server: `docker compose`, backup and restore with a "generation" and a full resync                                       | §10.1            |
| R7 | Password separate from the KEK, 24 words, password change without re-encryption                                              | §10.3            |
| R8 | First device link after a Telegram import: on the order of 100k entries                                                      | §2.1, ADR §6.2   |
| R9 | Future web client (WASM, libsodium.js) and mobile client                                                                     | §2.3, §2.4, §3.2 |

## Candidates

### 1. Engines on top of SQLite

**cr-sqlite (vlcn-io)**: a CRDT extension for SQLite.
- Last release: v0.16.3 of 2024-01-17. Commits from 2026-08 are only build fixes by third parties. MIT. [releases](https://api.github.com/repos/vlcn-io/cr-sqlite/releases?per_page=3)
- The author on 2024-12-30: "I'm full time on https://zero.rocicorp.dev/ these days", and also "at least 1-2 years" without time for the project. [issue #444](https://github.com/vlcn-io/cr-sqlite/issues/444)
- How it works:
  - LWW per column; changes are read and written through `crsql_changes`;
  - no checked FKs, and no UNIQUE other than the PK [constraints](https://vlcn.io/docs/cr-sqlite/constraints);
  - inserting into a CRR is 2.5 times slower [README](https://github.com/vlcn-io/cr-sqlite).
- No networking. Changes can be encrypted as a batch and sent through our own server; this is an inference from the API, there is no example.
- **Verdict:** the project is effectively abandoned; don't take it.

**SQLite Sync (sqliteai/sqlite-sync)**: a living CRDT extension in C.
- v1.2.0 of 2026-09-28, commits on 2026-10-20, Apache-2.0. [releases](https://api.github.com/repos/sqliteai/sqlite-sync/releases?per_page=3), [LICENSE](https://raw.githubusercontent.com/sqliteai/sqlite-sync/main/LICENSE)
- There are builds for macOS, Windows, Linux, iOS, Android and WASM; it loads as a loadable extension. [README](https://github.com/sqliteai/sqlite-sync)
- Conflict model:
  - for tables: Causal-Length Set (default), Delete-Wins, Add-Wins;
  - for text: "Block-Level LWW", the lines of one note merge independently. [README](https://github.com/sqliteai/sqlite-sync)
- There are `cloudsync_payload_encode` and `cloudsync_payload_apply`: changes can be exported as a BLOB and applied. [API.md](https://github.com/sqliteai/sqlite-sync/blob/main/API.md) Encrypting the BLOB and sending it through our own server is my inference; the documentation has no example.
- The documentation does not mention E2EE.
- The server side is the CloudSync service. Even with "your own" Postgres it has to be registered in the `dashboard.sqlitecloud.io` dashboard. [postgres quickstart](https://github.com/sqliteai/sqlite-sync/blob/main/docs/postgresql/quickstarts/postgres.md) No standalone sync service was found (not verified that none exists).
- **Blocker for R5:** "sqlite-sync computes a schema hash… The server rejects payloads whose schema hash it does not recognize". All participants "must have the same set of synced tables with identical structure". [schema.md](https://github.com/sqliteai/sqlite-sync/blob/main/docs/schema.md) So while one device has updated and another has not, they do not sync.
- Tombstone garbage collection not verified.

**Evolu (evoluhq/evolu)**: closest to us in spirit.
- `@evolu/common` 8.23.2 of 2026-10-17, MIT, very active. [releases](https://api.github.com/repos/evoluhq/evolu/releases?per_page=3)
- E2EE by default, XChaCha20-Poly1305. The relay sees the OwnerId, **change times in plaintext**, the NodeId, the approximate size and the IP. [privacy](https://www.evolu.dev/docs/privacy) For R1 this is roughly the same as what our server sees under ADR-0001 §10: the HLCs are open there too.
- Blockers:
  - **no Rust**, only TypeScript packages [packages](https://github.com/evoluhq/evolu/tree/main/packages); the database would live in the webview, not in Rust (R3);
  - its own schema, in TypeScript, with system columns [schema](https://www.evolu.dev/docs/schema); FTS5 and a custom SQL schema are not described;
  - schema evolution is additive only; nobody negotiates protocol versions ("nothing negotiates a version") [Protocol](https://www.evolu.dev/docs/api-reference/common/local-first/Protocol);
  - the relay is on Node.js, not axum.
- **Verdict:** a good protocol reference, but not suitable as a dependency.

**PowerSync.**
- Rust SDK 0.1.1 is "Experimental". `tauri-plugin-powersync` is in alpha ("Expect breaking changes"). [crates.io](https://crates.io/crates/powersync), [tauri](https://docs.powersync.com/client-sdks/reference/tauri.md)
- The service is under FSL-1.1-ALv2. [LICENSE](https://raw.githubusercontent.com/powersync-ja/powersync-service/main/LICENSE)
- The server is the source of truth and runs SQL over Postgres rows. [consistency](https://docs.powersync.com/architecture/consistency.md)
- E2EE is possible only as an "opaque ciphertext column", and then ids and filter fields are open. [blog](https://www.powersync.com/blog/building-an-e2ee-chat-app-with-powersync-supabase)
- **Verdict:** not suitable: an extra service, and the server sees the data structure.

**ElectricSQL.**
- A "read-path sync engine for Postgres"; it does not sync writes. [writes](https://electric.ax/docs/guides/writes)
- The server is in Elixir, there is no SQLite client. The company is moving into an "agent platform". [electric.ax](https://electric.ax/)
- **Verdict:** not suitable.

**Turso / libSQL.**
- Turso Sync is in beta. Conflicts: "last push wins" at the row level; the server serves physical database pages, that is, it reads the database. [conflict-resolution](https://docs.turso.tech/sync/conflict-resolution.md)
- A self-run sync server is described for development and testing. [local-sync-server](https://docs.turso.tech/sync/local-sync-server.md)
- The Turso engine does not support FTS5 or loadable extensions. [COMPAT.md](https://github.com/tursodatabase/turso/blob/main/COMPAT.md)
- **Verdict:** incompatible with E2EE in principle.

### 2. CRDT libraries

**Automerge + Keyhive/Subduction.**
- The Rust core is alive: `automerge` 0.12.0 of 2026-09-16, MIT. [crates.io](https://crates.io/crates/automerge)
- E2EE layer:
  - Keyhive: "pre-alpha", "DO NOT use this release in production applications", no audit [keyhive](https://github.com/inkandswitch/keyhive);
  - Subduction, the successor of Beelay, likewise: "DO NOT use for production use cases at this time" [subduction](https://github.com/inkandswitch/subduction).
- A document keeps its entire history, history trimming is "not prioritised"; schema migrations are manual. [modeling-data](https://automerge.org/docs/cookbook/modeling-data/)
- **Verdict:** E2EE is not ready; keep watching the project.

**Yjs / Yrs + secsync.**
- `yrs` 0.28.0 (pre-1.0), MIT, native Rust. [y-crdt](https://github.com/y-crdt/y-crdt)
- Tombstones of deleted structures are not collected. [yjs README](https://github.com/yjs/yjs#yjs-crdt-algorithm)
- Ready-made transport options:
  - secsync (E2EE transport): "beta", TypeScript, last commit 2024-09-21; the project is dormant. The server stores encrypted snapshots and updates, clients do the merging. [secsync](https://github.com/nikgraf/secsync)
  - y-sweet reads documents on the server, so it is incompatible with E2EE. [y-sweet](https://github.com/jamsocket/y-sweet)
- **Verdict:** usable only as a text-merging library; transport and keys are ours.

**Loro + Loro Protocol.**
- `loro` 1.16.2, MIT, Rust + WASM + Swift. The format has been stable since 1.0. [loro](https://github.com/loro-dev/loro), [v1.0](https://loro.dev/blog/v1.0)
- Shallow snapshots trim history. The price: peers sync only if they have versions after the trim point. [shallow snapshots](https://loro.dev/docs/concepts/shallow_snapshots)
- Loro Protocol, `%ELO` mode:
  - E2EE on AES-GCM; "The server never decrypts; it indexes plaintext headers only to support backfill and routing";
  - "Keys and key agreement are application‑provided and out of scope";
  - there is a minimal Rust server, `loro-websocket-server`. [protocol](https://github.com/loro-dev/protocol)
- The blog calls `%ELO` experimental, with a changing format. This is per the research agent; I did not open the page myself (403), and the README has no such note. [blog](https://loro.dev/blog/loro-protocol)
- **Verdict:** the best candidate if merging text inside a message is ever needed. Transport and keys are still ours.

### 3. Ready-made E2EE systems

**Anytype any-sync.**
- v0.13.8 of 2026-10-07, MIT, Go only. [any-sync](https://github.com/anyproto/any-sync)
- Changes are a signed, encrypted graph (DAG). The application builds its own state itself: "Applications using any-sync can utilize the objects received from it to construct their own internal state". [overview](https://raw.githubusercontent.com/anyproto/tech-docs/main/any-sync/overview.md)
- Self-host:
  - components: coordinator, 3 sync nodes, a file node, a consensus node, MongoDB, Redis and Garage, about 1 GB of RAM;
  - README: "designed for personal use, review, or testing purposes" [dockercompose](https://github.com/anyproto/any-sync-dockercompose).
- The reference middleware is distributed under a source-available license, not MIT. [LICENSE](https://raw.githubusercontent.com/anyproto/anytype-heart/main/LICENSE.md)
- **Verdict:** heavy, a Go sidecar, and the state model is still our own.

**Jazz.**
- Jazz 2.0 is alpha. The server checks permissions row by row and filters reads, that is, it sees the data; the word "encrypt" does not appear in the documentation. The Rust client stores data in its own RocksDB, not in our SQLite. [permissions](https://jazz.tools/docs/auth/permissions), [jazz](https://github.com/garden-co/jazz)
- Jazz Classic with E2EE is a JS stack, and it is no longer the main line anyway. [classic](https://classic.jazz.tools/llms-full.txt)
- **Verdict:** not suitable.

**Ditto.**
- The Rust SDK is proprietary, Linux and macOS ARM64 only. [crates.io](https://crates.io/crates/dittolive-ditto), [compatibility](https://docs.ditto.live/sdk/latest/compatibility/rust.md)
- The cloud is a "trusted peer". [cloud](https://docs.ditto.live/cloud/overview.md)
- Self-managed is Enterprise only.
- **Verdict:** not suitable.

**Replicache / Zero.**
- Replicache is in maintenance mode, the repository is archived. [replicache](https://github.com/rocicorp/replicache)
- Zero: "Zero is not local-first… authoritative server", "doesn't support offline writes". [when-to-use](https://zero.rocicorp.dev/docs/when-to-use.md)
- **Verdict:** not suitable.

**Triplit.**
- Last release: 2025-07-31; the team moved to Supabase. [supabase](https://supabase.com/blog/triplit-joins-supabase)
- Permissions are checked by the server, so it sees the data.
- **Verdict:** not suitable.

### 4. Without our own sync server

**Syncthing.**
- v2.1.6, MPL-2.0. [syncthing](https://github.com/syncthing/syncthing)
- Encrypted folders for untrusted devices: "should still be considered beta / testing only". [untrusted](https://raw.githubusercontent.com/syncthing/docs/main/users/untrusted.rst)
- Conflicts are resolved at the file level: a `.sync-conflict-…` copy appears. [syncing](https://raw.githubusercontent.com/syncthing/docs/main/users/syncing.rst)
- An open SQLite database cannot be synced, according to the maintainers on the [forum](https://forum.syncthing.net/t/bahaviour-of-st-on-sqlite-files-in-various-scenarios/5921) and [sqlite.org/howtocorrupt](https://www.sqlite.org/howtocorrupt.html).

**iCloud Drive.**
- Apple: "a SQLite database's store file must never be stored in iCloud". [iCloud Design Guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/iCloudDesignGuide/Chapters/iCloudFundametals.html)
- "Optimize Storage" can remove a file from disk until it is downloaded again. [Apple support](https://support.apple.com/guide/mac-help/optimize-storage-space-sysp4ee93ca4/mac)
- Whether an ad-hoc-signed Tauri app can get iCloud entitlements: not verified.

**S3 / WebDAV.**
- No ready-made Rust crate for "an encrypted append-only log on top of object storage" was found.
- There is a building block: Apache OpenDAL, a single API for S3, WebDAV and others. [opendal](https://github.com/apache/opendal)

**Section verdict.** This is only a transport for immutable encrypted log files, as in `advice-fable.md` item 1. Merging, quarantine, keys and purge stay the same as in ADR-0001. Part of S4c is saved; lost are Kinige Cloud (§2.6) and an easy path for the web client: a browser cannot read a Syncthing folder.

### 5. Existing apps with the same scheme: references, not dependencies

**Actual Budget**
- v26.10.0, MIT, alive. [actual](https://github.com/actualbudget/actual)
- Scheme:
  - HLC and one message per cell (LWW per column);
  - envelope `{timestamp,isEncrypted,content}` [sync.proto](https://raw.githubusercontent.com/actualbudget/actual/master/packages/crdt/src/proto/sync.proto);
  - the server stores `messages_binary(timestamp, is_encrypted, content)` and finds divergence between replicas with a Merkle tree [sync-simple.js](https://raw.githubusercontent.com/actualbudget/actual/master/packages/sync-server/src/sync-simple.js).
- Garbage collection is "reset sync": the history is compacted, the other devices are rolled back. [sync docs](https://raw.githubusercontent.com/actualbudget/docs/master/docs/getting-started/sync.md)
- In the desktop build the key is derived with PBKDF2 at 10,000 iterations, which is weak. [encryption-internals](https://raw.githubusercontent.com/actualbudget/actual/master/packages/loot-core/src/server/encryption/encryption-internals.electron.ts)
- This is the same architecture as ADR-0001, only simpler and weaker on keys. Our plan is not exotic: such a scheme has been running for years in a living project.

**Notesnook**
- Each item is encrypted separately, XChaCha20-Poly1305 from libsodium, on all platforms. The server stores `{alg, cipher, iv, salt, id}` "without further processing". [help](https://notesnook.com/help/how-is-my-data-encrypted)
- Edits to the same note made a minute or more apart are a manual conflict. [merge conflicts](https://notesnook.com/help/faqs/what-are-merge-conflicts)
- The server is beta, AGPL-3.0, .NET. [sync-server](https://github.com/streetwriters/notesnook-sync-server)
- Confirms our choice of primitive and the "server is a dumb envelope store" model.

**Standard Notes**
- Protocol 004: Argon2id + XChaCha20-Poly1305; the associated data contains the item UUID. [encryption](https://standardnotes.com/help/security/encryption)
- Since 2024 the project has been owned by Proton.

## Summary table

| Candidate            | Alive (2026-10)  | E2EE out of the box                      | Self-host                        | Rust owns SQLite + FTS5            | Version compatibility (R5)                 | Verdict               |
|----------------------|------------------|------------------------------------------|----------------------------------|------------------------------------|--------------------------------------------|-----------------------|
| cr-sqlite            | no (author left) | no, can be layered on top                | no server                        | yes (extension)                    | not described                              | no                    |
| SQLite Sync          | yes, 1.2.0       | no, can be layered on top (not verified) | no: SQLite Cloud service         | yes (extension)                    | **no**: everyone must have the same schema | no                    |
| Evolu                | yes, 8.x         | yes                                      | yes, Node                        | **no**: TS only                    | additive only                              | reference             |
| PowerSync            | yes              | no                                       | yes, but TS + a Postgres source  | Rust SDK experimental, Tauri alpha | not verified                               | no                    |
| ElectricSQL          | changing course  | no                                       | Elixir                           | no                                 | —                                          | no                    |
| Turso Sync           | beta             | impossible                               | for tests                        | no FTS5                            | —                                          | no                    |
| Automerge + Keyhive  | yes / pre-alpha  | pre-alpha                                | Subduction, "not for production" | Rust core                          | manual                                     | watch                 |
| Yrs + secsync        | yes / dormant    | secsync: TS, beta                        | no                               | Yrs in Rust                        | —                                          | merge only            |
| Loro + `%ELO`        | yes              | experimental                             | minimal Rust server              | yes, Rust                          | format stable since 1.0                    | merge only, reference |
| any-sync             | yes              | yes                                      | yes, heavy                       | no, Go                             | not verified                               | no                    |
| Jazz 2 / Classic     | alpha / legacy   | no / yes in JS                           | yes                              | no                                 | —                                          | no                    |
| Ditto, Zero, Triplit | —                | no                                       | no / Enterprise                  | —                                  | —                                          | no                    |
| Syncthing / iCloud   | yes              | Syncthing: beta                          | no server                        | file transport only                | —                                          | transport only        |
| **ADR-0001 (own)**   | —                | yes, per the PRD                         | axum + Postgres                  | yes                                | yes, `sv` per group                        | —                     |

## Comparison with our own protocol

**Risk of data loss.**
- **Our own:** the risk is bugs in our code. Checks: 39 property tests in S4a and 12 tests in S4b (ADR appendix §B), review by `sync-crypto-guardian`, an external audit before the public release (§10.3).
- **Someone else's engine:** on top of our bugs at the integration seam, three more risks are added:
  - bugs in the engine itself;
  - a mismatch between its model and the PRD: in SQLite Sync devices on different versions do not sync, in Loro nodes without a common point after history trimming do not sync;
  - the project dying: this happened to cr-sqlite, Triplit, Replicache.
- **A safety net outside sync exists in every option:** the Markdown copy, 7 database backups, the integrity check (§2.1, §12.3).

**Timelines.**
- §14 allots 14 days of spikes to sync: S4a 5, S4b 4, S4c 5. §15 does not say how many days slice 11 gets.
- Codex has already pointed out that these days test a hypothesis rather than estimate readiness (`review-codex.md` item 19).
- An engine can replace only S4a (merging, HLC). This is an estimate, not a measurement. But integration and an "encrypted payload through our own server" prototype will also take days. S4b, S4c and slice 11 do not shrink in any option.

**What we will have to write ourselves in any option.**
- The envelope and the AEAD over the open fields.
- The key hierarchy: Argon2id, DEK wrapping, 24 words, password change, recovery.
- The server: accounts, `seq` under a lock, blobs, quotas, backup with generations.
- Attachment encryption.
- Purge and the tombstone horizon, quarantine.
- The first device link after an import of 100k entries.
- The version matrix.

None of the candidates above provides this.

**Vendor lock-in.**
- **Our own:** no lock-in. The format is described in the ADR and public for self-hosted (§10.4).
- **SQLite Sync:** the server side is tied to SQLite Cloud.
- **PowerSync:** FSL license.
- **Evolu, Loro, Yrs:** MIT, but the update format and the protocol are theirs.

**Fit with the PRD.**
- Only ADR-0001 covers requirements R1–R9.
- On R1, ADR-0001 and Evolu have the same honest limitation: the server sees when and how much is written. The PRD accepts this (§10.3).

## Options

**A. Our own, as in ADR-0001 (I recommend this).**
- Pros:
  - exact fit with the PRD, including mixed versions, quarantine and generations;
  - the Rust core in `kinige-domain` builds for wasm32 for the web, without someone else's runtime;
  - the format and the server are ours, no lock-in;
  - the primitives are the same as Notesnook's, the architecture the same as Actual Budget's.
- Cons:
  - all responsibility for correctness is ours;
  - the ADR is already large: 400 lines plus a 233-line appendix;
  - the spike timelines are a hypothesis.
- Risk reduction:
  - S4a as a hard gate: a fault-injection simulation per the ADR test plan, before any server code;
  - check the decisions against Actual Budget (how "reset sync" works, where a Merkle tree is used instead of `seq`) and Loro Protocol / secsync (snapshots plus a log on a dumb server);
  - an external audit before the public release.

**B. A ready-made SQLite CRDT extension + our own encryption and server.**
- Realistically only SQLite Sync (cr-sqlite is abandoned).
- Pros:
  - ready-made per-column merging and line-by-line text merging;
  - builds for mobile platforms and WASM.
- Cons:
  - violates R5: a different schema means rejection;
  - the server side is tied to SQLite Cloud;
  - tombstone garbage collection not verified;
  - purge, quarantine and keys are still ours;
  - the server-side checks on headers from the ADR (purge ordering, `stale_purge`) are not possible on an opaque payload.
- At best it saves S4a, and it adds a dependency on a C extension on every OS.

**C. Hybrid: our own (A) + Loro (or Yrs) only for the `body` text.**
- The message body is stored as a Loro document inside the encrypted envelope. The other groups are as in the ADR.
- Pros: concurrent edits of the same message merge automatically, without a `conflict` revision.
- Cons:
  - the PRD says outright "a CRDT for text is not needed: messages are short and rarely edited" (§10.2);
  - automatic merging hides the conflict, while the ADR shows both versions, which is closer to principle 9;
  - the entry size grows, the document history needs trimming, a second data model appears.
- It makes sense only if the user decides that edits from two machines are frequent. It can be added later, as a new group or by bumping `sv` (ADR §4).

Separately, I do not recommend the "no server" option (Syncthing or iCloud). The protocol is the same, Apple forbids putting SQLite in iCloud, Syncthing's encrypted folders are beta. On top of that it breaks Kinige Cloud and the web client from the PRD.

## Recommendation

**Option A.** No ready-made engine takes on what is actually dangerous: keys, recovery, purge and tombstones, generations, mixed versions. The ones that do take on encryption are either not in Rust or at the alpha or experimental stage. So "buying off the shelf" adds a dependency without removing the risk.

What specifically to take from others:
- the libsodium primitives (already in the ADR);
- a reference for a simple server and an HLC log: Actual Budget;
- a reference for "a dumb server stores encrypted snapshots and updates": secsync and Loro `%ELO`;
- a reference for a per-item envelope: Notesnook.

If S4a goes over double its time budget (§12.1a), the fallback is to simplify the protocol, not to buy an engine. For example, LWW on the whole entry instead of field groups. The user decides this; see question 2.

## Questions for the user

1. **Concurrent editing of the same message on two machines:** show both versions with a "there is another version" mark (as in the ADR) or merge automatically line by line (option C)? This is the only thing a ready-made library really provides.
2. **If S4a does not fit into double its time budget (10 days):** simplify the protocol (fewer guarantees: for example, concurrent edits of flags and of a move resolved by LWW on the whole entry) or stop and revisit the MVP+ timelines?
3. **Metadata privacy:** the server sees when and how much you write (change times, sizes to within 1 KB); this is how ADR-0001, Evolu and Actual Budget work. The PRD accepts this. Please confirm that this is acceptable for a journal: hiding the rhythm is a separate, complex protocol.

## Not verified

- Whether SQLite Sync's encrypted payload can be pushed through our own server in practice. Whether they have a standalone sync service. How they collect tombstones.
- The "experimental" label on `%ELO` in the Loro blog: the page returned 403, the information is per the agent. Whether Loro's Rust server supports `%ELO` mode directly.
- Memory and overhead of CRDT documents for 100k messages: none of the candidates has a measurement.
- Production users of Automerge, Loro, Evolu, SQLite Sync: the primary sources do not name them.
- Whether OPFS and Shared Workers work in WKWebView under the Tauri scheme (important for Evolu).
- iCloud entitlements for an ad-hoc-signed app.
- The number of days for slice 11: not stated in §15 of the PRD.
