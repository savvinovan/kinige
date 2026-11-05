# ADR-0018: S4b results, the envelope and the keys with the real binding

Date: 2026-11-06. Status: **accepted** (KINIG-96; spec `docs/slices/S4b-envelope-and-keys.md`, code `crates/kinige-crypto` and `spikes/S4b-keys`, the map of tests and the numbers in `spikes/S4b-keys/README.md`). Changes ADR-0001 (revision 9, each change marked "S4b, 2026-11-06") and the PRD's version history and S4b row; no schema, migration or app change: `kinige-crypto` is a workspace member that the app does not link yet.

## What was proven
The envelope and key scheme of ADR-0001 is buildable with the chosen binding and keeps the promises of PRD §10.3.
- `libsodium-rs` 0.3.0 (over `libsodium-sys-stable` 1.24.3, libsodium **1.0.22**) builds and passes in CI on Linux, macOS 15 and Windows 2025. No fallback and no other library were needed.
- libsodium.js (`libsodium-wrappers-sumo` 0.8.4, libsodium 1.0.22) reproduces every byte of the committed vectors: Argon2id of both profiles, all subkeys, both wraps with their AD, padding, both envelopes with `AD_env`, the `conflict` revision id and its `snapshot_json`, canonical JSON with hostile strings, and blobs of 0, 1 MiB - 1, 1 MiB and 1 MiB + 1 bytes. What Rust seals opens in libsodium.js, with its own layout code. The web client is therefore unblocked on the format.
- Changing any bit of an envelope, replacing any header field by a valid other value, moving it to another record or account, or opening it with another key is refused by the AEAD before anything is parsed. A giant length is refused before any allocation. Garbage into every opener (envelope, wrap, blob, words, header JSON, payload JSON) is an error and never a panic.
- The account flows hold against an in-memory server: a password change without re-encryption (the stored envelopes are the same bytes), recovery from the 24 words, new words, a profile that only grows and a refusal before deriving, an unknown login that looks like a known one, and every crash or lost answer between the steps of registration, recovery and reissue. Nothing goes out before "Save". A full run logs no key, token, password, words or text in any of the encodings the code could print.
- S4a with real sealed envelopes keeps its nine properties and its negative controls (`spikes/S4a-sync`, feature `real-envelope`, in CI at a small fixed count). The protocol and the bytes are proven together.

## What was found
Seven corrections to the text of ADR-0001, none to the bytes of the format (details and tests in the README):
1. The format byte must be checked before the length rules, or a future format never reaches `pending_unknown` (§5.4, §A.4).
2. An authentic header that disagrees with the transport is a forgery and stops the page, like an AEAD failure; the AD alone cannot catch a genuine envelope served under another record (§5.4).
3. The outbox keeps envelope bytes and resends them: a rebuilt envelope has a new random nonce and the server answers `op_conflict` (§6.1).
4. The revision id is the first 16 bytes of the 32-byte hash, not a 16-byte hash (§8.1).
5. `max(seen, sent, default)` on a password change is moot; it decides on recovery (§8.2).
6. The binding's own key types print their bytes with `Debug`; `kinige-crypto` keeps its own redacted types and never stores the binding's.
7. libsodium.js needs the sumo build for Argon2id.

Build facts: the C library takes the profile's flags (`-O0` in dev makes Argon2id of profile 1 take about 6 s, so the workspace sets `opt-level = 3` for it); Windows downloads a prebuilt MSVC library over HTTP at build time (minisign-checked) and uses `zip` 8.6 with four `extract` advisories (ignored in `deny.toml` with the reason: build time only, a signed archive); `bip39` is CC0-1.0 (allowed in `deny.toml`).

Argon2id (decision 8, "if more than 2 s, discuss"), median of five runs for profile 1 and one run for profile 2, release build: profile 1 takes 0.28 to 0.33 s on the M3 Pro and the Linux and Windows runners and 0.69 s on the macOS runner; profile 2 takes 1.8 to 2.0 s on the same three and **5.6 s on the macOS runner**. Peak memory is the `memlimit` plus about 3 MiB (258 and 1027 MiB). Profile 1, the default, is far inside the limit. Profile 2 is over it on the weakest runner; it is what a newer client would raise accounts to and what an unusually cautious user could choose, so the question for the user is whether 5 s for a login on a slow machine is acceptable there. In WASM under node (M3 Pro) profile 1 took 0.49 s and profile 2 took 2.3 s; a browser was not measured.

## What is left
- **S4c:** the server reads the header with `envelope::peek` and the lengths with `Layout::parse` before it reads the body; it declares `entity`, `id` and `op` next to each envelope (finding 2 needs them); credential versioning and compare-and-swap on a password change; rate limits; the HTTP shapes of the requests the spike calls as functions.
- **Slice 11:** link `kinige-crypto` into the app; the OS secret store for the DEK, the token and the prepared package; the "Save" screen (Q12); the outbox keeps bytes; streaming blobs and resumable upload (the blob API holds the file in memory today); a licence line for the BIP-39 word list in `ATTRIBUTION.md` (the data of the `bip39` crate; the licence of the list itself is not verified); `libsodium` in `ATTRIBUTION.md` once it is in a shipped binary; the Windows build needs network (or `SODIUM_DIST_DIR`).
- **Before the web client:** a browser measurement of Argon2id (node only so far) and the sumo build; a check that a tab can allocate 1 GiB for profile 2.
- **Not modelled:** rate limits and TLS, DEK rotation (P2, the revision id changes with it), a constant-time review beyond `sodium_memcmp`, an audit of the binding.
