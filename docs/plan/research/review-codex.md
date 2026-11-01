The plan has become substantially better on the process side: vertical slices, visual acceptance, Markdown and the separation of UI and storage reduce the risks of the previous version. But **sync and encryption cannot yet be considered defined well enough for implementation**.

I read the PRD, the review and the three research documents. Below, "fact" means verified content of the documents or of a primary source; "risk" means a scenario of a possible implementation error. I do not revisit decisions D1–D19 on taste. I did not change any files.

1. **Critical — §10.3: crypto-shredding does not work as stated.**  
   **Fact:** the DEK, encrypted with a password-derived key, is stored on the server; deleting this record is promised to make even backups unreadable. However, an old backup keeps the wrapped DEK: the password or the stored client keys still allow the data to be decrypted.  
   **Proposal:** remove the guarantee that all backups are destroyed by deleting a row. Separately define retention periods for server backups and what happens to keys on devices and to exports. If cryptographic destruction of backups is really required, a separate key architecture is needed, not the current scheme.

2. **Critical — §10.1, §10.3, Q12: the key lifecycle and authentication are not defined.**  
   **Fact:** the password, master key, DEK, recovery key and device confirmation are listed, but the relationships between them are not described. It is unclear what happens on password change/reset, device loss, token revocation and account recovery.  
   **Risk:** an ordinary server-side password check may hand the server a secret from which the decryption key is derived. Token revocation also does not take back a DEK that has already been copied.  
   **Proposal:** before S4, describe registration, login, recovery, password change, device joining and revocation; separate authentication secrets from encryption secrets. Generate the DEK randomly and wrap it with separate keys for the intended recovery methods.

3. **Critical — §3.2, §9.2, §10.3: the web client breaks the unconditional promise of protection from the administrator.**  
   **Fact:** the web client decrypts data in the browser. **Risk:** if the page code is served by the same administrator the data is being protected from, they can substitute the JavaScript and obtain the password, the key or the plaintext. Server-side encryption does not prevent this.  
   **Proposal:** explicitly distinguish protection against reading the server DB from protection against a malicious client provider. For the web, define a trusted delivery channel; for the desktop, independent verification of update signatures. Do not promise the same trust model for these clients.

