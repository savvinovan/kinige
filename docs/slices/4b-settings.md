# Slice 4b: the settings window and the `settings.json` file

**Budget:** ~7 h until ready for acceptance, ~11 h until merge; if not ready after 14 h, stop and report (§12.1a). Calibration: slices 3, 4, 4a: code in 1.5–2.5 h, 4–8 h until acceptance. **Depends on:** 4a (accepted 29.10.2026), ADR-0010 (written before the code, accepted by the user). Huly: **KINIG-39**; branch `slice/4b-settings`, the PR and the first commit are `KINIG-39: …`. **Sources:** PRD §2.1, §5.1, §9.3, §12.1–12.3, §15; ADR-0005, -0006, -0009; the user's decisions on KINIG-39 (a window as in Zed, a tree of sections on the left, a single scroll, subscreens, a file with “everything, including AI access”).

## Scenario
1. I press ⌘, (or profile → “Settings”, or “Settings…” in the icon menu). A separate “Settings” window opens; it can be moved and resized. Pressing ⌘, again raises the same window; a second one does not appear.
2. On the left, a tree: “Appearance”, “Keyboard shortcuts”, “Copies”, “AI agent access” (with “Visible chats” and “Access log” under it), “Settings file”. The selected item is highlighted. On the right, the section content with a single scroll.
3. In “AI agent access” I press “Visible chats ›”. On the right, a subscreen opens with the list of chats and the line “AI agent access › Visible chats” with a back arrow. The nested item is highlighted in the tree. The access log is also on a subscreen, long, with the same single scroll.
4. I turn access on. The warning about the provider appears over the window, and the content under it does not shift, even with “always show scroll bars”.
5. In “Settings file” I see the path and “Copy path”. An agent (or I, in an editor) changes `text_size` to `"large"` in the file and marks the chat “Work” in `agent_access.chats`. Within 2 s the windows apply the changes by themselves: the main window gets large text, and in the access section “Work” is marked. Next to it, a quiet line “Changed via settings.json · 14:02”, and the access log gets the line “settings.json · access changed”.
6. I change a shortcut in the window. The same key in the file changes immediately; everything else in the file, including foreign keys, stays.
7. I break the file (`{"text_size": `). Nothing crashes, all settings stay as they were. “Settings file” shows “The file could not be read: line 1, column 15”, and the tree item has an error mark. No toasts. An unknown key, a wrong type or a value that is not allowed gives an error line at that key; the other keys are applied.
8. `kill -9` after an edit in the window and after an edit of the file, launch: everything is in place both in the file and in the window.

## Included
- **The `settings` window** (Tauri). Created on first open (`index.html#settings`), so the cold start does not change. Closing destroys the window. Opening it from the background sets the Regular policy, like `show_main`.
  - ⌘, is the “Settings…” item of the app menu. The menu is built from the default menu: without the “Edit” menu, ⌘C and ⌘V stop working in WKWebView (from memory, check).
  - “Settings…” is added to the icon menu: when launched in the background there is no window, and ⌘, is unavailable.
  - While the journal is not open (restore, “database is newer”), the item is unavailable.
  - Closing the window while a shortcut is being recorded ends recording mode.
- **Moving out of the main window.** `SettingsDialog` is split into section pages, `AgentAccessDialog` and `useAgentAccess` move to the settings window. The main window no longer has the settings and access `Modal`.
  - Opening an entry from the log: a new command `settings_open_entry(id)`. It checks the entry (`message_not_found` → “Entry deleted”) and sends the main window the `open-message` event.
  - The settings window does not get `listMessages`.
- **Components** (through `design-system`, tokens only): the section tree (`nav`, `aria-current`, ↑/↓, Enter) with an error mark, a subscreen header with “back”, the row “Configure ›”, the section page.
  - **The tree collapses (feedback at acceptance).** A section with nested items collapses with a chevron on the left (the name opens the section and expands the group); by default groups are collapsed, except the group of the current page (it expands on any navigation); a collapsed group containing the current subitem is highlighted, and a subitem's error mark is visible on the section. Keyboard after the WAI-ARIA tree pattern (→, ←, ↑, ↓, Home, End, Enter, `aria-expanded`), no animation; the state is remembered for the lifetime of the window and in `localStorage` (not in `settings.json`). The selected item is a fill only, with no border; the focus ring in all windows (main, capture, settings) appears only after a key press (`data-input` is set by `useInputModality`, and one shared rule in `base.css` hides the ring; focus after a mouse action and when a window opens stays, but without the ring, including the profile button after the menu closes); in the tree it goes around the whole row: on the live Mac the border around the selected section was the focus that WKWebView puts on the first item when the window opens.
  - **Selection is a fill, with no border (feedback at acceptance):** the selected option of a toggle (“Normal” / “Large”, “Read only” / “Read and write”), the selected backup in the restore window and the found search line are marked with a single `highlight.background` fill (no weaker than 1.2:1 on the field, the paper and the hover fill; checked by the contrast test); hovering over an unselected option changes only the text color.
  - **Settings rows are a card group with thin dividers (feedback at acceptance):** keyboard shortcuts (the name on the left, the keys and “Change” on the right in one group; recording does not change the row height or the button positions, while the recording field may widen to the left; in a narrow window the name wraps, and the group moves below it only when it still does not fit) and the “Visible chats” / “Access log” rows.
  - **“Copy” as an icon (feedback at acceptance):** one `CopyButton` component (a copy icon, a check mark for ~1.5 s, the label “Copied”, no animation and no toasts) for the file path and the connection blocks. The warning is one `Modal` over the window: the page under it does not change its own `overflow`.
