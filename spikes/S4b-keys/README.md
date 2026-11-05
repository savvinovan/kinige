# Spike S4b: the envelope and the keys

Date: 2026-11-06. Task: KINIG-96, spec `docs/slices/S4b-envelope-and-keys.md`. Result: ADR-0018, ADR-0001 revision 9, PRD v0.23.

The question: is the envelope and key scheme of ADR-0001, as written, buildable with the chosen binding on our three OSes, and does it keep the promises of PRD §10.3? Answer: yes, with seven corrections to the text (below) and no change to the bytes of the format. `libsodium-rs` 0.3.0 builds and passes in CI on Linux, macOS and Windows and links libsodium 1.0.22; libsodium.js (`libsodium-wrappers-sumo` 0.8.4, also libsodium 1.0.22) reproduces every committed vector, and what Rust seals opens in it.

## What it is
- **`crates/kinige-crypto`** (a workspace member, production code, native only; the app does not link it yet): the envelope (`envelope`, `header`, `operation`), the key hierarchy (`keys`, `profile`, `words`, `wrap`), the attachment blob (`blob`), the revision id and the server's hashes (`ids`), typed zeroed secrets (`secret`). Only the primitives of ADR-0001 §11, through `libsodium-rs`; no cryptography of our own.
- **`spikes/S4b-keys`** (this crate, outside the workspace, like S4a and S5): an in-memory account server (`server.rs`), a client with the account flows of §8 (`client/`), a link that loses requests and answers (`link.rs`), and a scanner for secrets in logs (`leak.rs`). What they exchange is real bytes from `kinige-crypto`.
- **`spikes/S4b-keys/vectors`**: the generator of the test vectors (`gen.mjs`, libsodium.js) and the Rust-to-JS check (`verify.mjs`). The vectors themselves live next to the crate that tests them: `crates/kinige-crypto/vectors/vectors.json` and `blobs/*.bin`.
- **`spikes/S4a-sync`**, feature `real-envelope`: the S4a simulation with every envelope sealed and opened by `kinige-crypto` and the revision ids from the real keyed hash. The default stays the hash seal.

## Run
```text
cargo test -p kinige-crypto                         # 62 unit tests and the smoke test, about 3 s
cd spikes/S4b-keys/vectors && npm ci && npm run check   # the committed vectors are what libsodium.js makes now, and the blobs decrypt in it
cd spikes/S4b-keys && cargo test                    # flows, crashes and lost answers, logs: about 25 s
cd spikes/S4b-keys && S4B_NODE=1 cargo test --test js_interop   # what Rust seals opens in libsodium.js (needs the `npm ci` above)
cd spikes/S4a-sync && S4A_CASES=500 S4A_CASES3=100 S4A_CONTROL_CASES=3000 S4A_SEED=1 \
  cargo test --features real-envelope --test properties --test negative --test scenarios --test real_envelope
cargo run --release -p kinige-crypto --example measure_kdf -- 1 5   # Argon2id profile 1, 5 runs; "-- 2 1" for profile 2
```
**Regenerating the vectors:** `cd spikes/S4b-keys/vectors && npm ci && npm run vectors`. It rewrites `vectors.json` and the four blobs. The JSON is deterministic (all inputs are fixed and fake); the blobs are not (a secretstream header is random), so regenerate only when a format changes, and add the new fingerprints of `vectors.json` to `.gitleaksignore` (the generic-api-key rule reads the fake `auth_key` as a key).

## Appendix B, S4b items 1-14: what covers each
Paths are relative to `crates/kinige-crypto/src/` for the crate's unit tests (`vectors_tests.rs`, `blob_vector_tests.rs`, `forgery_tests.rs`, `envelope_tests.rs`, `key_tests.rs`, `garbage_tests.rs`), to `spikes/S4b-keys/tests/` for `flows.rs`, `crash.rs`, `logs.rs`, `js_interop.rs`.