4. **Critical — §10.2–10.3: the authenticated format of an encrypted operation is not defined.**  
   **Fact:** the algorithm is specified, but the nonce format, encryption version, key identifier and binding of plaintext metadata to the ciphertext are missing.  
   **Risk:** without such a binding the server will be able to change the HLC/deletion flag or swap ciphertexts between objects without changing the encrypted body.  
   **Proposal:** specify the operation envelope and include in the AEAD associated data the account, type, id, operation identifier, HLC, deletion, and format and key versions. Verify authenticity **before** applying the metadata. The purpose of associated data is confirmed by the [libsodium documentation](https://doc.libsodium.org/secret-key_cryptography/aead).

5. **Critical — §4.2, §10.2: LWW does not provide the promised preservation of revisions.**  
   **Fact:** the server merges encrypted snapshots by metadata; the losing version is supposed to become a `message_revision`. But delivery of losing snapshots and sync of the revisions themselves are not described. The revisions table stores only `body_md`.  
   **Risk:** the server will discard the losing entry before another client receives it; changes to the chat, date, flags or attachments cannot be restored from a text revision.  
   **Proposal:** store and deliver immutable encrypted versions with unique ids; use LWW to pick the current state. Define what a restorable version contains, revision deduplication and history pruning rules.

6. **Important — §4.1–4.2, §10.2: HLC is so far a label, not a protocol.**  
   **Fact:** `hlc TEXT` and choosing the larger value do not define the encoding, a total order, tie handling or persistence of the clock state.  
   **Risk:** different devices get identical timestamps, string comparison diverges from numeric comparison, the clock rolls back after a DB restore; a device with a date far in the future suppresses subsequent edits.  
   **Proposal:** define the canonical representation, the tie-breaker, the local/receive algorithms, clock persistence and an allowed drift policy. Remove the phrase "a homegrown one is 50 lines": the complexity here is in the invariants and convergence checks.

7. **Important — §4.2, §10.2: the outbox does not capture the content of the version being sent.**  
   **Fact:** a row contains only a reference to the object and the HLC.  
   **Risk:** after two quick edits the background sender will read the already-new body but send it with the old timestamp. The acknowledgement of the first send may mistakenly delete a fresher pending edit.  
   **Proposal:** choose explicitly: an immutable payload per operation, or a current-state queue with atomic reads and conditional acknowledgement of a specific version. For idempotency, define the behavior for a repeated id with different content; a retry must reuse the prepared envelope.

8. **Important — §10.2: a monotonic server number does not yet guarantee a safe pull.**  
   **Risk:** if the number is an ordinary PostgreSQL sequence, transaction A gets 10, B gets 11 and commits first. The client sees 11 and advances its cursor; the later-committed 10 is skipped. This is a scenario of a possible implementation, not a statement about existing code. A sequence does not guarantee commit order — see [PostgreSQL](https://www.postgresql.org/docs/current/functions-sequence.html).  
   **Proposal:** define a publication mechanism without such gaps, for example serializing number assignment and commits within an account. On the client, apply a page of changes and save the cursor in one transaction; describe the initial load and cursor expiry.

9. **Important — §4.1, §10.2: trash, tombstones and restore contradict unconditional delete-wins.**  
   **Fact:** there is restore/undo of deletion and the rule "deletion beats an edit", but there are no separate restore semantics. The tombstone retention period is not set.  
   **Risk:** a restored entry gets deleted again; a long-disconnected client resurrects physically deleted data.  
   **Proposal:** separate the delete and restore operations and define their ordering. Do not tie the tombstone lifetime to the 30 days of the trash. After old tombstones are removed, require devices that are too old to do a full resync without automatically bringing back old entries.

10. **Important — §4.3, §10.2–10.3: the attachment protocol is not defined.**  
    **Fact:** §10.2 uses `sha256`, §10.3 an HMAC of the hash; the attachment snapshot includes the local state `blob_state`. Keys/nonces for files and repeated uploads of identical content are not defined.  
    **Risks:** blob corruption on a concurrent upload, permanently orphaned files, deletion of a blob still needed by a revision or another message.  
    **Proposal:** separate the local hash, the remote blob id and the device's upload state. Define immutable blob publication, integrity checks, resumption and upload limits; the garbage collector must account for live entries, the trash, revisions and unfinished operations.

11. **Important — §4.1–4.2, §10.2: cross-object invariants are not defined.**  
    **Risks:** two independently installed clients will create different Inboxes; a message arrives before its chat; an attachment before its message; LWW of a message will keep attachments from another version. UUIDs solve object uniqueness, but not these situations.  
    **Proposal:** fix the identity of the Inbox within an account, the ordering/buffering of dependencies and the boundaries of an atomic change. When a local database is first connected to an account, explicitly define how Inboxes are merged, how references are preserved and how existing data is resolved.

12. **Important — §3.2, §10.1, §12, §15: updates and version compatibility have dropped out of the plan.**  
    **Fact:** server migrations run automatically, but versions of the wire format, the encrypted payload and the minimum client are missing.  
    **Risk:** an old client receives a snapshot with a new field, edits it and overwrites the unknown data; an updated client migrates SQLite, after which rolling back the application is impossible.  
    **Proposal:** before sync, define a compatibility matrix, preservation of unknown fields or a ban on incompatible writes, a backup before migration and an update-failure scenario. Include signed updates and storage of the signing key in the roadmap; a signing mechanism exists in the [Tauri updater](https://v2.tauri.app/plugin/updater/).

13. **Important — §10.1, §16: backup is described as a set of files, without a restore guarantee.**  
    **Fact:** a Postgres dump plus an attachments folder is proposed, but not a consistent snapshot point or a restore procedure. A local automatic backup is not planned either.  
    **Risk:** the dump contains a reference to a missing blob; the restored server has an old cursor while the clients continue from a newer one. Sync by itself is not a backup.  
    **Proposal:** define the order of blob creation/deletion relative to the backup, reference checks and a server generation identifier after a restore. The mandatory check is restoring keys, history and attachments into a clean installation. Explicitly label the export as plaintext and define its protection separately.

14. **Important — §3.2, §6: capabilities and the sanitizer do not yet close the Tauri surface.**  
    **Fact:** the PRD requires per-window capabilities, but does not carry over from the research the need to restrict the app's own commands via `AppManifest::commands`. According to the documentation, registered application commands are available to all windows by default — [Tauri](https://v2.tauri.app/security/capabilities/).  
    **Proposal:** make a verifiable matrix "window → command → allowed arguments" and negative IPC tests. Additionally define allowed link schemes, the local file scope and a policy for external images/previews: loading them reveals network metadata, and an unrestricted Rust URL loader can reach local services.

15. **Important — §3.1–3.2, §9.2, §10.3: `HttpApi` does not replace the Rust core for the web.**  
    **Fact:** all business logic is in Rust and the server does not see the text, yet the web is expected to be obtained by swapping the API implementation. The server will not be able to perform search, forest computation and other operations on plaintext content.  
    **Proposal:** before promising a shared frontend, choose the web client architecture: Rust/WASM with browser storage, or a separate implementation with shared contract tests. `KinigeApi` is useful, but by itself does not settle where the logic and local data live.

16. **Important — §4.1, §10.3: the promise to hide dates does not match the plaintext identifiers and HLC.**  
    **Fact:** UUID v7 contains a Unix timestamp, and the HLC reveals timing information about changes. The structure of UUID v7 is described in [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html#name-uuid-version-7).  
    **Conclusion:** an encrypted `journal_date` hides the chosen diary date, but the server still sees the approximate creation time of objects, activity, sizes and blob reuse.  
    **Proposal:** compile an exact list of the metadata that is revealed. If hiding the creation time matters, use opaque external ids; hiding activity would require a much more complex protocol.

17. **Important — §9.2: synced embeddings have no freshness model.**  
    **Fact:** vectors are supposed to be synced, but their schema and their link to the text version are missing.  
    **Risk:** an API response for an old text will complete after an edit and replace a fresh vector; devices will mix models or pay for indexing again. Excluding a chat while the queue is running is not described either.  
    **Proposal:** store the source version/content hash, the model, the dimensionality and the processing version. Accept a result only for the current version; re-check the AI permission right before sending. Record whether repeated costs are acceptable and the behavior when AI is turned off, the model changes and a note is deleted.

18. **Important — §3.2, §4.2–4.3, §5, §12.2: local reliability is stated too optimistically.**  
    **Fact:** "one process, WAL, so there are no races" is wrong: tasks from several windows, sync and background handlers can compete within the process. WAL allows only one writer and does not rule out `SQLITE_BUSY` — [SQLite](https://www.sqlite.org/wal.html).  
    **Proposal:** define the write owner and transaction boundaries. Describe atomic file import via staging/rename, cleanup of orphans, and saving of drafts and their attachments. Distinguish the guarantees for a killed process and for a power loss; test them separately.

19. **Important — §2, §11, §14–15: validation of the product's usefulness starts too late.**  
    **Assessment:** a full design system with the key screens, nine slices, a 100k feed and final art before the four weeks of use start repeat the risk of a broad scope. Seven days of S4 are reasonable for testing a hypothesis, but are not an estimate of readiness for reliable sync/E2EE.  
    **Proposal:** start daily use after stable input, minimal chats, editing and a verified export; move quick capture earlier. Grow the design system component by component per slice, keeping visual acceptance; accept the forest with simple assets first. Run a short S4 on the envelope and the data lifecycle before the schema is fixed, and the full network check before slice 11.

20. **Important — §2.4, §4.1, §8, §16–17 and `01-review.md`: the documents allow incompatible interpretations by an agent.**  
    **Facts:** E2EE is both in P2 and required for MVP+; embeddings in §4.1 are still "on the server"; the review keeps the old recommendations for server-side AI; XP `hash(day, reason)` does not explain five separate bonuses; import by UUID does not define the conflict with an already modified entry.  
    **Proposal:** declare v0.4 the only normative document and explicitly mark the old recommendations as obsolete. Define an id for each XP award and the import modes "restore / merge / create a copy". Before the first public release, add a license check for code, fonts and art; the current documents do not establish the legal compatibility of AGPL and the paid part.

**Three main conclusions:**

1. **Crypto-shredding currently promises more than the scheme provides.** A threat model and the full key lifecycle are needed first.
2. **Outbox + HLC + LWW is a direction, but not yet a sync specification.** The main gaps: version preservation, cursors, restores and the attachment lifecycle.
3. **Usage needs to start earlier.** Keep the chosen stack and visual acceptance, but shorten the path to a daily journal; grow the full design system and art as usefulness is confirmed.
