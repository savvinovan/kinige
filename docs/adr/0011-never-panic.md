# ADR-0011: "Never crash" — lints instead of discipline, a panic loses no data

Date: 2026-11-01. Status: **accepted** (tech debt task KINIG-28, not a slice). PRD §12 (quality), ADR-0009 (a panic in a writer job and in an MCP call is caught by `catch_unwind`).

## Context
The app is a personal journal: a crash in the middle of a write or at startup is worse than any error in the window. Before this task the rule rested on discipline (there was almost no `unwrap` in the app code), but nothing checked it, and panics did not get into the log.

## Decision

**Lints (`Cargo.toml`, `[workspace.lints]`, `deny`).** `clippy::unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`, `indexing_slicing`, `string_slice`; `unsafe_code`. The behavior in tests is set by `clippy.toml` (`allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests`, `allow-print-in-tests`; for `unreachable`, `todo`, `unimplemented` and `string_slice` clippy 1.94 has no such setting).
- The root of every integration test (`tests/*.rs`) starts with `#![cfg(test)]`: otherwise clippy does not consider helpers outside `#[test]` to be test code. Examples (`examples/`) and `build.rs` allow what they need with a `#![allow]` line with a reason.
- `unsafe_code = "deny"`, not `forbid`: the only `unsafe` is `geteuid` in `effective_uid` (`kinige-store/src/settings_file/io.rs`); `std` has no safe equivalent, and a dependency for one call is worse than one justified line. The test `ipc_restore_e2e` (macOS) calls `set_var`, which is `unsafe` in 2024: `#[allow(unsafe_code)]` on the test.
- The exception for app code is a provably infallible literal: `#[allow(clippy::expect_used)]` and `.expect("why it cannot fail")`. There are no exceptions right now.
- The hooks for the safety-net tests (`panic_in_read_for_test`, `panic_in_agent_read_for_test`, the `panic_next` flags of the copy, maintenance and the MCP listener) panic on purpose: the first two are marked `#[allow(clippy::panic)]`, the flags fire only with `debug_assertions`, and they are absent in release.
- `arithmetic_side_effects` is not enabled: 199 hits on counters and iterators that do not depend on input (verified by running it). Lengths and offsets from input were reviewed by hand; allocations are always proportional to data already received (an entry ≤ 512 KiB, an MCP frame ≤ 4 MiB, the settings file ≤ 1 MiB, an import ≤ 1 GiB).

**Safety net.**
- `panic = "unwind"` in all profiles; the `config.rs` test fails if `panic = "abort"` appears in the manifest.
- The panic hook (`src-tauri/src/panic_hook.rs`) writes to `kinige.log` only the thread and the location (file:line); the panic text is not written (it may contain data), and a path inside dependencies is cut to `<crate-version>/src/…`.
- The Tauri command handler is wrapped in `catch_unwind` (`guarded`): a panic in the synchronous part (argument parsing, a synchronous command on the main thread) does not take the process down, and the window gets an `internal` error (a clone of `InvokeResolver` is kept before the call; tauri panics on a second response, so the rejection is under `catch_unwind` too). The module is `startup.rs`. A limitation verified against the tauri 2.12 sources: a panic in the asynchronous part of a command is caught only by the tokio task — the process is alive, but the call gets no response (the window's promise does not settle); reads, writes and agent calls in `kinige-store` catch panics themselves and answer with an error.
- A startup error (`setup`, building the app) is a log write (category only) and exit with code 1, not a panic (tauri itself panics on a `setup` error).
- Threads are started via `thread::Builder::spawn` (`std::thread::spawn` is closed off by `disallowed-methods` in `clippy.toml`; in tests — `#[allow(clippy::disallowed_methods)]`), and an OS refusal is an error (`StoreError::Internal` / a log write). Poisoned mutexes are recovered (`unwrap_or_else(PoisonError::into_inner)`): previously some places silently skipped work, and the copy's waiting thread spun without pauses when poisoned.

**Canonical JSON (`jcs.rs`).** Groups are written straight into a `String` (UTF-8 is guaranteed by the type, without `expect`); the bytes did not change. The write depth is limited by `MAX_DEPTH` = 128 — this is the `serde_json` parsing limit (`remaining_depth` in `de.rs`): no more than 127 nested arrays and objects are written, the 128th is `JcsError::TooDeep`; what we write, we can read. A test checks the limit against `serde_json` itself. A line about the depth was added to ADR-0001 (details, A.4 item 5): deeper means `malformed`.

**Panic hunters.** `src-tauri/tests/panic_hunter_ipc.rs` — every command of every window (including without an open store) gets empty, wrongly typed and hostile-valued arguments; `panic_hunter_mcp.rs` — the same for all MCP tools over a real socket; `kinige-domain/tests/hostile_input.rs` — multi-byte characters at every position of the samples for all domain parsers.

## Consequences
- A new `unwrap` in app code will not pass CI. If one is needed — `Result`, `?`, `get()` or a justified exception as above.
- Real problems found along the way (in the log of task KINIG-28): quadratic parsing time for tags and snippets on a line like `[a](`×N or `<a`×N (up to 20 s on a 200 KB line) — a bracket nesting limit of 64 and a single search for the closing one were introduced; `std::env::args()` panicked on a launch argument that is not UTF-8; `app.state::<Store>()` in the clipboard hotkey would have panicked without an open journal.
