# Independent review of the Kinige v2 plan (PRD v0.4)

Read: `02-prd.md`, `01-review.md`, `research/tauri-stack.md`, selectively `research/competitors.md` and `research/apple-tech.md`. No files were changed. I did no web checks: every statement below either follows from the PRD text or is marked as my judgment/assumption. I did not re-check library versions and prices from `research/` — they are already marked CONFIRMED/UNCERTAIN there.

I do not challenge the "Decided by the user" decisions (stack, E2EE, self-hosted, dark theme, etc.) — I only show where, inside the PRD, they do not fit with each other.

---

## Items (in descending order of importance)

### 1. Critical — The server cannot "merge in kinige-domain" if it does not see the data (§3.1, §10.1, §10.2, §10.3)

**Problem.** §10.1 says: the server is "dumb storage", it merges only by metadata (id, HLC, deletion flag). §10.2 says: "Merging is in `kinige-domain`, the same on the client and the server", including "the losing version → `message_revision(reason='conflict')`" and "deletion wins over an edit, but the edit is kept in revisions". The latter is impossible with E2EE: the server can neither create a revision with a body nor understand that an edit "was kept". §3.1 also draws `kinige-domain` as reused by the server "for merge". The implementing agent will most likely implement merge on the server and run into encrypted blobs, or start sending plaintext fields to the server "for merging".

**Proposal.** State explicitly: for each entity the server stores only the latest snapshot by maximum HLC (or an append-only log) and does not interpret the content. All conflict semantics are on the client at pull time: if an incoming snapshot beats a local unsynced one, the client itself puts the losing local version into `message_revision(reason='conflict')`. The losing device must learn that it lost: the server replies to the push with "rejected, the current snapshot is this" (or the client sees it on the next pull). Remove "merge" from the §3.1 description of what the server reuses (the server can reuse only the protocol types from `kinige-sync`).

### 2. Critical — "Deletion wins over an edit" contradicts a restorable trash and LWW (§4.1, §10.2)

**Problem.** A 30-day trash implies that `deleted_at` can be cleared (restored). If a separate rule "deletion wins over an edit" applies during sync, then a restore on device B (clearing `deleted_at`, a larger HLC) will lose to an older deletion from device A — the note cannot be restored, and that is a direct violation of principle 9, "nothing is lost silently". At the same time it is unclear at what point a soft deletion becomes a "tombstone" and how a tombstone differs from a snapshot with `deleted_at`.

**Proposal.** One rule instead of two: `deleted_at` is an ordinary snapshot field, LWW by HLC, like all the others. There is no separate "delete wins". Physical deletion after 30 days is a local operation on expiry (plus a "purged" tombstone record with an HLC so that the other devices purge too). Write this into §10.2 and rewrite the S4 criterion: "a restore from the trash on one device reaches the other".

### 3. Critical — The login password and the encryption password are not separated; the salt and the recovery key wrapping are not described (§10.3, Q12)

**Problem.** "The master key is derived from the password (Argon2id) and never leaves the device", but the server is multi-user with accounts — which means there is a password login. If the client sends the same password to the server for authentication, the administrator (or a log dump) gets the password and derives the master key — E2EE is nullified. Further: (a) where the Argon2id salt is stored for logging in from a new device — not stated; (b) "the recovery key (24 words)" — not stated how it opens the DEK: a second DEK wrapping is needed (the DEK encrypted with a key derived from the 24 words) on the server, otherwise the 24 words restore nothing; (c) a password change = re-wrapping the DEK, not re-encrypting the data — this also needs spelling out, otherwise the agent will re-encrypt 100k entries; (d) "confirmation from an already linked device" is a separate protocol for transferring the key between devices (QR/PAKE), a noticeable amount of work.

**Proposal.** Describe the key scheme in one paragraph: two independent keys are derived from the password via Argon2id (a KEK for wrapping the DEK and an auth secret for the server; the server stores only a hash of the auth secret); the salt and Argon2 parameters are on the server in plaintext next to the account; the server holds two DEK wrappings — by the password and by the recovery key; a password change = a new wrapping. Move key transfer "from a linked device" to P1/P2; in MVP+ — password only (which is what the S4 criterion says). This is exactly the content of Q12 — it must be resolved before slice 11, not "in a spike".

