# ADR-0022: Settings only in SQLite, read and partly changed by agents through MCP (supersedes ADR-0010)

Date: 2026-11-20. Status: **proposed** (KINIG-160, slice 5d; the spec `docs/slices/5d-settings-through-mcp.md` was reviewed by `sync-crypto-guardian` on 20.11.2026 and its rules are in the text; the guardian reviews the PR, the user accepts). PRD §2.1, §5.1, §9.3, §12.3; ADR-0001 appendix §C.4, ADR-0009, ADR-0010.

## Context
ADR-0010 made `settings.json` the input and `app_setting` the state, so a person or an agent could edit settings from outside. The file needed a polling thread, a safe reader, an atomic writer and an error page, and it was the one path by which an agent could switch its own access on. The user's decision of 20.11.2026: the file goes, everything lives in SQLite, and agents change settings through MCP, not by editing the database file.

## Decision
- **Settings live only in `app_setting`.** Nothing is read from a file. The user's old `settings.json` is never opened, written, renamed or deleted, and the window says nothing about it (it may hold chat titles; the release note says it can be deleted).
- **Two tools** through `Store::agent_call`, one log row per call:
  - `get_settings` (any mode) answers from its own struct, never from the window's `Settings`: the text size, the two shortcuts with their defaults, whether the copy is on (`default_folder`, `chosen_folder` or `off`, never a path), the agent's mode and visible chat ids (selected with ADR-0009's predicate, no titles), and `writable`.
  - `set_setting(key, value)` ("Read and write") changes `text_size` and nothing else. The key maps to a fixed Rust enum, so no input string becomes an `app_setting` key. The other six known names answer `setting_not_writable`; raw `app_setting` names, case or space variants and wrong values answer `invalid_argument`. The access check is repeated inside the writer transaction that writes the setting and its log row.
- **Why an agent cannot widen its own access** (this answers ADR-0010's "rejected: an MCP tool"). Access on or off, the mode, the visible chats and the copy folder are not in the allowlist; they change only through the settings window's commands, which MCP cannot reach. No switch inside the app (a variable, a flag, a hook, a driver step) turns access on for tests: `sqlite3` on a throwaway database is the only test path. Accepted, as PRD §4.1: a process of the user that writes the database file itself.
- **Why shortcuts are read-only** (the guardian's ruling). `Cmd+KeyV` passes `is_reserved` and would register; every Cmd+V in any app would then save the clipboard to Inbox, where an agent reads the passwords it holds. A reserved-key list cannot be complete for every app, and `shortcuts.capture` is a system-wide key grab that steals focus. The user may later allow it with Ctrl+Alt combinations only.
- **The log.** `args_json` of `set_setting` holds `key` (one of the seven known names, else `"other"`) and, on success only, the canonical `value`; a refused value never reaches the log. The outcome `not_writable` is new (`outcome` is plain `TEXT`: no migration). Old `settings_file` rows keep their label until rotation drops them.
- **The windows.** After a change both windows get `settings-changed`, as after a click; no tool shows, raises or focuses a window (rule 11).
- **The `Hotkeys` change lock stays.** The window is still a caller (two quick changes), though no file thread queues edits any more. It is a second mutex, separate from `inner`, held over check, register, save and release, so key presses and the overlay never wait for a change. The two commands stay `async`: the lock holder waits for the main thread inside `register`, and a sync command waiting for the lock would deadlock.
- **Restore can only narrow access:** off, no chats, "Read only", whatever the backup held. Without the file a backup's `read_write` would become effective for the first time, and its chat list may hold chats removed since. The copy folder is the backup's own.
- **The `settings_file_hash` row stays, inert.** No code touches it. The slice has no migration, so 0.9.2 can still open the database after a downgrade, and in 0.9.2 a missing or different hash means "apply the file", which would switch access back on from a stale file. With the row kept, 0.9.2 rewrites the file from the database. Accepted: a restore followed by a downgrade may still apply a stale file.
- **Clean-up.** At every start Kinige removes only regular files named exactly `.settings.json.` + 32 lowercase hex + `.tmp` (what the atomic writer left after a crash). A look-alike, a symlink and a directory stay; no link is followed; the log gets a count.

## Rejected
- **The file as an optional input:** it is the path that switched access on without the warning.
- **A generic `set_setting` over every key:** the key set grows with every slice; an allowlist of one cannot widen by accident.
- **Letting an agent change shortcuts behind a reserved-key list** (see above).
- **A migration that drops the hash row:** it would break the downgrade.

## Consequences
- ADR-0010 is superseded; ADR-0009 gains two tools and ADR-0001 appendix §C.4 the `args_json` rule and the outcome. `tools/list` has 6 tools in "Read only" and 10 in "Read and write"; a client sees the old list until it reconnects.
- Syncing settings between devices is still not settled (ADR-0001, "Not settled").