- **The file** `<data folder>/settings.json` (`$KINIGE_DATA_DIR/settings.json` in tests). Strict JSON, UTF-8, permissions 0600, up to 1 MiB. Keys:

  | Key                    | Values                                                     | Default                       | Section            |
  |------------------------|------------------------------------------------------------|-------------------------------|--------------------|
  | `text_size`            | `"normal"` or `"large"`                                    | `"normal"`                    | Appearance         |
  | `shortcuts.capture`    | a shortcut, as in the window: `Ctrl+Alt+KeyN`              | `"Ctrl+Alt+KeyN"`             | Keyboard shortcuts |
  | `shortcuts.clipboard`  | the same                                                   | `"Alt+Shift+KeyV"`            | Keyboard shortcuts |
  | `mirror.folder`        | an absolute path to an existing folder                     | `null` (`~/Documents/Kinige`) | Copies             |
  | `agent_access.enabled` | `true` or `false`                                          | `false`                       | AI agent access    |
  | `agent_access.mode`    | `"read"` or `"read_write"`                                 | `"read"`                      | AI agent access    |
  | `agent_access.chats`   | an array of `{"id", "title"}`; without `id`, by exact name | `[]`                          | › Visible chats    |

  Not settings and not in the file: `sidebar_collapsed`, `recent_emoji`, the database backup (state only), the language (from the system, ADR-0005).
- **File rules** (ADR-0010):
  - The app writes all keys with their current values. The exception is the default `mirror.folder`: `null` is written, so that the file contains no personal path.
  - A missing key means the value does not change. `null` means the default value.
  - The effective state is still in `app_setting`; the file is an input. The ADR-0009 visibility predicate in SQL does not change. Applying the file is one writer transaction, followed by re-registering the shortcuts and a `settings-changed` event to the windows.
  - Order: at start the file is applied before `register_saved` and before the MCP listener; after `restore_backup` the file is applied on top of the restored database.
  - No file: it is created from the current state (also on the first launch of this version). Deleting the file does not reset the settings.
- **Sync in both directions.** All work with the file happens in one thread.
  - Watching is polling once per 1 s on a fingerprint (`mtime`, size, mode, inode, “symlink”), without `notify`. The thread recognizes its own write by the hash of the written bytes and does not apply it again. If the hash matches, it is not an external edit: if the file differs from what the app would write from the database, it is rewritten from the database (ADR-0010).
  - An edit in the window: re-read the file → change one key → add the missing known keys → write to a temporary file, `fsync`, `rename`. If the file changed since the last read, the external edit is applied first.
  - The last edit wins.
  - If the file could not be parsed, the window does not overwrite it, changes from the window go only to the database, and the error is visible.
- **Errors without crashes:** an unknown key is a warning, and the key is kept in the file. A wrong type or value is an error for that key, and the previous value stays. Shortcut: “taken” or “not accepted by the system”, as in the window. Folder: not absolute, does not exist or is inside the data folder: refused. A chat that is `ai_excluded`, an ambiguous name or a chat that is not found: an error for that element.
- **Risk mitigations** for “any process with write access turns on agent access”. They do not reverse the decision:
  - a symlink or a file writable by the group or by everyone → the `agent_access.*` and `mirror.folder` keys are not applied, error;
  - every change of `agent_access.*` from the file is an `mcp_log` row (`client = "settings.json"`, `tool = 'settings_file'`, `args_json` holds the flag, the mode and the number of chats, without names);
  - next to the changed section, “Changed via settings.json · time”, for `mirror.folder` too.
- **IPC (§12.3):** the `settings` capability for the `settings` window:
  - `get_language`, `get_settings`, `set_text_size`, `choose_mirror_folder` (the dialog is attached to the settings window);
  - `set_shortcut`, `reset_shortcut`, `shortcut_recording`;
  - the six access commands of 4a;
  - the new `get_settings_file` (path, state, errors) and `settings_open_entry`;
  - `core:event:allow-listen` and `allow-unlisten`.

  The moved commands are revoked from `main`; `get_settings` and the `settings-changed` event stay; `open_settings` (profile → “Settings”) and a subscription to the `open-message` event are added. The capture window has exactly seven commands.