### 4. Critical — Crypto-shredding as described does not make backups unreadable (§10.3, §1.2 item 6)

**Problem.** It is claimed: "deleting the account erases the encrypted keys on the server → the data, including backups, is unreadable". But if the DEK wrapping lives in the same Postgres that goes into the dump (§10.1: "backup — a Postgres dump + the attachments folder"), then the backup contains both the encrypted DEK and the data; the backup's owner + the user's password (or the 24 words) can read everything. Erasing from the "live" database does not affect the backup. Against the user themselves the scheme does not work either (the DEK is on their devices) — that is normal, but the "including backups" promise must either be ensured or withdrawn. Separately: the agent may take "deletion = deleting the keys" literally and leave the blobs on disk forever.

**Proposal.** Either (a) add a server-side key component: the DEK wrapping is additionally encrypted with a random per-account server key that is stored outside the dump (a separate file/secret, not backed up together with the data) — then erasing it really does make backups unreadable; or (b) write honestly: "deletion erases keys and data from the server; backups are kept for N days and then rotated". Plus: when deleting an account, delete the blobs/records themselves too (quota, disk).

### 5. Critical — Whole-entry snapshot + LWW between clients of different versions silently loses fields (§10.2, §4.1, §2.6)

**Problem.** The unit of sync is "the whole entry snapshot", the merge is "the larger hlc wins". When client v2 adds a field (for example, `reply_to_id`, tags) and client v1 on another machine edits the same entry, the v1 snapshot without the new field will win and erase it. Payload versioning and schema migrations on clients of different versions are not mentioned in the PRD at all (only "migrations" in `kinige-store`). With E2EE the server cannot help.

**Proposal.** Include `schema_version` in the encrypted snapshot; the client must preserve unknown fields and return them when re-sending (round-trip), or refuse to edit entries with a version higher than its own and offer to update. The rule "a client is not older than the server by more than N versions" and the minimum supported protocol version — in the API. This must be decided before slice 11, because afterwards the snapshot format is a public contract for self-hosted users.

### 6. Important — Tombstones without a lifetime and "resurrection" of entries by a stale device (§4.1, §10.2)

**Problem.** It says "tombstone → physical deletion", but does not say how long the server keeps a tombstone. If it is deleted before a device that has been offline for half a year does a pull, that device will bring the entry back on push (for it the entry is alive, and the server knows nothing about it) — classic resurrection. Also, on pull the client gets its own changes back (the cursor is a shared server `seq`); this must be filtered by `device_id`/HLC, otherwise there will be spurious "conflicts".

**Proposal.** Write down: a tombstone lives on the server for no less than T (for example, 90 days); a device with a cursor older than T must do a full resync (the server answers "cursor too old"). Pull skips snapshots with `device_id == self`. Add the scenario "a device came back after 100 days" to the S4 criteria.

### 7. Important — LWW at the whole-entry level loses concurrent edits of different fields (§10.2, §4.2)

**Problem.** Device A sets "favorite", device B edits the text — one snapshot wins, the second change disappears and does not even make it into revisions (revisions are only about `body_md`). For "short messages that are rarely edited" this is rare, but flags/dates/moving between chats are frequent operations from different machines.

**Proposal.** Inside the encrypted snapshot, store an HLC per field group (`body`, `flags`, `chat_id/journal_date`, `deleted_at`) and merge the groups independently (that is ~30 lines in `kinige-domain`, without CRDTs). If you don't want that — at least explicitly write down the limitation and add a test showing that losing a flag during a concurrent text edit is expected.

### 8. Important — Contradictions inside the PRD that will confuse the agent (§2.4, §4.1, §10.3, §2.1, header)

