# KINIG-96: spike S4b, envelope and keys

Budget: ~9 h to merge (code ~4–5 h, guardian and review ~4 h); stop and report at 18 h. Huly: KINIG-96 (under KINIG-20, slice 11); branch `spike/S4b-keys`, PR and first commit `KINIG-96: …`. Release: none (nothing visible in the app). Source: PRD §10.3 and §14 (S4b criteria: forging an HLC or a flag, or moving an envelope, is detected; a password change without re-encryption; recovery from 24 words), ADR-0001 §5, §8–§11, appendix §A.1, §A.2, §A.5, §B (S4b items 1–14), §D.

S4a showed that ADR-0001's protocol needed nine fixes once it met hostile delivery. S4b does the same for the bytes and the keys. **Is the envelope and key scheme of ADR-0001, as written, buildable with the chosen binding on our three OSes? Does it keep the promises of §10.3?** Where it does not, fix the rule and edit ADR-0001 in place, marked "S4b, <date>", in the same PR.

## Scenario
1. `cargo test -p kinige-crypto` builds libsodium through the chosen binding, on macOS, Windows and Linux in CI.
   - It seals and opens envelopes and wraps against committed test vectors.
   - It checks that changing any open byte, or moving an envelope to another record or account, is refused before anything is parsed.
2. `cargo test` in `spikes/S4b-keys/` runs the account flows against an in-memory server: registration, login on a new device, password change, recovery with the 24 words, reissuing the words, and crashes and lost answers between their steps. A device that logged in with the new password reads every envelope written before the change. Nothing was re-encrypted.
3. The S4a simulation runs its properties with real sealed envelopes in place of its hash seal, at a small fixed count. The protocol and the bytes are proven together.

## Included
- **Gate first (appendix §B item 11):** the binding of ADR-0001 §11 (`libsodium-rs`; fallback `libsodium-sys-stable` with a thin wrapper, decision 1) builds and passes a smoke test in the `rust`, `macos` and `windows` CI jobs. Record the libsodium version it links.
  - If neither builds on one OS, stop and report with the build logs. **Don't switch to another crypto library on your own:** that changes ADR-0001 §11 and needs the coordinator.
- **`crates/kinige-crypto` (workspace member, production code; not built for wasm32: `kinige-domain` stays crypto-free).** Only ADR-0001 primitives, never our own cryptography. Every function cites its ADR section.
  - **Envelope `format_version = 1`** (§5, §A.1): the layout, `AD_env` (the domain label, `account_id` and all open bytes before the nonce as received), a random 24-byte nonce, `sodium_pad` to 1024 bytes, `key_id`.
    - Opening checks the order of §A.4: lengths, and limits before any allocation; then AEAD; then parsing.
    - The payload's group set must equal `header.groups`. Duplicate JSON keys are refused.
  - **Key hierarchy** (§8.1): Argon2id profiles 1 and 2 (§8.2, §D) → `pw_root` → `auth_key` and `KEK_pw`; the 24 words → `rc_root` → `rc_auth_key` and `KEK_rc`; `DEK[key_id]` → `K_env` and `K_rev`.
    - `wrap_pw` and `wrap_rc` with `AD_wrap`.
    - BIP-39: `parse_in_normalized`, then `to_entropy`, never `to_seed`, after the input normalization of §8.1.
  - **Profile rule** (§8.2): unknown → refuse; lower than `kdf_profile_seen` → refuse **before deriving**. On a change or a recovery the profile is `max(seen, sent, default)`.
  - **Blob** (§7, §A.2): `secretstream` in 1 MiB chunks. A truncated stream, a missing `TAG_FINAL`, or bytes after it → error.
  - **The `conflict` revision id with `K_rev`:** replace S4a's stand-in in `kinige-domain::sync::conflict`. The input bytes stay in the domain and the keyed hash goes here; the domain crate keeps no crypto dependency.
  - **Secrets:** zeroed after use (`memzero`, or a zeroing wrapper over it). No `Debug` or `Display` of any key, password, words or token. A test proves that formatting them redacts. Never panic; garbage-input tests for every opener and parser (rule 12).