- **1. Test vectors.** `vectors_tests.rs`: Argon2id of both profiles and every subkey, the words, both wraps with `AD_wrap`, padding, both envelopes with `AD_env`, the revision id and its `snapshot_json`. `blob_vector_tests.rs`: blobs of 0, 1 MiB - 1, 1 MiB and 1 MiB + 1 bytes. Rust to JS: `js_interop.rs` with `vectors/verify.mjs`.
- **2. Substitution of an open field, moving an envelope.** `forgery_tests.rs`: every bit of every byte; each header field replaced by a valid other value; `format_version`; `key_id`; another account; a purged turned into an upsert. `S4a-sync/tests/real_envelope.rs`: a header changed after sealing is `Tampered`. The S4a properties run with real envelopes.
- **3. A giant `header_len` or length is refused before allocating.** `forgery_tests::a_giant_length_is_refused_before_anything_is_allocated` (`Layout::parse` takes the 9 prefix bytes and the declared total, nothing else), `envelope_tests::the_limits_of_the_format_are_exact`, `garbage_tests::layout_parse_survives_every_prefix_and_total`.
- **4. A truncated blob, no `TAG_FINAL`, bytes after it.** `key_tests::a_truncated_blob_or_one_without_final_or_with_trailing_bytes_is_an_error`, `bytes_after_the_final_chunk_are_an_error_whatever_the_size_of_that_chunk`, `blob_vector_tests::a_damaged_vector_blob_is_refused_everywhere_it_can_be_damaged`, `garbage_tests::random_bytes_never_open_as_a_blob`.
- **5. A password change without re-encryption; recovery from the 24 words.** `flows.rs`: `a_password_change_does_not_reencrypt_and_old_entries_stay_readable` (the stored envelopes are the same bytes and a new device reads all of them), `recovery_with_the_24_words_gives_back_the_data_and_ends_other_sessions`.
- **6. After recovery the old words are invalid; reissue works.** `flows.rs`: `after_recovery_the_old_words_are_invalid_and_the_new_ones_work`, `reissuing_the_words_replaces_them`.
- **7. A lower or unknown profile is refused before deriving.** `flows.rs`: `a_server_that_sends_a_lower_profile_than_seen_is_refused_before_deriving` and `an_unknown_profile_number_is_refused_before_deriving` (both check that no Argon2id ran), `a_password_change_on_a_device_that_saw_a_higher_profile_than_the_server_sends_is_refused_before_deriving`; the unit tests of `profile.rs`.
- **8. An unknown login: a 16-byte salt, the same profile; recovery answers.** `flows.rs::an_unknown_login_looks_like_a_known_one` (the shape, the repeat, the same `AuthFailed` and the same server log lines for "no such login" and "wrong key"); `ids::tests::the_unknown_login_salt_depends_on_the_secret_and_the_login`.
- **9. Words: `to_entropy`, a typo, a swapped word, upper case.** `key_tests.rs`: `the_official_bip39_phrases_give_their_entropy` (the official vectors), `a_typo_is_unknown_word_and_a_swapped_dictionary_word_is_a_checksum_error_most_of_the_time`, `input_is_normalized_before_parsing`, `only_24_words_are_accepted`; `garbage_tests::nonsense_words_are_refused_and_never_panic`.
- **10. Logs hold no key, token or text.** `logs.rs`: a full run (registration, login, password change, recovery, reissue, an envelope) with more than 20 secrets scanned for in hex, base64 (four variants), decimal lists and text; the scanner is first shown to find a deliberate leak in every encoding. Redaction of `Debug`: `secret.rs`, `key_tests::words_print_as_redacted`, `envelope_tests::the_opened_value_prints_no_payload` and `an_operation_prints_no_group_text`.
- **11. The binding builds on macOS, Windows and Linux.** `crates/kinige-crypto/tests/smoke.rs` in the `rust`, `macos` and `windows` CI jobs (it asserts libsodium 1.0.22); the whole crate is built and tested in the same three jobs.
- **12. Canonical JSON equals libsodium.js's.** `vectors_tests::canonical_json_is_the_same_bytes_as_libsodium_js_makes`: non-ASCII, a surrogate pair, quotes, controls, DEL, U+2028, key order by UTF-16 code units (an astral key before U+FF5E).
- **13. Crashes and lost answers between the steps.** `crash.rs`: nothing goes out before "Save" (registration, recovery, reissue); a cancelled save; a crash with the new words on screen; a lost answer and a lost request at registration and at the replacement; a restart that settles an uncertain recovery by logging in; a taken login; an `account_id` clash; reissue.
- **14. Recovery on a device that has seen profile 2 when 1 is sent.** `flows.rs`: `recovery_on_a_device_that_saw_profile_2_when_1_is_sent_gives_profile_2_and_logs_in`, `a_password_change_never_lowers_the_profile_and_another_device_logs_in_after_it`.

Garbage into every opener (CLAUDE.md rule 12): the envelope, the wrap, the blob, the words, the header JSON, the payload JSON and the duplicate-key walker, in `garbage_tests.rs` and `strict_json.rs`; every truncation and extension of a valid envelope and of a blob is refused. S4a items 28 and 29 (a password change then login on another device; a lower profile refused, recovery works) are the same flows as 7 and 14.

## Findings
Each was met while writing the code or the tests and is now in ADR-0001 revision 9 (marked "S4b, 2026-11-06"). None changes the bytes of the format.