- §2.4 "P2 and later" ends with the word **"E2EE"**, while §10.3 and slice 11 make end-to-end encryption part of MVP+. A leftover from v0.3 — delete it.
- §4.1: "Derived data (…; **embeddings — on the server**) is cached" contradicts §9.2 "embeddings are computed by the client". A leftover — delete it. The same in `01-review.md` §4.5 ("it makes sense to compute embeddings on the homelab server") and the §3 table ("pgvector") — mark as outdated.
- The header and D2: web is a "thin client"; §10.3: "the web client is not 'thin'". Keep one wording.
- `message_revision` has no `hlc`/`device_id`, but the export (§16) and §10.2 treat revisions as data. Decide: revisions are local (then history differs across devices, and exports from different machines differ) or a synced entity (then `hlc`, `device_id`, outbox are needed). I would make them synced, append-only — that is cheap.
- `01-review.md` "Same-day update" still asserts "the design system in Claude Design" — a paragraph above, this is cancelled. Remove it so that the agent does not read two opposite decisions.

### 9. Important — P0 scope for a single developer and the slice order (§2.1, §14, §15)

**Problem.** Before the first value (slice 1) there are: slice 0 (CI, Tauri, Storybook, tokens, visual tests, screenshot script), slice D (the whole design system, including **three feed variants** as full-fledged stories), spikes S1 and S3. P0 includes revisions, the trash, moving, date changes, link previews, a 100k feed, Russian FTS, the forest with a Blender art pipeline, the hotkey, the tray, accessibility, export+import. This is broader than what the review itself names as the cause of failure (L7). S4 at 7 days covers the protocol, server, Postgres, docker compose, encryption, keys, recovery — unrealistic; S4 is the biggest part of the whole project.

**Proposal.**
- Choose among the three feed variants from static concepts/sketches, not from three sets of components; only one in Storybook.
- Move from P0 to P1: link previews (network, privacy, a separate parser), date changes, three levels of forest detail (keep one sprite + a dot), the "media" filter.
- Put slice 8 (hotkey, tray) right after 1–2: it is the first "pillar", Capture, small in scope, and its absence from the 4-week trial will distort the conclusions.
- Split S4: S4a — the protocol on plaintext data as an in-process simulation of two clients with property tests (no server, no network); S4b — the crypto envelope and key onboarding; S4c — server + docker compose. Separate limits.
- Keep only "your own export → a clean install" for import in P0 (that is exactly slice 9).

### 10. Important — Client updates, signing and notarization are missing from the PRD (§12, §15)

**Problem.** `research/tauri-stack.md` item 11 recommends `tauri-plugin-updater` with mandatory signing and keeping the key outside `.env`; the PRD says not a word about updates, build signing, macOS notarization (Gatekeeper), Windows signing. For a "product", without this no outside user will launch the app, and for yourself, updating over the network via the homelab is the main scenario. The updater signing key is a thing that must not be lost and must not leak (a leak = forged updates).

**Proposal.** In slice 0 — a CI build with signing (at least ad-hoc) and an ADR "where the signing/notarization keys live"; in slice 10 — the updater with an endpoint on the homelab and a key revocation procedure. Put the Apple Developer ID (paid) and a Windows certificate on the list of risks/costs.

### 11. Important — The web client breaks the E2EE threat model (§2.3, §10.3)

**Problem.** The web client is served by the same server. A server administrator who "cannot read the data" can swap the JS and get the key at the next login. This is a well-known weakness of E2EE in the browser; the PRD does not name it, and the marketing line "the administrator cannot read it" becomes false for the web.

**Proposal.** Write into the "cost of the decision" in §10.3: "for the web client the E2EE guarantee is weaker — trust in the server that serves the code". Mitigation options (P2): serve the web client from a separate origin/CDN, a browser extension, SRI + a reproducible build. For the desktop the guarantee holds.

### 12. Important — Feed order: `created_at` or `journal_date`? (§4.2, §6, §4.1)

**Problem.** `journal_date` is edited manually, the feed groups "by days", the `message_chat_time(chat_id, created_at)` index and keyset pagination are by `created_at`. A message with a changed date will end up under one day's header but sorted by another day's time — or the agent will group by `created_at`, and the manual date will stop meaning anything in the feed. "Jump to date" — by which field is also not stated.