- **`spikes/S4b-keys/`** (a separate cargo project like `spikes/S4a-sync`, path dependency on `kinige-crypto`): an in-memory account server and a client.
  - The server stores `generichash(auth_key)` and `generichash(rc_auth_key)` and compares them in constant time.
  - It derives the salt for an unknown login from `server_secret`.
  - Registration and recovery are idempotent by `account_id` and by the new `rc_auth_key` (§A.5).
  - A recovery revokes the other sessions.
  - It models the "Save" gate of the words: no registration or replacement request goes out before the save is confirmed.
  - It covers appendix §B S4b items 5–8, 13 and 14 as tests, including the crash and lost-answer cases.
  - Item 10 (logs) is covered as follows: the spike captures everything the client and the server log and asserts that no secret's bytes appear in it, in any encoding the code could print (hex, base64, decimal arrays).
- **Test vectors (§B items 1 and 12):**
  - Fixed key, nonce and input for: the envelope, `AD_env`, both wraps, the kdf subkeys, padding, the revision id and its `snapshot_json`, and blobs of 0 bytes, 1 MiB − 1, 1 MiB and 1 MiB + 1.
  - Make the vectors once with libsodium.js (`libsodium-wrappers`, a dev-only npm package in the spike, licence checked), so byte compatibility with a future web client is proven rather than assumed. Commit them as JSON. The Rust tests read them.
  - Record in the README how to regenerate them.
- **S4a with real envelopes:** a cargo feature or a second seal implementation in `spikes/S4a-sync` that seals with `kinige-crypto`. CI runs the properties with it at the small fixed count. Its default stays the fast hash seal.
- **Measurement:** profile 1 (and profile 2 once) wall time and peak memory on the M3 Pro and on the three CI runners. Note it in the README, with the commands.
- **Docs:** `spikes/S4b-keys/README.md`; ADR-0018 "S4b results", half a page (what was proven, what was found, what is left for S4c and slice 11); ADR-0001 edits in place marked "S4b"; a PRD changelog line; `deny.toml` updated if the binding needs it (the licences must permit redistribution); and `ATTRIBUTION.md` if libsodium is statically linked into a shipped binary.

## Not included
- The OS secret store (Keychain, Credential Manager, Secret Service): slice 11.
- HTTP, axum, Postgres, TLS, rate limits: S4c.
- Any UI: the words screen, "Save" as a PDF, onboarding (Q12).
- DEK rotation (P2).
- Changes to `kinige-store`, the schema, or the app.
- Linking `kinige-crypto` into `src-tauri`: it is not linked into the app in this spike. CI builds and tests it in the workspace, so it stays compiling on all three OSes, but the shipped app does not depend on it yet.

## What "done" looks like
- CI is green on all jobs:
  - `kinige-crypto` is built and tested in the `rust`, `macos` and `windows` jobs;
  - the spike has its own step or job;
  - S4a runs with real envelopes.
- Every appendix §B S4b item 1–14 is listed in the README with the test that covers it, or with the reason it is not covered.
- The vectors made by libsodium.js pass in Rust.
- The negative tests pass. Garbage tests cover every opener: the envelope, the wrap, the blob, the words and the header JSON.
- README, ADR-0018, ADR-0001 edits and the PRD line are in the PR, and the measurement is in the README.

## Touches sensitive areas
Yes: encryption and keys (CLAUDE.md "Sensitive areas"; home-made cryptography is forbidden). `sync-crypto-guardian` reviews before merge, checking the bytes against §A.1 and the key flows against §8 line by line. `slice-reviewer` checks the spec, the evidence and the CI. New dependencies: the libsodium binding, `bip39`, `libsodium-wrappers` (dev, spike only), each checked by `cargo-deny` or `npm` licence review.
