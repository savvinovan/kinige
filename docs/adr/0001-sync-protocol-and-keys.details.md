# ADR-0001, appendix: formats, checks, schema, test plan

Appendix to [`0001-sync-protocol-and-keys.md`](0001-sync-protocol-and-keys.md), same status — "proposed", revision 6. The chain "PRD → ADR → this appendix" is normative. References like "§3.5" point to sections of the ADR unless "PRD" is stated. Terms follow the "Terminology" table of the ADR.

## A. Formats and protocol tables

### A.0. Field groups

The `core` group is written at creation. Derived data (`body_search`, `day_activity`, a chat's `updated_at`, tags) is not synchronized (PRD §4.1). `attachment_local` is not synchronized (PRD §4.2).

| Entity           | Group       | Fields                                                                                                                                                                                                                    |
|------------------|-------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| message          | `core`      | `created_at`, `created_tz`, `reply_to_id`, `origin` (`user` \| `agent` \| `import_telegram`, immutable), `ext` (optional, immutable: `{"src","chat","msg"}`, values as strings; external import id, ADR "User decisions") |
|                  | `body`      | `body_md`, `edited_at`, `base`, `supersedes` (up to 64 HLCs), `vv` (S4a, 2026-11-05: up to 16 HLCs, one per device) (§3.4)                                                                                                |
|                  | `flags`     | `is_task`, `task_done_at`, `favorite_at`, `pinned_at`                                                                                                                                                                     |
|                  | `placement` | `chat_id`, `journal_date`, `base`, `supersedes`, `vv` (§3.4; a message of a deleted or unavailable chat is shown in Inbox as derived state, §3.6)                                                                         |
|                  | `deleted`   | `deleted_at`                                                                                                                                                                                                              |
| chat             | `core`      | `created_at`, `is_inbox` (Inbox's `core` is a constant)                                                                                                                                                                   |
|                  | `meta`      | `title`, `icon`, `color`                                                                                                                                                                                                  |
|                  | `order`     | `sort_key` (on equal keys, `id` decides the order)                                                                                                                                                                        |
|                  | `state`     | `pinned_at`, `archived_at`                                                                                                                                                                                                |
|                  | `privacy`   | `ai_excluded` — separate, so that it is not lost on a concurrent rename                                                                                                                                                   |
|                  | `deleted`   | `deleted_at`                                                                                                                                                                                                              |
| attachment       | `core`      | `message_id`, `kind`, `sha256`, `filename`, `mime`, `byte_size`, `width`, `height`, `duration_ms`, `url`, `link_title`, `link_site`, `remote_blob_id`, `blob_key`                                                         |
|                  | `order`     | `sort_index`                                                                                                                                                                                                              |
|                  | `deleted`   | `deleted_at`                                                                                                                                                                                                              |
| message_revision | `core`      | `message_id`, `snapshot_json` (for `conflict`: `{"g", "h", "sv", "raw"}` of the `body` or `placement` group of version P, `raw` is a JSON string with the exact bytes of the group, §3.4), `saved_at`, `reason`           |

Each group's value is canonical JSON (RFC 8785, restricted: strings, booleans, `null`, objects, arrays, integers within ±2⁵³). Other clients' groups are stored and resent verbatim (§3.3).

**Conventions (as implemented in slice 1).**
- **Optional field:** for an "empty" value the field is present with the value `null` (`reply_to_id`, `edited_at`, `base`, `task_done_at`, `favorite_at`, `pinned_at`, `deleted_at`, `color`, `archived_at`); the field is absent only where the table says so — `ext` in the `core` of a message without an external id. An empty list is `[]` (`supersedes`). Parsing must accept both forms; writing is as described here.
- **`is_inbox` in a chat's `core` is informational; the client identifies Inbox by id** (§3.6): the flag on a chat with a different id does not make it Inbox, either in the UI or in the Markdown copy folder.
- **The group limit** of `body` is in canonical JCS bytes, not in text length (slice 1: 512 KiB): escaping doubles the size, and the envelope is limited to 1 MiB (§5.3).
- **The first group rewrite (slice 2, append, ADR-0006):** `kinige_domain::append::rewrite_body` writes canonical bytes of `body` only, the new HLC is strictly greater than the previous one (otherwise an error), `edited_at = now`; `base`/`supersedes` per §3.4 (a confirmed version — **by the definition of §3.4: there is no outbox row without `acked_seq`, and `server_known = 1`**; slices 1–10 have no outbox, so the caller takes the `CurrentBody::body_confirmed` flag from the `server_known` column, and in slice 11 it must also take the outbox into account: `base = hlc(V)`, `supersedes = []`; the device's own unconfirmed version: `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`, ≤ 64, the oldest are dropped). HLC timestamps are parsed strictly (`hlc::parse`); a damaged one is `Corrupt`. Other groups, unknown groups, unknown fields of `body` and **extra fields of the `hlc_json` entries of other groups** are preserved verbatim (the `hlc_json` entry of `body` itself is rebuilt: `{"h": new HLC, "sv": 1}`); `schema_version` is the maximum `sv` over groups. A record with a group in an `sv` higher than supported, or with a damaged `hlc_json`, is not appended to (the first is read-only, the second is an error).
- **Rewriting any group (slice 3):** `kinige_domain::rewrite::rewrite_group` — the same procedure for a message's `deleted`, `flags`, `placement`, `body` and a chat's `meta`, `state`. The group is rewritten with a new HLC strictly greater than the previous one; other groups, unknown groups, unknown fields of the group and extra fields of the `hlc_json` entries of other groups are preserved verbatim; a missing group, or a missing `hlc_json` entry for it, is an error (not a re-creation). The table columns after a rewrite are built from the new group bytes (`message_columns`, `chat_columns`), not the other way round. The first rewrite of `placement` (a day change) sets `base`/`supersedes` per §3.4 as for `body` (`chain_fields`). **Read-only** is a record in which at least one group (including an unknown one) has an `sv` higher than supported; any action on it is a `read_only` error, and the bytes do not change. Canonical values: `deleted` — `{"deleted_at":ms|null}`; `flags` — `is_task`, `task_done_at`, `favorite_at`, `pinned_at`; removing a task clears `task_done_at`.
- **An `edit` revision (slice 3):** on every edit of `body` (including a `- [ ]` checkbox in the text), a `message_revision` row is written in the same transaction — a record with only the `core` group: `{"message_id", "reason":"edit", "saved_at", "snapshot_json"}`, where `snapshot_json` is an **object** `{"groups":[{"g":"<name>","h":"<HLC>","sv":<u32>,"raw":"<exact bytes of the group as a string>"}, …]}` — **a full snapshot of the record before the edit**, all groups (known and unknown), the array sorted by group name, the whole object canonical (JCS). `raw` is a JSON string, as in `conflict` (§3.4): re-canonicalization on another client does not change the bytes. The HLC of the revision's `core` group equals the HLC of the new `body` version (one event), `sv = 1`; the id is a random v4; the columns `snapshot_json`, `saved_at`, `reason`, `message_id` are a parse of this group. Appending with ⌘Enter does not write a revision (the old text is a prefix of the new one, ADR-0006); `toggle_task_item` (a checkbox in the text) does. Edits of `flags`, `placement`, `deleted` do not create revisions.
  - **Group order in the snapshot** — by name, byte-wise in UTF-8 (group names are ASCII, so the order matches the order by UTF-16 code units). The `snapshot_json` column contains the same JCS text as the `snapshot_json` field of the `core` group.
  - **Purge:** when a record is physically deleted, the revision gets a tombstone with `cascade_of = the record's id` (see the next item). **Leaving `clock_skew` (§2):** a revision is part of the edit event, so it is restamped together with `body` (its `core` gets a new HLC equal to the new HLC of `body`).
  - **Size:** a revision whose envelope would exceed the 1 MiB limit (§5.3) stays local and does not go to the server; a decision for slice 11 (the edit is not blocked, the revision is local). **How "local" differs in slice 11:** in slice 3 there is no such flag — every revision has `server_known = 0` and, as soon as the outbox appears, would be a candidate for sending. When enqueuing, slice 11 builds the envelope and checks its size; if it is over the limit, it does not create an outbox row and records a "not sent: size" mark (a column or table in slice 11's migration; slice 3 has neither, and `message_revision` rows do not change). The revision stays `server_known = 0` without an outbox row and is not rechecked on every sync; it is not on the server and not on other devices, and that is acceptable: a revision is insurance for the author of the edit, not part of the record's state.
- **Purge and tombstones (slice 3):** a record from the trash (`deleted_at` is not `null`) is physically deleted — "Delete forever", or automatically after 30 days (`deleted_at ≤ now − 30 days`, at startup and once a day). In one transaction: `tombstone('message', id, purged_of, purged_at, NULL, NULL)` with a snapshot of the HLCs of **all** the record's groups (including unknown ones) in canonical JSON `{"<group>":"HLC"}`; for each revision of the record `tombstone('message_revision', id, {"core":"HLC"}, purged_at, cascade_of = the record's id, held = NULL)`; then the rows of the revisions, `message_tag` and the record itself are deleted. Nobody deletes tombstones in slices 1–10 (they are kept until binding and for no less than 180 days, §3.5).
  - **A partial `purged_of` snapshot.** If a record's `hlc_json` cannot be read entirely (a damaged group, an unreadable timestamp), the snapshot is built as far as possible: unreadable groups do not go into it, and the tombstone is written. A record whose `hlc_json` cannot be read at all is not deleted: it stays in the trash and shows an error, and the automatic purge pass moves on (the `purge_failed` flag is visible in "Settings"); a record whose `deleted` group is in an unsupported `sv` is not touched by automatic purge, and manually it gets `read_only`. **Consequence for slice 11:** a group missing from the snapshot is considered **newer** than the snapshot (the table of push responses in §A.3: `stale_purge` — "the server knows a group newer than the snapshot or a group missing from the snapshot"; for `purged` — "any known group newer than the snapshot → the record with its children goes to quarantine"). So a partial snapshot is safe: the worst outcome is that the record goes to quarantine rather than being silently deleted, and a human decides. But the snapshot must not be considered complete: the slice 11 client must not conclude from it that "no group of the record is left on the server".

### A.1. Envelope and DEK wraps — bytes

All integers are big-endian. Label strings are ASCII without a terminating zero. `account_id` and other UUIDs in the AD are 16 raw bytes.

**Envelope** (`format_version = 1`). It is stored in this same form in `change_outbox.envelope` and on the server.

```text
off    len     field
0      1       format_version = 0x01
1      4       key_id (u32), 1 = first DEK
5      4       header_len H (u32), 1 ≤ H ≤ 4096
9      H       header: UTF-8 JSON (§5.2)
9+H    24      nonce = randombytes_buf(24)
33+H   N+16    crypto_aead_xchacha20poly1305_ietf_encrypt(
                 m  = sodium_pad(payload, 1024), N is a multiple of 1024, 1024 ≤ N ≤ 1 MiB + 1024,
                 ad = AD_env, npub = nonce, k = K_env[key_id])
AD_env = "kinige.env.v1" (13 bytes) ‖ account_id (16) ‖ bytes[0 .. 9+H)
```

**DEK wrap** (`wrap_pw`, `wrap_rc`):

```text
off  len  field
0    1    wrap_version = 0x01
1    24   nonce = randombytes_buf(24)
25   48   crypto_aead_xchacha20poly1305_ietf_encrypt(m = DEK (32), ad = AD_wrap, npub = nonce, k = KEK_pw | KEK_rc)
total 73 bytes
AD_wrap = "kinige.wrap.v1" (14) ‖ wrap_version (1) ‖ account_id (16) ‖ key_id (u32, 4) ‖ kind (u8: 1 = password, 2 = recovery)
          ‖ if kind = 1: salt (16) ‖ kdf_profile (u8)
```

The profile and the salt are part of the AD: if the server substitutes them without a password change, the wrap will not decrypt.

**Parsing limits** — checked **before** allocating memory based on lengths:
- `H ≤ 4096`;
- the ciphertext length is a multiple of 1024 plus 16 and no more than `1 MiB + 1024 + 16`;
- the total length matches the length declared by the transport.

### A.2. Attachment blob

```text
P = sodium_pad(file, 1024)            # always ≥ 1 byte of padding → |P| ≥ 1024, an empty file = 1024 bytes
chunks = P cut into 1 048 576-byte pieces; the last chunk is shorter or equal, never empty
blob = header (24, init_push(blob_key))
     ‖ for each chunk: push(chunk, ad = empty, tag = MESSAGE | FINAL for the last one) → |chunk| + 17
```

Reading:
- `pull` by chunks of the same size plus 17;
- end of file before `TAG_FINAL` is an error; bytes after `TAG_FINAL` are an error;
- then `sodium_unpad`, and `sha256` and `byte_size` are checked against the `core` group.

Publishing the same `remote_blob_id` again gets the "already exists" response, which is a success. An unfinished upload can be overwritten; a finished blob is immutable. So a damaged blob is replaced only under a new `remote_blob_id` (§7).

### A.3. Push responses per `op_id`

| Response         | When                                                                                                              | Client                                                                                                                                                                                                                               |
|------------------|-------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `ok(seq)`        | accepted                                                                                                          | `acked_seq`, `server_known = 1`                                                                                                                                                                                                      |
| `duplicate(seq)` | same `op_id`, same bytes                                                                                          | same as `ok`                                                                                                                                                                                                                         |
| `op_conflict`    | same `op_id`, different bytes                                                                                     | client error, sync stopped, report to the user                                                                                                                                                                                       |
| `purged`         | the entity is already purged                                                                                      | by the condition in §3.5: any known group newer than the `purged_of` snapshot — the record with its children goes to quarantine; otherwise silent deletion; no local tombstone (no snapshot) — quarantine; a tombstone in every case |
| `stale_purge`    | the server knows a group of the entity newer than the `purged_of` snapshot (or a group missing from the snapshot) | pull, re-evaluate; the tombstones of this cascade's children are not sent                                                                                                                                                            |
| `clock_skew`     | `wall_ms` in any HLC of the header is greater than `server_now + SKEW_MAX`                                        | leaving `clock_skew` (§2)                                                                                                                                                                                                            |
| `too_old`        | protocol version below the minimum                                                                                | "update the client"                                                                                                                                                                                                                  |
| `stale`          | the pusher's cursor is behind a drop it missed, or is in a generation other than the current one (S4c)            | do not apply anything; pull, re-evaluate and run the cycle again. The envelope is not stored, so nothing needs undoing                                                                                                               |

**S4c, 2026-11-06:** a repeated `purged` is answered `ok` with the `seq` of the accepted one and its `purged_of` (`ok` row, ADR-0017); the row `stale` is new (ADR-0017, S4c items): the push request carries the pusher's cursor and `known_epoch`, and the server judges them by the rule of ADR §6.3, after `too_old` and before `clock_skew`. A repeat of an accepted envelope is still a `duplicate` first, whatever the cursor.

### A.4. Check order

**Server, push:**
1. Session and account; error → 401. **S4c, 2026-11-06 (this step and the next were swapped):** the shape of the bearer token is checked first (no database), then the session is looked up, **before a byte of the request body is read**. A client with no valid session gets 401 whatever it sends, and the server reads no body for it (the earlier order, lengths first, made the server read up to 16 MiB from a client that had no session; decided in ADR-0019).
2. Lengths (§A.1) and header parsing; error → 400. **S4c, 2026-11-06:** the server judges every envelope's declared length against the cap, then `Layout::parse` of the nine prefix bytes, then the header (`envelope::peek`'s routing fields, parsed strictly), **before it reads the envelope's body**; the whole request body is limited as it is consumed; the `entity`, `id` and `op` declared next to the envelope must equal the header's (`400 header_mismatch`). All envelopes of a request are judged before any is processed: one malformed one refuses the request.
3. `op_id` deduplication (`duplicate` or `op_conflict`) — **before** all purge rules, so that a repeat of an accepted envelope does not get `purged`.
4. `too_old`.
4a. (S4c, 2026-11-06) `stale`: the pusher's cursor `(generation, seq)` and `known_epoch` against the server's drops and lineage, the rule of ADR §6.3 — after `too_old` and before `clock_skew`; a repeat of an accepted envelope was already answered at step 3.
5. `clock_skew`.
6. The entity is already purged (`purged`) — for any `kind` except a repeated `purged`, which gives a no-op `ok` carrying the accepted snapshot (S4c, 2026-11-06).
7. For `purged`: the `stale_purge` check — each group of the `purged_of` snapshot against the maximum HLC of that group from the entity's headers.
8. Assigning `seq` under the account lock, writing.

**Client, pull:**
1. The cap on the whole envelope (`MAX_ENVELOPE` = 9 + 4096 + 24 + 1 MiB + 1024 + 16) and nothing else about lengths (S4b, 2026-11-06). A future format must fit within this cap or an old client answers `TooLarge` and not `pending_unknown` (ADR §5.4).
2. `format_version`; unknown → `pending_unknown` with `entity` and `id` from the transport, the cursor moves. Then the length rules of version 1 (§A.1: `header_len`, the ciphertext multiple of 1024 plus 16, the declared total), which are not applied to another format. Then `key_id`; unknown → `pending_unknown` the same way. On a full resync such a record counts as "is on the server".
3. AEAD; error → the page is not applied, the cursor stays, the error is visible to the user.
4. Matching the authentic header's `entity`, `id`, `op` and group stamps against those declared by the transport. **A mismatch is handled like an AEAD failure (S4b, 2026-11-06):** the page is not applied, the cursor stays, the error is visible; the server served a genuine envelope under another record.
5. Strict parsing: HLC, `sodium_unpad`, JSON without duplicate keys, the payload's set of groups equals `header.groups`, the nesting of a group's value is no more than 127 nested arrays and objects (the `serde_json` parsing limit, `remaining_depth` = 128; the Rust writer does not write deeper either, `jcs::MAX_DEPTH`; a web client on `JSON.parse` has no limit and must check the depth itself). Error → quarantine with the reason `malformed`, the cursor moves, the error is visible.
6. The clock receives all HLCs of the header (§2).
7. Tombstone and the purge rules (§3.5).
8. Merge, the page and the cursor — in one transaction.

**Logs** (§8.8): the request bodies of registration, login, recovery and password change are not logged; tokens and `auth_key` — never, hashes are allowed. (S4c, 2026-11-06: nor what a dependency prints, see ADR §8.8.)

### A.5. Repeating registration and recovery (§8.3, §8.6)

Without a typed-back check, not only a failure before the request is dangerous but also a lost response: the client restarted, built new keys and words, and the saved PDF belongs to nobody. Therefore:
- the client saves the **prepared package** (`account_id`, login, `salt`, profile, `auth_key`, `rc_auth_key`, `wrap_pw`, `wrap_rc` and the DEK; without the password and the words' entropy) in the OS secret store **before** showing the words, and after a restart repeats the same request until it gets a definite answer. The DEK is in the package so that after a restart the client can work without entering the password again; it lives where it normally does (§8.8). **The KEKs are not in the package (S4b, 2026-11-06):** the wraps are bound to the `account_id`, so after an `account_id` clash the client cannot rebuild them from the package; it asks for the password and the words again (the words on the PDF), and a restart in the middle of that changes nothing else. The clash is a v4 UUID collision, so this path is for a hostile or broken server;
- the server is idempotent (S4c, 2026-11-06: it compares the repeated request with the stored columns, which are the hashes themselves, so no record of a request ever holds a raw key, and compares the keys' hashes with `sodium_memcmp`; a package that was applied and then changed by a later password change or recovery is no longer "the same package", which is the stricter reading):
  - registration — first the `account_id` check: the same package → success, `account_id` taken by another package → a separate error (the client creates a new `account_id` and rebuilds the wraps, §1, the words stay the same); only then whether the login is taken — otherwise a repeat after a lost response would get "login taken";
  - recovery — by the new `rc_auth_key` **and equality of the rest of the package**: the replacement was already made by the same package → success, by another package → a separate error;
- if the outcome of a recovery is uncertain after a restart (the old `rc_auth_key` is no longer in memory), the client tries to log in with the new password: success = the replacement went through; refusal = the old words have to be entered again (S4b, 2026-11-06: a refusal by the profile rule counts as a refusal: a server that still sends a lower profile than this device has seen has not applied the replacement, since a replacement made here gives the new password at least that profile; the same login settles a lost answer to a password change, §8.5);
- "Save" counts only after a successful file write or sending to print; cancelling the dialog does not count;
- if the login is taken, the user picks another login, while `account_id`, the keys and **the words do not change** (`rc_root` does not depend on the login);
- after a definite answer, the package is deleted from the store.

### A.6. HTTP API (S4c, 2026-11-06)

The shapes `server/` implements (`server/src/dto.rs`, `server/src/wire.rs`; ADR-0019). Everything is under `/v1`, over TLS terminated by a reverse proxy (S4c-2). Nothing here changes a byte of §A.1.

**Conventions.**
- JSON bodies are UTF-8. Keys, salts, wraps and tokens are **base64url without padding** (RFC 4648 §5) in strings. A **generation** is a decimal string (it is a random 63-bit number, and JSON numbers are not safe above 2^53); `seq`, `epoch`, `after`, versions and sizes are numbers below 2^63.
- A session is `Authorization: Bearer <token>`, the 32 random bytes of §8.4 in base64url. A missing, malformed or unknown token is `401 {"error":"auth"}`, the same as a wrong key.
- An error is `{"error":"<code>"}` with a status: `400` the request is not what the protocol allows (the code names the rule, never the content), `401 auth`, `409` the state refuses a well-formed request, `413 too_large`, `429 rate_limited` with `Retry-After`, `500 internal` only for a database failure, never for a malformed request. Three answers do not come from the handlers and carry the framework's plain body instead of `{"error":…}`: an unknown route `404`, a known route with another method `405`, and a JSON body above 16 KiB `413`. A client reads the status first.
- Account bodies are at most 16 KiB. Logins are normalized as in §8.3 and at most 254 characters.
- A client sends `X-Kinige-Protocol: <n>` (the HTTP protocol version of §4, now 1) on a push; a missing header is version 0. The server reads this header only on a push; a pull and the account requests ignore it.
- `GET /healthz` → `200 ok`, no session, no database: the process is up.

**Accounts and sessions** (bodies are the structs of `dto.rs`; a field named `…_key` is 32 bytes, a `wrap_…` is the 73 bytes of §A.1):
- `POST /v1/register` `{account_id, login, salt (16), profile (1|2), auth_key, rc_auth_key, wrap_pw, wrap_rc, key_id}` → `200 {token}`; `409 account_id_taken` (another package behind this id), `409 login_taken`; a repeat of the same package is `200` with a new session (§A.5).
- `POST /v1/login/params` `{login}` → `200 {salt, profile}`, for any login: an unknown one gets the §8.4 salt and profile 1. Not rate limited, so it says nothing else: **no credential version** (it would tell an account that changed its password from one that did not). The version comes with `POST /v1/login`, and a password change names that one.
- `POST /v1/login` `{login, auth_key, replaces?}` → `200 {token, account_id, key_id, wrap_pw, cred_version}`; `401 auth` for a wrong key and for an unknown login, with the same body and the same log lines; `429` after the budget. `replaces` is a token of the same account that is ended in the same transaction.
- `POST /v1/logout` (session) → `200 {}` and the session is ended; `401` for a token that is not a session.
- `POST /v1/password/change` (session) `{auth_key (old), expected_version, new: {salt, profile, auth_key, wrap_pw}, revoke_others}` → `200 {cred_version}`; `409 version_conflict` with `current_version`; `401 auth` for a wrong old key; `409 profile_downgrade`.
- `POST /v1/recovery/fetch` `{login, rc_auth_key}` → `200 {account_id, key_id, wrap_rc, profile}`; `401 auth` for a wrong key and an unknown login alike; `429` (shares a budget with the next).
- `POST /v1/recovery` `{login, old_rc_auth_key, new: {salt, profile, auth_key, wrap_pw}, rc_auth_key, wrap_rc}` → `200 {token, cred_version}` and every other session is ended; a repeat of the same package is `200`; `409 package_mismatch` (the new `rc_auth_key` is already in force behind another package), `409 profile_downgrade`, `401 auth`.
- `POST /v1/words/reissue` (session) `{auth_key, new_rc_auth_key, wrap_rc}` → `200 {}`; `409 package_mismatch`; `401 auth`.

**Push** `POST /v1/push` (session, checked first: a request without a well-formed bearer token or with an unknown one is `401` and its body is not read), headers `X-Kinige-Cursor: <generation>.<seq>.<known_epoch>` (required) and `X-Kinige-Protocol`. The body is a run of **frames**, big-endian, at most 16 MiB and 1000 frames, never empty:

```text
u8  entity_len (1..=32) | entity (ASCII) | id (16 raw bytes) | op (16 raw bytes) | u32 envelope_len | envelope (§A.1, as it is stored)
```

The server reads the declared `envelope_len` against `MAX_ENVELOPE` (`413`), the nine prefix bytes with `Layout::parse`, and the header with the strict parser before it reads the rest of the envelope; an unknown `format_version` is `400 unsupported_format` (the server cannot read another format's header); a header that disagrees with the declared `entity`, `id` or `op` is `400 header_mismatch`. The answer is `200 {"results":[{"op","result","seq"?,"purged_of"?}]}`, one per frame in order, where `result` is `ok`, `duplicate`, `op_conflict`, `purged`, `stale_purge`, `clock_skew`, `too_old` or `stale` (§A.3); `seq` is present for `ok` and `duplicate`; `purged_of` for the `ok` of a repeated `purged`. The envelopes of one request are processed one by one, each in its own transaction under the account lock. **A failure in the middle** (a database error) leaves the envelopes before it committed and answers `500` with no results: the client does not know which went in, pushes the request again, and the ones that did are `duplicate`, which is harmless.

**Pull** `POST /v1/pull` (session) `{generation, after, known_epoch, limit?}` → one of:
- `{"result":"page","generation","after","next","head","epoch","items":[{"seq","entity","id","op","envelope"}]}` — `envelope` is base64url of the stored bytes, `entity`, `id` and `op` are what the server declares for it (the client compares them with the authentic header, §5.4); `next` is the last `seq` of a truncated page, otherwise `head`; the page is at most `limit` (default 200, at most 1000) envelopes and stops once 8 MiB are in it, the first envelope always goes, and a `limit` or a byte budget of 0 is raised to 1 (a page is never empty by configuration). **The cursor moves to `next` even when `items` is empty** (at the head `next` is `head`), and when `after` is above `head` `next` is `after`, so a cursor never moves backwards. `limit` is clamped to 1..1000, not refused;
- `{"result":"stale","generation","epoch"}` (§6.3);
- `{"result":"generation_changed","generation","stale","epoch"}` (§6.3).

## B. Test plan (S4a — protocol, S4b — envelope and keys)

Everything except the test vectors is property tests: 2–3 clients in one process, with random delays, duplicates, reorderings and disconnects (PRD §14). The criteria of PRD §10.2 S4a are included here without repetition.

**S4a — sync**

1. Convergence of the records' state under any delivery order; a repeated push is idempotent.
2. HLC string comparison matches tuple comparison; counter overflow is monotonic.
3. A restart after receiving a "future" timestamp does not produce repeated HLCs.
4. The clock ticks from envelopes dropped by `op_id` or by a tombstone: the next local timestamp is greater.
5. Two offline edits on one device (with and without outbox compaction), then sync — `conflict` revisions are **not** created. The same for two offline moves (`placement`).
6. A real concurrent edit — exactly one `conflict` revision with the same id and content on all devices. The same for a concurrent move into different chats (`group = 2`); a concurrent `flags` edit gives no revisions.
7. A text conflict preserves both versions (PRD).
8. The first binding of two databases with records older than 180 days, both with an Inbox — everything goes to push, the quarantine is empty, there is one Inbox.
9. Purge without an account, then binding of a database copy — `purged` goes out, the record is not resurrected.
10. Restore versus purge in both `seq` orders, plus a dishonest server order (purge accepted after the restore) — the client loses neither the record nor its children, no divergence.
11. A purge ack with a concurrent local restore — the record is not silently deleted, it goes to quarantine.
12. A `purged` response to a push of a live edit — the record with its children is in quarantine at the author's, the others do not have it, the feeds match.
13. A child arrives after its parent's `purged` — deleted locally, `purged(child)` sent, a repeat is a no-op.
14. A device came back after 200 days with an edit of a deleted record — quarantine, no resurrection (PRD).
15. Server restore from a backup (`generation`) with an unconfirmed outbox — everything arrived, the `server_known` records are back on the server.
16. `pending_unknown` → client update → applying gives the same state as applying directly.
17. Mixed `sv` per group: an old client edits `flags`, a new one edits `body`; both edits survive, and the unknown group is not lost when the old client resends.
18. A client with its clock 25 h ahead — push rejected; after the clock is fixed, sync recovers without losing local edits, `base` and revision ids are recomputed.
19. A restore from the trash reaches another device (PRD).
20. Deleting a chat with a late move of a message into it — the message is shown in Inbox with a mark (derived), nothing is lost, no `placement` envelopes arose.
21. A database copy with the same `device_id`: an HLC tie is resolved deterministically, the loser's body is in a revision.
22. An envelope that passes AEAD but fails parsing — `malformed` quarantine, sync continues.
23. Server: parallel pushes of one account — the cursor does not skip records (S4c). **S4c-1, 2026-11-06: covered** by `server/tests/concurrency.rs`, with a negative control (`seq` from a plain sequence skips records); see `server/README.md`.
24. The first binding of a database copy: a record edited on the copy and purged on the original goes to quarantine and is not deleted; an untouched copy is deleted silently.
25. Two live instances with one `device_id` edit one message with different HLCs — both versions are kept, there is a `conflict` revision (`supersedes` is empty).
26. A new client (`sv = 2`) resends a snapshot with an old client's `body` — on the old client there is no integrity error, no "read-only" mode, no revision; the `body` bytes are verbatim the same.
27. Chat purge: `purged(chat)` arrives before the `purged` of the messages (the normal order, §3.6), or the page is split between them — the recipients get zero `placement` envelopes and zero quarantine records; the chat's old messages in between are hidden, not shown in Inbox.
28. A password change on device X, then logging in again on Y — login works; the profile is not downgraded either on a password change or on recovery.
29. The server sent a profile lower than the one seen — refusal with an explanation, recovery with the words works.
30. A `generation` change with local tombstones — the tombstones are resent, devices without a tombstone do not keep the record in the trash.
31. A full resync with a `pending_unknown` for a record — the record is not in quarantine.
32. A crash in the middle of generating 100k envelopes at the first binding — after a restart there are neither duplicates nor gaps.
33. Leaving `clock_skew` with unconfirmed envelopes within `SKEW_MAX` — they are repeated as is (`duplicate`), only the rejected ones are restamped, together with `base` and `supersedes`.
34. Purge without an account, binding of a copy after 200 days — the tombstone survived, the record was not resurrected.
35. A late message of a purged chat stays visible in Inbox after the chat's tombstone expires, and on a new device after compaction of the server's `purged(chat)`.
36. A device that is two `body` versions behind edits `flags` — the others get zero `conflict` revisions.
37. 65+ offline edits without outbox compaction — no more than one extra revision, not a single lost version.
38. The invariant `canonical(columns) == groups_raw[g]` for the groups this client wrote, after any local edit (property).
39. A message created by an agent or by an import keeps its `origin` after edits and sync.
40. Chat restore versus chat purge: an online race (the purge was sent based on a stale pull) and an offline restore in both `seq` orders — the chat with its messages is either alive everywhere or entirely in quarantine at the device that restored it; there is no restored empty chat.
41. The chat purge initiator disappeared after the ack of `purged(chat)` before the cascade — the messages are hidden, not lost; after the chat's tombstone expires they are visible in Inbox with a mark.
42. A device with an unsent `body` edit came back after 200 days, and the text was edited twice on other devices in the meantime (the intermediate version was compacted) — its own version is in a `conflict` revision. The same at the first binding of a database copy.
43. An equal HLC with different bytes in `flags` or in a chat group — no revision, an event in the app log without content.
44. A `conflict` revision from two devices with different client versions — the `snapshot_json` bytes match (`raw` as a string); with an unsupported `sv` of version P, only a device that supports it creates the revision.
45. A blob is damaged on the server → "upload again" on a device that has the file — a new `remote_blob_id`, the other devices get the new `core` and read the file, the old blob is deleted.
46. Re-importing the same Telegram export — zero new records, the report says "skipped N"; on a second device after sync — also zero.
47. An unbound copy does a local purge of a message with an attachment, the message is restored on the bound database, then the copy is bound — the attachment and the blob are intact, the children's tombstones are lifted, the attachment is recreated from `held`. The same for a chat with messages and on a `generation` change; the same on a `stale_purge` of the parent in a push race. After `ok(seq)` on `purged(child)` its `held` and `cascade_of` are empty, and a later lift of this child's parent's tombstone does not resurrect it.
48. A message is moved into a chat after the chat's deletion (D3), the chat is restored offline (D1) and purged (D2) — the moved message does not go into D1's quarantine and is not deleted at D2 on the ack; it is in Inbox for everyone. Returning the chat from quarantine: messages without a tombstone are moved via `placement`, those with a tombstone as a copy; no duplicate live messages.
49. Moving a message out of a chat within the purge window: D1, offline since before the chat's deletion, moved M into X (the HLC of the move is less than the HLC of the deletion) and synced between the ack of `purged(chat)` and `purged(M)` — the server answers `stale_purge` to `purged(M)`, the initiator does not repeat it after pull, and M is alive in X everywhere. A dishonest server accepts `purged(M)` after the move — D1 takes M into quarantine rather than deleting it silently. At the initiator, after M's tombstone is lifted, there is no `purged(M)` outbox row.
50. A `purged` response to a push without a local tombstone (the server accepted someone else's `purged(X)` between pull and push) — X is in quarantine, a tombstone is written; the next pull of `purged(X)` refines the snapshot and is otherwise a no-op.

**S4b — envelope and keys**

*Status (S4b, 2026-11-06): items 1–14 are covered, each by named tests, in the map in `spikes/S4b-keys/README.md`; the vectors are `crates/kinige-crypto/vectors/` (made by libsodium.js, regenerate with `spikes/S4b-keys/vectors/gen.mjs`).*

1. Test vectors with a fixed key, nonce and input and the expected bytes for: the envelope, `AD_env`, both wraps with `AD_wrap`, the `crypto_kdf` subkeys, padding, the id of a `conflict` revision and its `snapshot_json` (`raw` as a string), blobs of 0 bytes, 1 MiB − 1, exactly 1 MiB, 1 MiB + 1.
2. Substitution of each open field (HLC, `purged`, `sv`, `key_id`, `format_version`, `account_id`) and moving an envelope to another record are detected (PRD).
3. A too large `header_len` or length is rejected before memory is allocated.
4. A truncated blob, a blob without `TAG_FINAL`, bytes after `TAG_FINAL` — error.
5. Password change without re-encryption; recovery with the 24 words (PRD).
6. After recovery the old words are invalid; reissuing the words works.
7. The server sent a profile lower than the one seen, or an unknown one — the client refuses before deriving the key.
8. Unknown login: a 16-byte salt, the same profile; recovery responses are indistinguishable.
9. Mnemonic → `to_entropy` (not `to_seed`). A typo into a non-dictionary word gives `UnknownWord`; a substitution with another dictionary word is caught by the checksum with probability 1 − 2⁻⁸ (24 words have 8 checksum bits). Upper-case input is normalized before parsing.
10. The server and client logs after a run contain no `auth_key`, `rc_auth_key`, tokens, text.
11. Build of the chosen libsodium binding on macOS, Windows, Linux — at the start of S4b.
12. Canonical JSON: the same group value from Rust and from a libsodium.js client gives the same bytes (a test vector).
13. A client crash between showing the new words and the replacement request on recovery — nothing changed on the server, the old words still work. Without pressing "Save" (a successful file write or print), the replacement (and registration) request does not go out. The response is lost and the client restarted — a repeat with the same package gives one account, and the words from the PDF work; with an uncertain recovery outcome, logging in with the new password confirms the replacement. A taken login — the words stay the same.
14. Recovery with the words on a device with `kdf_profile_seen = 2` when profile 1 is sent — the new password gets profile 2, and the device logs in with its new password without a refusal.

## C. Consequences for the schema of PRD §4.2

PRD §4.2 is a draft for the ADR. Slice 1 needs:
- **in all synchronized tables:**
  - `hlc_json` of the form `{"<group>":{"h":"HLC","sv":1}}`;
  - `server_known INTEGER NOT NULL DEFAULT 0`;
  - `groups_raw` of the form `{"<group>":<raw canonical bytes>}` — the source for resending; replaces the draft's `unknown_json`, because unknown groups and fields live there too;
  - field columns — the parsed representation, migrated if the `sv` is old;
- `message`: `base`, `supersedes` in `body` and `placement`; `origin TEXT NOT NULL DEFAULT 'user'` and `ext_json TEXT` (the `core` group);
- `message_revision`: `hlc_json` (`core` only), `schema_version`, `groups_raw`;
- `attachment`: `blob_key`, `groups_raw`; `remote_blob_id` — UUID v7 (§7);
- new tables:
  - `tombstone(entity, id, purged_of, purged_at, cascade_of, held)`: `purged_of` is a JSON snapshot `{"<group>":"HLC"}` (§3.5); `cascade_of` is the id of the cascade's parent (the message for attachments and revisions, the chat for its children), `NULL` at the root; `held` is the held snapshot of a child that arrived while the tombstone is alive (§3.5): the merged groups in the form of `groups_raw` + `hlc_json`, not envelope bytes; it exists only on tombstones with `cascade_of`, and is `NULL` on those confirmed by the server. `cascade_of` and `held` are cleared on `ok(seq)` for `purged(child)`, when the tombstone is lifted, and at a recipient of `purged(child)` for an existing tombstone with `cascade_of`: the child's purge is confirmed by the server, and lifting the parent no longer resurrects it. Lifting a parent's tombstone goes recursively over `cascade_of` and affects only children with a non-empty `cascade_of`. Sending tombstones at binding and on a `generation` change — parent first (§6.2);
  - `quarantine(id, entity, entity_id, payload, reason, created_at)`;
  - `pending_unknown(seq, entity, entity_id, envelope)`;
  - `message_ext(src, ext_chat, ext_msg, message_id)`, primary key `(src, ext_chat, ext_msg)` — a local non-synchronized index, derived from `core`, recomputed on pull. Insertion is `INSERT OR IGNORE`: two messages with different ids and the same `ext` (a concurrent import on two devices) do not break the recomputation; the duplicate shows up in the import report and in the UI;
- `change_outbox`: `acked_seq`;
- `sync_state`: `device_id`, `hlc_l`, `hlc_c`, `generation`, `cursor`, `known_epoch` (S4a: the `GenerationChanged` answer carries the new `generation` and the server's current `epoch`, which the device stores as its cursor generation and as `known_epoch`; the drop `epoch` of the last applied pull page, sent with the next pull, replaced by the server's current epoch from the answer when a resync starts because the `generation` changed; §6.3), `account_id`, `kdf_profile_seen`. A pull page carries `epoch`; a `GenerationChanged` answer carries `generation`, `stale` and `epoch`.

Slice 1 has no sync, but `hlc_json`, `sv = 1`, `device_id`, the HLC clock and local tombstones work from the very first write.

### C.1. Slice 1 refinement (21.10.2026)

- **The Inbox constant:** `00000000-0000-4000-8000-000000000001` (UUID v4: version 4, variant 8). Inbox's `core` group is `{"created_at":0,"is_inbox":true}` (the `created_at` column of `chat` is 0 too); the groups `meta` (`{"color":null,"icon":"📥","title":""}`, the stored name is empty — the UI shows the translated one), `order` (`{"sort_key":"a0"}`), `state`, `privacy`, `deleted` are created on first launch together with `core` under one HLC. Idempotent: Inbox is identified by id.
- **Migration 1 has only the tables slice 1 writes to:** `chat`, `message`, `sync_state` (`device_id`, `hlc_l`, `hlc_c`), plus the local non-synchronized `app_setting` (`mirror_dir`, `text_size`) and `mirror_dirty(chat_id, journal_date, seq)`. The other tables of this section (`tombstone`, `message_revision`, `message_ext`, `attachment`, `change_outbox`, `quarantine`, `pending_unknown`) will be added by their slices with `CREATE TABLE` migrations without changing existing rows: while nobody writes to them, an empty table protects nothing. The columns that rows depend on (`hlc_json`, `server_known`, `groups_raw`, `schema_version`, `origin`, `ext_json`, `base`/`supersedes` of `body` and `placement`) exist on `message` from the very first write; there is no `body_search` column (decided in slice 4, see §C.3: the search index is separate local tables `search_fts` and `search_map`; `message_ext` was added by migration 4).
- **A new record** is created with all five groups `core`, `body`, `flags`, `placement`, `deleted` under one HLC (one event) and `sv = 1`; the group bytes are JCS (RFC 8785, keys by UTF-16 code units), and the columns are built from the same values.

### C.2. Slice 3 refinement (27.10.2026)

- **Migration 3** (`CREATE TABLE` and an index; rows of synchronized tables do not change): `message_revision(id, message_id, snapshot_json, saved_at, reason, hlc_json, server_known, schema_version, groups_raw)`; `tombstone(entity, id, purged_of, purged_at, cascade_of, held)` with the primary key `(entity, id)`; the local non-synchronized `message_tag(message_id, tag)` (derived from `body_md`, recomputed in the migration by Rust code) and `mirror_chat(chat_id, folder)` (which folder of the Markdown copy holds the chat's files); a partial index `message_deleted`. Before migrating an existing database, `PRAGMA integrity_check` is run synchronously and a backup is taken (`backups/pre-migration-v<schema>-<UTC>.db`, via the SQLite backup API); no backup — no migration.
- **A new chat** is created with all six groups (`core`, `meta`, `order`, `state`, `privacy`, `deleted`) under one HLC and `sv = 1`; `order.sort_key` is `a` and six base-36 digits (`a000001`, …), Inbox is `a0`. Equal keys on two devices are acceptable: `id` decides the order.
- **Database file backup and restore** (local, not synchronized): `backups/daily-YYYY-MM-DD.db` (the 7 latest), `backups/pre-migration-…` (the 3 latest). A restore always creates a new `device_id` (§1); a damaged file is not deleted but set aside as `corrupt-<ms>.db`. Next to the database lies a marker file `needs-recovery`: empty — the background check found damage (at launch the synchronous check rechecks it); with the content `restoring` — a restore was started and not finished (at launch the window will offer the backups). The Markdown copy folder named in the settings of the backup itself (or the default folder) is renamed FIRST, before any database file is moved, to `<name>-before-restore-<UTC>` (on a name clash, the suffix `-2`, `-3`); if the rename fails, the restore does not start and nothing is touched. A retry after a failure that happened after the first file move (the `restoring` marker) does not move the folder a second time and returns the path of the latest `…-before-restore-…` next to it in the command's response (`previousMirror`); the window does not show this path, it has a generic text about "…-before-restore-…" without the path.
- **The name of a chat folder in the Markdown copy:** the title, stripped of `/ \ : * ? " < > |` and control characters, up to 80 characters; case-insensitive clashes are distinguished by the suffix ` (<8 characters of the id>)`, and the plain name stays with the oldest chat; Inbox is `Inbox`.

### C.3. Slice 4 refinement (28.10.2026)

- **Migration 4** (`CREATE TABLE`; rows of synchronized tables do not change): the local non-synchronized `message_ext(src, ext_chat, ext_msg, message_id)` (`WITHOUT ROWID`, primary key `(src, ext_chat, ext_msg)`, an index on `message_id`) and the search tables `search_map(rowid, message_id UNIQUE)` and `search_fts` (FTS5 without stored text, `contentless_delete=1`, `unicode61 remove_diacritics 0`; the decision is ADR-0008). `after_migration(4)` builds both from existing records: `message_ext` from `ext_json` (`INSERT OR IGNORE`; a damaged `ext_json` yields no rows), the index from `body_md` of all records, including those in the trash (the query filters out the trash). The version of the index rules is `app_setting.search_index_version`; on a mismatch, the index is rebuilt on open. Before migrating an existing database, a backup `pre-migration-v3-<UTC>.db` is taken (C.2).
- **Deviation from the §4.2 draft:** there is no `message.body_search` column. The index is a separate derived table: there is no need to change rows of a synchronized table for derived data. ADR §C (the first paragraph of the list) and appendix §A.0 already call `body_search` derived.
- **The index is updated in the same transaction** as any change of `body_md`: send, append, edit and a `- [ ]` checkbox, import. Delete forever (`purge`) deletes the index row and all the record's `message_ext` rows in the same transaction; so a re-import brings such a record back as new (the report says "new"). Revisions (`message_revision`) do not go into the index.
- **The `core` group of an imported record** (bytes are JCS, keys by UTF-16 code units; an example for chat 9223372036854775807, message −5):
  `{"created_at":1552000000007,"created_tz":"Europe/Moscow","ext":{"chat":"9223372036854775807","msg":"-5","src":"telegram"},"origin":"import_telegram","reply_to_id":null}`.
  `ext.chat` and `ext.msg` are strings matching `^-?[0-9]{1,20}$` (Telegram ids are 64-bit, JCS is limited to ±2⁵³). `reply_to_id` is the id of a Kinige record (UUID v4) if the replied-to message of the same Telegram chat has already been imported, otherwise `null`: `core` is immutable, and the field cannot be filled in later. The `message.ext_json` column holds the same canonical bytes of `ext`. The other four groups (`body`, `flags`, `placement`, `deleted`) are byte for byte the same as for a user's record, `groups::new_message`.
- **HLC in bulk writes:** each imported record gets its own new HLC (`next_hlc` at the time of import; the clock continues from the last value, so in a batch of 500 records the HLCs strictly increase), one for all five groups, `sv = 1`, `server_known = 0`. The original time from Telegram goes into `created_at` and `journal_date`, not into the HLC: this is the "except for imports" exception of §10 (the server must not learn the writing time from the HLC, and for an import it would be fake). A batch is one writer transaction; between batches the writer is free.
- **Backup before an import:** `backups/pre-import-<UTC>.db` (`BackupKind::PreImport`), the 3 latest are kept; it is taken the same way as the daily backup and is visible in the "The database is damaged" window.
- **Order within a second:** `created_at` = `date_unixtime` × 1000 + k ms, where k is the ordinal number of the message within that second in the file (no more than 999); otherwise the random record `id` would shuffle them in the feed.

### C.4. Slice 4a refinement (28.10.2026, MCP, ADR-0009)

- **Migration 5** (`CREATE TABLE`; rows of synchronized tables do not change): the local non-synchronized tables of the agent access log.
  - `mcp_log(id INTEGER PRIMARY KEY AUTOINCREMENT, at, client, tool, args_json, outcome, entry_count)` — `entry_count` (the number of entries in the request) is written in the same transaction and does not change on `purge`; one row per request: a tool call, a connection (`tool = 'connect'`), a disconnection (`'disconnect'`), a reconnection of the bridge after a drop (`'reconnect'`).
  - `mcp_log_message(log_id, message_id)` (`WITHOUT ROWID`, primary key `(log_id, message_id)`, an index on `message_id`) — the ids of the records the request touched.
  - Before migrating an existing database, a backup `pre-migration-v4-<UTC>.db` is taken (C.2). Neither the access settings (`app_setting`: `mcp_enabled`, `mcp_mode`, `mcp_chats`) nor these tables go into slice 11's `outbox`.
- **An access log without content.** `args_json` is built from an **allowlist** of fields of the parsed arguments (`kinige_domain::agent::args_json`): `chat_id`, the dates `from`/`to`/`date`, the flags `favorites`/`tasks`/`task`/`done`, the item number `item`, the `page` marker, `query` no longer than 128 characters. The `text` field and record ids are never there: ids go into `mcp_log_message`. Property test: for any arguments of any tool, entry text does not get into `args_json`. `client` is the client's own words (`clientInfo.name`): control characters removed, no longer than 64 characters, not presented as verified. The outcome (`outcome`): `ok`, `disabled`, `read_only`, `not_found`, `no_agent_entry`, `not_a_task`, `invalid_argument`, `error`.
- **Transactions.** A data change and the access log row are one writer transaction; a refusal rolls back everything except the row about the refusal (it is written in a separate transaction of the same writer job). Reads: the query on the read connection, then a writer transaction with the access log row and the ids of the records included in the response, then the response; if the commit fails, the agent gets an error, not data.
- **Cascades.** A `purge` of a record (`trash::purge`) deletes its `mcp_log_message` rows in the same transaction; the `mcp_log` row stays. Rotation during maintenance (`run_maintenance`): rows older than 90 days and beyond 20 000 (the newest stay), then `mcp_log_message` rows without a parent, all in one transaction.
- **The `core` group of an agent's record** (bytes are JCS; no `ext`; `origin` is immutable, the value is additive, `sv` does not grow):
  `{"created_at":1760000000000,"created_tz":"Europe/Moscow","origin":"agent","reply_to_id":null}`. The other four groups are as for a user's record (`groups::build_message`), all five under one HLC, `sv = 1`.
- **The `flags` group with `task = true`** is created right away, in the same transaction and under the same HLC as the other groups: `{"favorite_at":null,"is_task":true,"pinned_at":null,"task_done_at":null}`. The `is_task` column is built from the same values. An agent may mark as done (`task_done_at`) any task entry of a visible chat, but cannot turn someone else's entry into a task.
- **An exception to §A.0: an agent's append writes an `edit` revision**, unlike ⌘Enter (ADR-0006). The user may have edited the agent's entry: the version before the append is saved as a revision (`reason = 'edit'`, a full snapshot, as with `edit_message`), then the `body` group is rewritten (§3.3, §3.4). The target is the latest live entry of the chat in `(journal_date, created_at, id)` order; "own" is determined by `origin = 'agent'`, not by the client name: client B appends to client A's entry. The capture window does not append to an agent's entry (`origin ≠ 'user'`).
- **Data access.** The visible set of chats: the chat is in `mcp_chats`, it does not have `ai_excluded`, it is not deleted; the archive is visible. The predicate (`json_each` over the `app_setting` value) is in **every** read statement of the agent and inside the write transaction; the "Chats" row of search is a separate statement with the same predicate. Everything outside the set, deleted, in the trash and non-existent gets the same `not_found` (including Inbox by default, if it is not marked).

### C.5. Slice 5 refinement (31.10.2026, "On this day" and replies)

- **Migration 6** (`ALTER TABLE`; rows of synchronized tables do not change): `ALTER TABLE draft ADD COLUMN reply_to_id TEXT`. `draft` is a local non-synchronized table (C.1, migration 2); existing drafts get `NULL`. A draft row is deleted only when both the text and `reply_to_id` are empty. Before migrating an existing database, a backup `pre-migration-v5-<UTC>.db` is taken (C.2).
- **The `core` group of a user's reply.** The first non-null `reply_to_id` in a user's record. Bytes (JCS, keys by UTF-16 code units; no `ext`):
  `{"created_at":1793000000000,"created_tz":"Europe/Moscow","origin":"user","reply_to_id":"5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d"}`.
  The other four groups are byte for byte as for a user's record (`groups::build_message`), all five under one HLC, `sv = 1`. `core` is immutable: the link is neither edited nor removed; edit, move, date change and append do not touch it.
- **Check before writing (Rust, in the writer transaction).** `reply_to_id` is a lowercase UUID v4; a record with this id exists, is in **the same chat** as the reply, and is not in the trash. Otherwise the error `reply_target_not_found`, nothing written. The capture window and the agent (MCP) do not create replies and do not accept `reply_to_id`.
- **A link without a foreign key, three states of the old record.** On display: `live` (the record is in place, including in another chat after a move), `trash` (`deleted_at` is not empty: the quote is labeled "Entry in the trash"), `missing` (there is no record: deleted forever, or on pull it has not arrived yet; "The original entry was deleted"). A link to an entry of an `ai_excluded` chat is stored in the reply the same way; texts of the other chat get only into the UI quote, and `reply_to_id` is not given to the agent (ADR-0009). In sync, a link to a record that has not arrived yet does not break applying: it is `missing` until the record arrives.
- **Slice 11 (sync), what to take into account in advance.** On pull, `missing` may mean not "deleted" but "has not arrived yet": then the quote label must be different (the current "The original entry was deleted" would be wrong). When applying a received record, the days of local replies to it must be marked dirty (`mirror_dirty`), otherwise the `↩` line in the Markdown copy stays without a link; this will need an index on `message.reply_to_id`, which does not exist now (selecting the replies to a record is a full scan, so slice 5 does not do it).
- **The date of someone else's envelope (slice 11).** The card and MCP `on_this_day` take the year of the oldest entry from `journal_date` via `year_of`; if an applied envelope brings a record with a non-date in `journal_date`, that year is skipped, and if it is the oldest value, the card and `on_this_day` become empty. So the `journal_date` format (`is_valid_journal_date`) is checked when an envelope is applied, before writing to `message`.
- **Draft and a moved reply target.** The line above the input field of a draft gets the `moved` state if the record is alive but is in another chat (a reply is accepted only to a record of the same chat). In the feed and in search results such a record stays `live`: the `moved` state exists only in `chat_draft`.
- **The agent access log** shows entries without `reply_to`: the agent did not see the link or the quote (ADR-0009).
- **Import.** The `reply_to_id` of an imported reply (C.3) is shown the same way as for user replies.

### C.6. Slice 7 refinement (03.11.2026, attachments, ADR-0014)

- **Migration 7** (`CREATE TABLE` and `ALTER`; rows of synchronized tables do not change):
  - `attachment(id, message_id, kind, sort_index, sha256, filename, mime, byte_size, width, height, duration_ms, url, link_title, link_site, remote_blob_id, blob_key, deleted_at, hlc_json, server_known, schema_version, groups_raw)` with an index on `message_id`. It is the first synchronized `attachment` table; there is no foreign key.
  - The local non-synchronized `attachment_local(attachment_id, state, file_name)`, `state` limited to `local | missing` until slice 11; `draft_attachment(draft_key, attachment_id, sort_index, kind, filename, mime, byte_size, width, height, sha256, file_name, created_at)`; `mirror_file(chat_id, journal_date, attachment_id, name)` (what the Markdown copy wrote into a day folder).
  - Before migrating an existing database a backup `pre-migration-v6-<UTC>.db` is taken (C.2). Existing drafts are kept.
- **Groups of an attachment** (§A.0): `core`, `order` (`sort_index`) and `deleted`, all three under one HLC of their own and `sv = 1`, written in the send transaction. Unknown groups and fields are kept verbatim like everywhere else.
- **The `core` group.** Every field of the §A.0 row is present; the optionals not used by a kind (`duration_ms`, `url`, `link_title`, `link_site`) are `null`. `sha256` and `blob_key` are lowercase hex of 64 characters (32 bytes). `remote_blob_id` is a UUID v7 and `blob_key` is 32 bytes from the OS CSPRNG, both made when the draft attachment is turned into an attachment, in the send transaction. `filename` is at most 255 bytes, has no control characters (C0, DEL, C1), no path separators and no bidi override characters (U+202A..U+202E, U+2066..U+2069), and is never empty (`file` replaces an empty name). `mime` is a string (`application/octet-stream` if unknown). `core` is immutable except for the §7 re-upload. Bytes (JCS, keys by UTF-16 code units; illustrative values, `sha256` is not derived from a file):
  `{"blob_key":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f","byte_size":245760,"duration_ms":null,"filename":"IMG_0001.jpg","height":960,"kind":"image","link_site":null,"link_title":null,"message_id":"5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d","mime":"image/jpeg","remote_blob_id":"01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","url":null,"width":1280}`.
  `order` is `{"sort_index":0}`, `deleted` is `{"deleted_at":null}`. `kind` is `image | file | audio | video` in slice 7 (`link` comes with the P1 previews). For an image `width` and `height` are taken after the EXIF orientation is applied; for the other kinds they are `null`.
- **Slice 7a note (04.11.2026, voice notes, ADR-0015): no change to the bytes or the shape of `core`.** `audio` and `video` may carry `duration_ms` (the Telegram import since slice 7, the recorder since 7a); `image`, `file` and later `link` keep it `null`. The value is `null` or an integer `0 ..= 4 294 967 295 000` (the import's full range, a `u32` of seconds in milliseconds); a writer that cannot satisfy this leaves `null` instead of failing. The recorder's own limits (1 s to 30 min) are a product rule of `send_voice`, not a protocol rule. Reference bytes of an `audio` row with a length (illustrative values): `{"blob_key":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f","byte_size":321536,"duration_ms":40000,"filename":"voice-2026-11-04-213012.m4a","height":null,"kind":"audio","link_site":null,"link_title":null,"message_id":"5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d","mime":"audio/mp4","remote_blob_id":"01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","url":null,"width":null}`.
- **An empty `body`.** `body_md = ""` is valid exactly when the entry has at least one live attachment (a text-less photo, or an imported entry whose placeholder was removed). The bytes of its `body` group are `{"base":null,"body_md":"","edited_at":null,"supersedes":[]}`. `edit_message` follows the same rule; the capture append never prepends `\n\n` to an empty body.
- **Trash and purge.** Trashing an entry does not touch its attachments' `deleted` group: they are hidden with the entry. Purging the entry writes in the same transaction `tombstone('attachment', id, purged_of, purged_at, cascade_of = <message id>, NULL)` with a snapshot of the HLCs of all the attachment's groups, deletes the `attachment`, `attachment_local` and `draft_attachment` rows and marks the day dirty. The file and the thumbnail are deleted after the commit; a failed deletion is finished by the next sweep because the tombstone exists. `purged(attachment)` needs `remote_blob_id` (§5.2), which a tombstone does not carry: harmless while there is no account, and slice 11 adds the column.
- **An exception to §A.0 (like ADR-0006): removing the Telegram placeholder writes no revision.** When media of an imported entry are attached later, the "[… not imported]" line is removed from `body_md` only if the entry is unedited since import: `hlc(body) == hlc(core)` (the import writes all groups under one HLC, §C.3, and every rewrite bumps `body`) and `body_md` equals the import's rendering in ru or en. The rewrite is the usual one (new HLC strictly greater, `base`/`supersedes` per §3.4, `edited_at = now`, tags, search, copy mark) without the `edit` revision. Two devices that import the same media produce equal texts and duplicate attachment rows (the limitation of §C.3).
- **Attachment envelope size.** One attachment envelope is far below 1 MiB (§5.3); no new local-only rule is needed.

### C.7. Chat colour refinement (04.11.2026, KINIG-59)

No schema change: no migration, no new column or index. `chat.color` and `meta.color` already existed; this fixes what the field holds.

- **The `meta.color` values.** One of seven lowercase names — `blue`, `teal`, `green`, `orange`, `pink`, `violet`, `grey` — or `null`. `null` means automatic: the colour is computed on every read from the chat id (32-bit FNV-1a over the UTF-8 bytes of the id, modulo 6, into the order `blue, teal, green, orange, pink, violet`; `grey` is never automatic) and is never written. A client may therefore leave `color` as `null` forever, and a new or imported chat is created with `"color":null`, exactly the creation bytes of slice 3. Only an explicit pick writes a name.
- **Unknown values.** A string that is none of the seven names (another client's value) shows as automatic and is kept verbatim: renaming, an emoji change or pinning rewrites `meta` with the unknown `color` untouched. Inbox shows grey whatever `meta.color` holds, and this client does not write it (`set_chat_color` on Inbox is refused). A `color` that is not a string and not `null` (a number, an object) is a corrupt `meta` like any other mistyped field (§A.0), not an unknown value: this client has nothing to keep verbatim, so later local edits of that chat (rename, emoji, colour, pin, archive) fail with `internal` until another client rewrites the group; the chat itself still displays.
- **Picking** rewrites the `meta` group with a new HLC strictly greater than the old one, like a rename or an emoji change (`rewrite_group`); `title`, `icon` and unknown fields are kept byte for byte. Picking the value already stored writes nothing (no new HLC). Picking the colour that is currently *shown* for a `null` chat still writes, because the stored value differs.
- **Concurrency.** The group layout does not change, so a rename on one device and a colour pick on another resolve by `meta` last-writer-wins, exactly like title against icon today; the loser's field is lost with the group, as it already is for title and icon.
- **Not carried by the Markdown copy** (the chat folder name does not depend on the colour).

## D. libsodium primitives and Argon2id profiles

| Profile                  | `opslimit` | `memlimit`          | libsodium preset                                                             |
|--------------------------|------------|---------------------|------------------------------------------------------------------------------|
| 1 (lower bound, default) | 3          | 268435456 (256 MiB) | `MODERATE` — about 0.7 s on a 2.8 GHz Core i7 according to the documentation |
| 2                        | 4          | 1073741824 (1 GiB)  | `SENSITIVE`                                                                  |

An account's profile only grows (§8.2). **Checked (S4b, 2026-11-06):** the two rows are asserted against `libsodium-rs` 0.3.0 in `profile.rs` and were read from libsodium.js 0.8.4 (libsodium 1.0.22): `crypto_pwhash_OPSLIMIT_MODERATE` 3, `MEMLIMIT_MODERATE` 268435456, `OPSLIMIT_SENSITIVE` 4, `MEMLIMIT_SENSITIVE` 1073741824, `ALG_ARGON2ID13` 2. Argon2id output of both profiles is identical in the two (the vectors).

| Purpose         | Function                                             | Constants (headers of the `stable` branch)                              |
|-----------------|------------------------------------------------------|-------------------------------------------------------------------------|
| Envelope, wraps | `crypto_aead_xchacha20poly1305_ietf_*`               | key 32, nonce 24, tag 16                                                |
| Blob            | `crypto_secretstream_xchacha20poly1305_*`            | key 32, header 24, `ABYTES` 17, `TAG_FINAL = PUSH \| REKEY`             |
| Password        | `crypto_pwhash`, `ALG_ARGON2ID13`                    | salt 16; `MODERATE` 3 / 268435456; `SENSITIVE` 4 / 1073741824; one lane |
| Subkeys         | `crypto_kdf_derive_from_key` (BLAKE2b)               | master key 32, context 8, subkey 16–64 (we use 32)                      |
| Hashes, PRF     | `crypto_generichash` (BLAKE2b)                       | output 16–64 (32 by default), key 16–64                                 |
| Padding         | `sodium_pad` / `sodium_unpad` (ISO/IEC 7816-4)       | block 1024                                                              |
| Other           | `randombytes_buf`, `sodium_memcmp`, `sodium_memzero` | —                                                                       |

## E. Sources

- libsodium documentation: https://doc.libsodium.org/ — the sections `secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction`, `secret-key_cryptography/secretstream`, `password_hashing/default_phf`, `key_derivation`, `padding`.
- libsodium headers and sources (`stable`): `include/sodium/crypto_{aead_xchacha20poly1305,secretstream_xchacha20poly1305,pwhash_argon2id,kdf_blake2b,generichash_blake2b}.h`, `crypto_pwhash/argon2/pwhash_argon2id.c` (one lane), `configure.ac` (1.0.22): https://github.com/jedisct1/libsodium/tree/stable
- Crates:
  - https://docs.rs/libsodium-rs/latest/libsodium_rs/ — 0.3.0 of 2026-10-14, the crate was created on 2025-04-04 (crates.io);
  - https://github.com/jedisct1/libsodium-sys-stable — 1.24.3;
  - https://docs.rs/bip39/latest/bip39/struct.Mnemonic.html — 3.0.0, `parse_in_normalized`, `to_entropy`;
  - https://docs.rs/chacha20poly1305/ — 0.11;
  - `sodiumoxide` has been archived since 2022: https://github.com/sodiumoxide/sodiumoxide;
  - versions `argon2` 0.6.0, `blake2` 0.11.0, `aead-stream` 0.6.0, `uuid` 1.28.0 — crates.io API, 2026-10-20; `Uuid::now_v7()` in `uuid` 1.28.0 — feature `v7`, `src/v7.rs` (verified during the rev 4 review).
- RFC 9106, §4: https://www.rfc-editor.org/rfc/rfc9106.html
- RFC 8785 (JCS, Informational): keys sorted by UTF-16 code units, no whitespace, numbers per ECMAScript: https://www.rfc-editor.org/rfc/rfc8785.html
- `serde_json::value::RawValue` (feature `raw_value`, keeps the value's bytes without reformatting; serde_json 1.0.154): https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html
- A ready-made Rust crate for JCS was neither chosen nor checked: with our type restrictions, canonicalization is key sorting and escaping, with test vectors in S4b.
- BIP-39: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki and `bip-0039/bip-0039-wordlists.md`
- Kulkarni et al., "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases", 2014, fig. 5: https://cse.buffalo.edu/tech-reports/2014-04.pdf