**Proposal.** State: the feed and jump-to-date use `(journal_date, created_at, id)`; the index is `message(chat_id, journal_date, created_at, id)`; the day for the forest and `day_activity` is also `journal_date`. At the same time decide the "day boundary" (does an entry at 00:30 belong to the previous day?) — a "day ends at HH:MM" setting is cheap if built in right away, and expensive later.

### 13. Important — Attachments: encryption of large files, the server-side identifier, link previews (§4.2, §4.3, §10.2, §10.3)

**Problem.** (a) XChaCha20-Poly1305 "the whole snapshot" for files means encrypting the entire file in memory; voice notes and future videos need a streaming format (chunks, `secretstream`) and resumable downloads. (b) Locally, files are addressed by `sha256`; on the server, by HMAC(sha256). The `attachment` schema has no field for the server-side identifier; `blob_state='remote_only'` on a new device requires knowing what to request. (c) `link_image_sha256`/link previews — the app itself goes to other people's sites from the user's local network: leaking the fact that the link exists and the IP, plus SSRF-like risks with a self-hosted web client if the server does the previews.

**Proposal.** Describe the encrypted blob format (an N MB chunk, a nonce per chunk, a header with size and MIME, all under the DEK); add `remote_id` to `attachment`; link previews — only on the client, only on explicit action or with a setting, in P1 (see item 9).

### 14. Important — Metadata leaks the PRD does not name (§10.3)

**Problem.** In plaintext: id, type, HLC, deletion flag, size. The HLC contains wall-clock time — the server sees the exact time of every edit, i.e. a complete picture of activity by days and hours (which for a journal is exactly the "forest"). Sizes — the approximate length of entries, the number and size of photos. `deleted_at`/the deletion flag — when things were deleted. This is the same caveat as Day One's (dates are not encrypted), and that is fine — but the PRD in §10.3 claims "dates … are encrypted", which is half wrong.

**Proposal.** Rewrite the paragraph honestly: "the server can see the time of changes (via HLC), sizes and the fact of deletion; it cannot see the content, names, `journal_date`, `created_at`". Optionally: round the size (padding to 1 KB) and use only `seq` for server-side ordering, keeping the HLC inside the envelope plus a coarse HLC outside — this is already an engineering choice, mark it as a possible improvement.

### 15. Minor — HLC: encoding, tie-break, clock drift (§4.2, §10.2)

**Problem.** `hlc TEXT` is compared as "larger" — a fixed width and lexicographic sortability are needed, otherwise `"9…" > "10…"`. With an equal HLC from two devices, a tie-break by `device_id` is needed (if it is not part of the HLC). There is no policy for badly skewed clocks (uhlc, according to their research, rejects timestamps that are "too far in the future"): what does the user see when their edits are rejected?

**Proposal.** The format `{wall_ms:013}-{counter:05}-{device_id}` (or an equivalent) in an ADR; a sorting test; the rule "on drift > X minutes show a warning and do not block local work".

### 16. Minor — Schema: FTS does not cover everything, FKs when applying a pull, chat deletion cascade, `xp_ledger` (§4.2, §9.1, §8)

- §9.1 promises search by file names, link titles and chat names; `message_fts` indexes only `body_search`. Either separate FTS tables or an aggregated column are needed.
- `message.chat_id REFERENCES chat(id)`: on pull a message may arrive before its chat (different server `seq`, a partial pull) — apply the pull in a single transaction with `PRAGMA defer_foreign_keys`, or have no FKs on the synced tables.
- Chat deletion: what happens to the messages — a tombstone for each, or are they "hidden" by the parent? This affects the trash, export and quotas.
- `xp_ledger.id = hash(day, reason)` with "+1 for additional notes up to +5" will produce one record per `reason` — `hash(day, reason, n)` or `reason='extra_3'` is needed. P1, but the formula is in the PRD.
- `change_outbox` without dedup by `entity_id`: ten edits of one entry between syncs — ten rows, but a single snapshot goes out on push; better `UNIQUE(entity, entity_id)` with an `hlc` update.

### 17. Minor — Platform limitations of quick capture and secrets on Linux (§5.1, §9.2)

