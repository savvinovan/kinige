# ADR-0002: TS types from Rust via ts-rs

Date: 2026-10-19. Status: accepted (slice 0). Number 0001 is reserved for "protocol + keys".

## Context
PRD §3.2: Rust → TypeScript types are generated, not written by hand; choose `ts-rs` or `tauri-specta`.
The frontend does not know about Tauri (only `KinigeApi`), and `kinige-domain` must build for `wasm32` and must not depend on Tauri.

## Decision
`ts-rs` 12 in `kinige-domain`. Types with `#[derive(TS)]` and `#[ts(export)]` are written to `web/src/api/generated/`
on `cargo test` (the directory is set by `TS_RS_EXPORT_DIR` in `.cargo/config.toml`). The files are committed.
After `cargo test`, CI runs `git diff --exit-code -- web/src/api/generated`: stale types turn the build red.
The wrapper over `invoke` (`TauriApi`) is written by hand: there are few commands, and the argument and result types come from the generated ones.

## Why not `tauri-specta`
- The version for Tauri 2 at the time of the decision is `2.0.0-rc.25` (crates.io), that is, not stable.
- It ties the command descriptions to the `tauri` crate, while the protocol and domain types must live outside Tauri
  (`kinige-domain`, `kinige-sync` are used by the server and the WASM client).
- The gain of `tauri-specta` is typed command wrappers; with one or two dozen commands behind `KinigeApi`, this does not pay for the risk of an rc version.

## Consequences
If the number of commands grows and the hand-written wrappers start to drift from Rust, return to `tauri-specta` after its stable release.