- **Documents:**
  - ADR-0010. What it contains:
    - where the file lives, the format and the keys;
    - “missing key” and `null`;
    - the file as an input, `app_setting` as the effective state;
    - polling instead of `notify`, the write order and the race rule;
    - error handling;
    - the accepted risk “writing to the file = agent access to the journal” and the mitigations;
    - rejected: JSONC, dropping `app_setting`, an MCP tool;
    - locality (not synced);
  - PRD changes, v0.12: §2.1 (profile menu, copy), §5.1 (shortcuts), §9.3 (turning on via the file, log), §11.3 (settings window), §12.3 (matrix), §15 (a “4b” line after 4a);
  - `CLAUDE.md` in the first commit: in “State”, “slice 4a accepted 29.10.2026”; in “Build and tests”, `screens.yml` and `scripts/screens/<slice>.sh`.

## Not included
- Syncing settings between devices (ADR-0001, “Not decided”).
- New settings: the day boundary (`DAY_BOUNDARY_MINUTES` is a constant), UI language, theme, density, `ai_excluded` for a chat (P1).
- JSONC with comments and editing the file while preserving formatting. A default settings file and a JSON Schema for editors.
- An MCP tool for settings: an agent edits the file through file system access.
- Remembering the size and position of the settings window. Search in settings. An “Open in editor” button.
- Windows and Linux (slice 10).

## What “done” looks like
- **Window:** 860 × 600 by default, smallest 640 × 440. On the left, a tree with a width from a token (collapsible, the selected item without a border); at the smallest height and with large text the tree does not scroll.
- **Scrolling:** on the right, the only scrollable area. Long lists only on subscreens; “Show more” for the log at the bottom of the page.
- **States:**
  - access off;
  - the log is empty;
  - the file has no errors;
  - the file could not be parsed;
  - key errors (several);
  - unsafe file permissions;
  - long paths and names wrap or are truncated with an ellipsis.
- **Storybook stories:** every page and state, en and ru, normal and large text. Narrow window: everything is reachable, compromises are acceptable.

## Checks
- **Rust:**
  - a “garbage hunter” for file parsing: random bytes, non-UTF-8, BOM, deep nesting, 1 MiB + 1, a non-object, huge strings and numbers. Errors only, no panic and no write to the database;
  - for each key: a wrong type, a value that is not allowed, `null`, absence, an unknown key;
  - a write keeps foreign keys; a broken file is not overwritten; a deleted file is recreated;
  - an own write is not applied again; an external edit is seen within 2 s (the interval is shortened in the test); the “window ↔ file” race;
  - a symlink and permissions 0664; `abort` during a write: the file is entirely old or entirely new;
  - applying at start before MCP; the file on top of `restore_backup`;
  - an `mcp_log` row for a change from the file; the log contains no paths and no file content (markers).
- **IPC (mock):**
  - each `settings` command is allowed for the settings window;
  - the settings window is denied `list_messages`, `search_messages`, `send_message`, `purge_message`, `restore_backup`, `import_*`, `capture_*`;
  - `main` is denied the moved commands;
  - the capture window has exactly seven commands.
- **Frontend on `MockApi`:**
  - the tree and `aria-current`, subscreens and “back”, keyboard;
  - the `settings-changed` event in both windows; file errors; “Entry deleted”;
  - en and ru keys match;
  - a CSS test: one element in the window with `overflow: auto|scroll`;
  - `scrollbars.spec.ts`: one scroll, the page width does not change with the warning. Baselines come from `visual-update.yml`.
- **Screenshots in `docs/screens/4b/`:** `scripts/screens/4b.sh` on the macOS runner (`gh workflow run screens.yml --ref slice/4b-settings -f slice=4b`). Input is AX only; the file is edited by the scenario.
  - Screenshots: 01 Appearance; 02 Shortcuts; 03 Copies; 04 Access (off); 05 the warning (next to it `no-shift.txt` with the frames before and after); 06 Visible chats; 07 Log (long); 08 File without errors; 09 after an external edit (the access section with “Changed via settings.json” and the log); 10 the main window with large text after a file edit; 11 the file could not be parsed; 12 key errors; 13 large text; 14 the smallest window size; 15 after `kill -9`.
  - Texts: `settings-file.txt` (a synthetic file before and after, `ls -l` with 0600) and `one-window.txt` (two presses of the menu item give one window).
  - The user checks ⌘, from the keyboard at acceptance.

## Touches sensitive areas
**Yes, `sync-crypto-guardian` before merge:**
- agent access is now set by a file: applying it, file permissions, the order at start and after a restore;
- a new value of `mcp_log.tool`, without a migration;
- the IPC matrix of the new window.

The SQLite schema, the sync protocol and encryption do not change. There are no new crates in the tree: `sha2` and `libc` become direct dependencies of `kinige-store` (they are already in `Cargo.lock` transitively, ADR-0010).

After the `sync-crypto-guardian` review (30.10.2026) the scope came to include: a safe restore from a backup when `mirror.folder` is in the file, applying the file on every poll under the rule “the hash matches, the database wins”, the “before/after” comparison and the log row in one transaction, resetting access before applying the file after a restore, matching Inbox by name, structured file errors without `serde` text in the log, protection from FIFOs and devices, the data folder with mode 0700.

## Open questions
None.