**Problem.** A global hotkey on Wayland is limited by the protocol (marked UNCERTAIN in the research, not reflected in the PRD); `single-instance` under snap/flatpak requires configuration; Secret Service may be missing on minimal environments — storing the OpenRouter key "in the system keychain" on Linux is not guaranteed.

**Proposal.** In slice 10 — an "X11/Wayland × GNOME/KDE" matrix for the hotkey with a fallback "hotkey only inside the window + tray"; for the OpenRouter key — a fallback to a file with 0600 permissions and an explicit warning.

### 18. Minor — Legal and product (§2.6, Q13)

- AGPL for the core + a commercial part inside an **open desktop client** (AI, pet, cosmetics) — the paid part is trivially forked; open-core usually works when the paid part is on the server or in hosting. Not a blocker, but it affects Q13.
- Sending text to OpenRouter in a "product" requires a privacy policy and explicit consent; "turnkey hosting" for other people = personal data operator obligations (GDPR/152-FZ depending on jurisdiction) — I would explicitly move hosting for others to "out of scope" until the public release.
- The review already notes the risk of the name "Forest" (Seekrtech); in the PRD the feature is called "лес/Forest" (forest/Forest) everywhere — add a trademark check before publication to Q13.
- OFL fonts — ok; CC0 placeholder sprites — make sure they do not leak into the release.

### 19. Minor — Local backups and data loss without a server (§4.1, §10.3)

**Problem.** The server is E2EE, locally it is a plaintext SQLite. On losing the password and the 24 words "the local copies remain", but until MVP+ there is no server at all, and the local database is the only copy; backup = manual export. For the 4-week "on yourself" trial this is already a risk.

**Proposal.** In slice 1 or 3 — automatic periodic export/DB snapshot into a chosen folder (Time Machine will pick it up); an integrity check `PRAGMA integrity_check` at startup with a clear message.

### 20. Minor — Wordings the agent will misunderstand

- "The server accepts idempotently by `(entity_id, hlc)`" — it does not say what to do with a snapshot whose HLC is smaller than the one already stored: the answer must be "accepted, but lost, here is the winner", not an error and not silently storing two versions.
- "Delete data = delete keys" — see item 4: physical deletion is needed too.
- "All colors are semantic tokens … without duplicating tests" + "Storybook visual tests" in CI: clarify that visual snapshots are taken on one theme, and when a second theme appears only the token test is run — otherwise the agent will double the snapshots.
- "A file over 400 lines is a signal to split" combined with "no abstractions for future features" — for `kinige-store` with SQL this will conflict; add "except migrations and generated types".
- §15 non-functional targets "at 100k" — it does not say on what hardware and with what sample to measure; "the methodology is fixed in slice 0" — good, but the slice 4 acceptance criterion must refer to a specific script.
- Exporting `manifest.json` as "full data" for 100k messages is a single JSON of hundreds of MB; JSONL per entity is better, spell it out.

---

## Three main conclusions

1. **The main architectural hole is at the junction of E2EE and sync, not in the choice of stack.** The server cannot merge, create revisions and apply the "deletion wins" rule if it does not see the data; "deletion wins over an edit" is incompatible with a restorable trash; crypto-shredding does not cover backups; the login password is not separated from the encryption password. All of this is solved on paper in a day (one ADR "protocol + keys"), but it must be solved **before** S4 and before the snapshot format becomes a contract for self-hosted users (items 1–5).

2. **Entry snapshot + LWW is a workable choice only with payload versioning and field groups.** Without `schema_version`/round-trip of unknown fields, clients of different versions will silently erase each other's data; without per-field HLC they will lose flags during a concurrent text edit. Both additions are cheap now and expensive later (items 5, 7).

3. **The P0 scope is still broader than the L7 lessons, and the order does not reflect the priority of Capture.** Quick capture is eighth, the three feed variants are built as three design systems, link previews and date changes sit in P0, S4 at 7 days covers a third of the project. Cut/reorder (item 9), and add what is missing entirely: updates, signing, notarization and local backups (items 10, 19) — without them neither the "product" nor even the 4-week trial on yourself is safe.