1. **A.4 checked the lengths before the format.** An envelope of a future `format_version` may be laid out differently; applying the rules of version 1 first turns it into an error instead of `pending_unknown` (a one-byte envelope of format 2 must be `UnknownFormat`, not "too short"). Now: a cap on the whole envelope, then the format, then the rules of version 1, then `key_id` (§5.4, §A.4).
2. **An authentic header that disagrees with the transport had no class.** The AD covers the header the sender wrote, so a server that serves a genuine envelope under another `id` passes the AEAD; only the match against the transport (A.4 step 4) catches it, and the ADR did not say what to do on a mismatch. Now: the same as an AEAD failure, the page is not applied (§5.4). The real-envelope simulation opens every envelope this way.
3. **The outbox must keep bytes.** With a random nonce the same record sealed twice under one `op_id` is two byte strings, and the server answers `op_conflict`. `spikes/S4a-sync/tests/real_envelope.rs` pins it; §6.1 now says it.
4. **The revision id is the first 16 bytes of a 32-byte hash.** BLAKE2b's output length is a parameter, so "16 bytes of `crypto_generichash`" had two readings with different ids. The vector pins the 32-byte form.
5. **`max(seen, sent, default)` on a password change is moot.** The old password is derived from the sent profile first, and that refuses a profile below `seen`; `seen` matters on recovery. The first version of the test expected a change to succeed on a device that had seen 2 while the server sent 1. §8.2 says where `max` matters; `Profile::for_new_password` takes the client's default as a parameter.
6. **The binding's `Debug` prints key bytes.** `libsodium-rs` derives `Debug` on its `Key` types. `kinige-crypto` builds them for one call from its own redacted types and never stores or returns them.
7. **libsodium.js standard has no Argon2id.** `crypto_pwhash` exists only in `libsodium-wrappers-sumo`; the web client needs the sumo build.

Also found, not about the scheme: the C library is built with the profile's flags (`-O0` in dev: one Argon2id takes about 6 s), so the workspace sets `opt-level = 3` for `libsodium-sys-stable`; on Windows the build script downloads a prebuilt library over HTTP (signature-checked) and uses `zip` 8.6, which has four RustSec advisories about `extract`, ignored in `deny.toml` with the reason; `bip39` and `bitcoin_hashes` are CC0-1.0, added to the allowed licences.

## Measurement
Argon2id (`crypto_pwhash`, the profiles of §D), `cargo run --release -p kinige-crypto --example measure_kdf -- <profile> <runs>`, one profile per process, a fixed fake password and salt. Peak memory is the process's peak resident set (`ru_maxrss`; `PeakWorkingSetSize` on Windows), the program included. The runner rows are from the `rust`, `macos` and `windows` jobs of CI.

| Machine                               | Profile 1 (3 passes, 256 MiB) | Profile 2 (4 passes, 1 GiB) | Peak memory (1 / 2) |
|---------------------------------------|-------------------------------|-----------------------------|---------------------|
| Apple M3 Pro, 18 GB, macOS 26 (local) | 332 ms (323-377, 5 runs)      | 1930 ms (1 run)             | 258 / 1027 MiB      |
| ubuntu-24.04 runner                   | 334 ms (324-337, 5 runs)      | 2028 ms (1 run)             | 258 / 1028 MiB      |
| windows-2025 runner                   | 283 ms (279-291, 5 runs)      | 1819 ms (1 run)             | 261 / 1030 MiB      |
| macos-15 runner (arm64)               | 694 ms (678-812, 5 runs)      | 5595 ms (1 run)             | 259 / 1027 MiB      |

Median of the runs, with min-max in brackets. An earlier CI run of the same code gave 688 ms and 5269 ms on the macOS runner and 298 ms and 1988 ms on the Windows runner, so the runners vary by a few percent run to run; profile 2 is one sample.

The limit of decision 8 is 2 s "on the weakest machine". No old or low-end machine was available; the GitHub runners are the weakest here. Profile 1, the default, takes 0.3 to 0.7 s everywhere. Profile 2 takes about 2 s on the M3 Pro and the Linux and Windows runners and 5.6 s on the macOS runner, which is over the 2 s of the decision; it is the profile a newer client would raise accounts to, so this is for the user to discuss (ADR-0018), not a blocker for S4b. A device with less free memory than the profile's `memlimit` fails the allocation (`CryptoError::Sodium`); profile 2 needs 1 GiB. In WASM (libsodium.js sumo 0.8.4 under node 26, on the M3 Pro) profile 1 took 486 ms and profile 2 took 2315 ms, one run each; a browser was not measured.

## Not verified
- A real browser: libsodium.js was run under node only. The memory limit of a tab (profile 2 needs 1 GiB) and the time there are not checked.
- The licence of the BIP-39 English word list as data (the `bip39` crate is CC0-1.0; the list in it is the BIP's); to be settled before the crate ships in the app (slice 11). `ATTRIBUTION.md` has a line for it.
- The audit of `libsodium-rs` 0.3.0 and the wrapper code beyond the functions used here.
- Timing side channels: `sodium_memcmp` is used for every comparison of a secret or a hash, but timing was not measured.
- Whether `-Os` (the release profile's `opt-level = "s"`) slows the C library: a run with it gave the same time as `-O3` on the M3 Pro (323 ms median of 5, profile 1), but it was not confirmed that the flag reached the C compiler. The override to `-O3` stays.
- The blob API holds the whole file in memory (the chunk loop is the one a streaming reader will use); streaming and the resumable upload are slice 7 / slice 11 work.
