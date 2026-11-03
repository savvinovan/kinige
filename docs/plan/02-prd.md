# Kinige (Project Forest) — PRD v0.18

**Type:** a personal journal and notes in the form of a "chat with yourself", local-first.
**Platforms:** desktop — macOS first, then Windows and Linux (Tauri 2). Later a web client (decrypts data in the browser) and a phone, if the main app catches on.
**For whom:** at first for the author (4 weeks of daily use); the goal is a product: self-hosted, a free core with all the main features and a paid tier with extended ones (§2.6).
**Stage:** planning → spikes → MVP.

The review this version is based on: [`01-review.md`](01-review.md). Research: [`research/`](research/). Mockups: [`references/`](references/) — **a mood reference, not a specification**.

## Version history

- **v0.18 (03.11.2026)** — for slice 7, attachments (`docs/slices/7-attachments.md`, KINIG-15, ADR-0014, ADR-0001 appendix §C.6):
  - §3.2: "hash check" became "hash and size check"; the copy runs outside the writer into a per-process `attachments/.tmp/<process>/`, the rename and the row go in one writer job, and the startup sweep (also a writer job) touches only what no live attach owns: other processes' temporary folders, files without a row (moved to `.orphans/`, deleted after 30 days; deleted at once if the id has a tombstone) and the state of rows whose file is gone (`missing`);
  - §4.2 / §4.3: `attachment` is the first synced attachment table (migration 7: groups `core`, `order`, `deleted`; `blob_key` from the OS CSPRNG and `remote_blob_id` UUID v7 created in the send transaction; hex encoding; an empty `body_md` is valid exactly when the entry has a live attachment); the local `attachment_local`, `draft_attachment` and `mirror_file` tables; limits 2 GiB per file, 10 per entry, 64 MiB per paste; thumbnails in `thumbs/` (960 px, EXIF stripped, built lazily by two threads); formats are decided by content (JPEG, PNG, WebP, GIF are `image`; HEIC, AVIF, TIFF, BMP, SVG, RAW are plain files; video and audio are file cards until the players);
  - §4.3: the Markdown copy puts the files in a folder per day next to the day file, `<chat>/<year>/<YYYY-MM-DD>/<name>`, linked as `![name](<YYYY-MM-DD/name>)` or `[name](<YYYY-MM-DD/name>)` above the entry text; only files the copy wrote itself are ever removed;
  - §5: the paperclip, paste and drop attach to the not yet sent entry; sending needs a text or an attachment; the tiles survive a restart and `kill -9` in the draft;
  - §6: albums of 1–10 photos and file cards in the bubble, the box sized from `width`/`height` before the file loads (the feed does not jump), a lightbox with ← / → and "Open in default app"; entries with no text are described by a summary ("Photo", "3 photos", a file name) in the reply quote, "On this day", search, the forest card and the capture context;
  - §7.2 / §7.4: attachments count 0.5 each in the day weight, the day card says "5 notes · 3 photos";
  - §9.1 / §9.3: search finds an entry by its file names (index version 2); an AI agent sees each entry's attachment metadata (kind, name, bytes, image size), never bytes, paths or keys;
  - §12.3: `attach_pick`, `attach_paste`, `attach_dropped`, `remove_draft_attachment` and `open_attachment` are for the main window only (the capture window keeps seven commands, `settings` is denied; negative tests `src-tauri/tests/ipc_slice7.rs`); the webview reads files only through the `kinige-media` protocol, which answers only the `main` webview (403 to others), serves only exact `thumb|full/<uuid v4>` paths, `full` only for an image of a shown format up to 64 MiB and never SVG; a drop gives JS a single-use token, never a path; a system app opens a clone, and only documents on an allowlist (everything else is revealed in the file manager, never run);
  - §16: the Telegram import copies photos, files, videos, voice messages and audio (stickers and animations stay placeholders), attaches them to entries imported earlier exactly once, and removes the "… not imported" line only from an entry that is unedited since import (`hlc(body) == hlc(core)`; no revision, an exception like ADR-0006); a path in the export is checked (relative, no `..`, no drive letter, inside the folder after resolving links, a regular file).

- **v0.17 (02.11.2026)** — for slice 6, the year's forest (`docs/slices/6-forest.md`, KINIG-14):
  - §7.4: the forest sits in a pane above the feed of the open chat (not in the trash or in search results): a flat strip by default (a 7 × 53/54 heat map like GitHub's, tinted by season, denser for fuller days), expanded into the board (an oblique projection, see below) with the feed below it; the strip can be hidden, and "Show forest" in the profile menu brings the strip back; no sidebar item. All chats count, archived ones included, the trash does not; a click opens the open chat if it has entries that day, otherwise the chat with the most entries, the Inbox on a tie. The card shows the date, "N notes · stage" and the start of the day's first entry (no photos line until slice 7);
  - the frontend gets the whole year table from Rust (`get_forest_year`: every day with its cell, season, notes, stage and excerpt); there is no wasm build and no TypeScript copy of the calendar rules (ADR-0013, slice 6 paragraph);
  - §12.3: `get_forest_year`, `get_forest_day_target` and `set_forest_view` are for the main window only (the excerpts are entry text); the capture window keeps seven commands, the `settings` window is denied the new ones (negative tests `src-tauri/tests/ipc_slice6.rs`); the view state is two `app_setting` keys (`forest_view`, `forest_mode`), local, not synced, not in `settings.json`;
  - §7.1: the board is drawn in an oblique projection with the weeks along the horizontal axis of the screen (found in slice 6: the 45-degree isometry fills a third of a wide pane; a week is (64, 12) and a weekday (-26, 50) world units, ADR-0013); the hit-test is the inverse of the projection; §7.5: the render script of slice 9 must draw for this projection, not for a 45-degree camera;
  - §15 / ADR-0013: `get_forest_year` reads the year's entries row by row over the `message_day` index and sums each day in Rust, with no cache table; on the CI macOS VM (100,000-entry bench) four runs gave p95 88, 159, 182 and 86 ms against the p95 < 100 ms budget (a SQL aggregate was tried and was slower, p95 244 and 290 ms); the user's decision of 03.11.2026 is to check it on the real Mac at acceptance and add a derived per-day stats table only later (KINIG-47) if it is noticeable;
  - §15, row 6: "simple assets, one level of detail + dot" became "placeholder shapes, three levels of detail" (dot, simplified, detailed; the shapes are replaced by atlases in slice 9).

- **v0.16 (02.11.2026)** — for spike S3 (KINIG-14) and ADR-0013, only §7.1, Q4 and §14:
  - forest grid: **B (7 × 53, weekday × week)** chosen; the board is 53 or 54 columns by year (the 54th column in leap years that start on the last day of the week); grid A is rejected and described in the spike report; the first weekday follows the system locale;
  - rendering: Canvas 2D with a layer cache for pan and zoom, no PixiJS; Q4 is decided and S3 is done (§13, §14).

- **v0.15 (01.11.2026)** — translated to English (KINIG-62); no content changes.
  - Added to §15 (slice 10a, `docs/slices/10a-windows-build.md`): Windows build ahead of slice 10 — the workspace compiles on Windows, an unsigned NSIS installer goes into the GitHub Release next to the macOS `.dmg`, CI has a Windows job, screenshots of the real window come from a GitHub Windows runner; AI-agent access (MCP, Unix socket) is disabled on Windows until named pipes (slice 10); Linux, signing and the updater stay in slice 10.

- **v0.14 (01.11.2026)** — for the task "CI and process" (KINIG-46), only §12 and the roadmap:
  - CI is GitHub Actions (not Gitea): Linux runners, a `macos` job (clippy and the app crate's tests on `macos-15`), `gitleaks`, `cargo-deny`, Dependabot (§12.1, item 2);
  - screenshots of the real window are taken by a GitHub macOS runner via `screens.yml`, not by `screencapture` on the user's Mac; the reviewer does not launch the app on the user's Mac (§12.1, item 3; §12.1a);
  - the list of "living" documents is brought in line with the ones that exist (§12.1, item 6).

- **v0.13 (31.10.2026)** — for slice 5 (`docs/slices/5-on-this-day.md`):
  - "On this day" and replies: `message.reply_to_id` is written into the `core` group of a user entry (UUID v4 of an entry in the same chat, immutable), checked in Rust, a reference without a foreign key; states of the old entry `live` / `trash` / `missing`; the quote is also shown on replies imported from Telegram (§4.2, §6, ADR-0001 appendix §C.5);
  - the "On this day" card: Inbox, the three most recent years in detail (up to two entries), the rest as one line; hiding "until the next journal day" is stored locally in `app_setting`, is not synced and is not written to `settings.json` (§2.1);
  - migration 6: a `reply_to_id` column added to the local `draft` table (the draft carries the entry being replied to) (§4.2);
  - Markdown copy: a line `↩ [YYYY-MM-DD HH:MM](<path>)` under the time heading of a reply (§2.1);
  - components: the "On this day" card, a year row, a compact entry with "Reply", a "Reply to entry" row above the input field, a quote inside the bubble (§11.3);
  - IPC matrix: `get_on_this_day`, `dismiss_on_this_day` added to the main window, `send_message` and drafts carry `reply_to_id`; the capture window still has exactly seven commands, the `settings` window is denied the new commands (§12.3);
  - slice 5 in the roadmap (§15).

- **v0.12 (30.10.2026)** — for slice 4b (`docs/slices/4b-settings.md`) and ADR-0010:
  - settings are a separate window with a tree of sections ("Appearance", "Keyboard shortcuts", "Copies", "AI agent access" with the sub-screens "Visible chats" and "Access log", "Settings file"), ⌘, and an item in the tray icon menu; the main window no longer shows settings (§2.1, §11.3);
  - the `settings.json` file in the data directory is an input for settings (text, shortcuts, copy folder, AI agent access): the app and a human or an agent edit it, changes are picked up within 1–2 s; the effective state stays in `app_setting` (§5.1, §9.3, ADR-0010);
  - agent access can be turned on through the file: an unsafe file (a symlink, writable by group or everyone) does not apply such keys, every change from the file is a line in the access log (§9.3);
  - IPC matrix: the `settings` window with its own capability, the moved commands are revoked from the main window (§12.3);
  - slice 4b in the roadmap (§15).

- **v0.11 (28.10.2026)** — for slice 4 (`docs/slices/4-telegram-import.md`), spike S5 and ADR-0008:
  - there is no `message.body_search` column and no `message_fts` table: the search index is the local, non-synced tables `search_fts` (FTS5 without storing the text, only word stems) and `search_map`; the second local table of migration 4 is `message_ext` (external import ids) (§4.2, §9.1; ADR-0001 appendix §C.3);
  - import from Telegram: "Saved Messages" → Inbox, private chats → new chats 💬, the original entry time is kept (`created_at`, `journal_date`), the HLC stamp is the moment of import (§5, §9).

- **v0.10 (22.10.2026)** — for the user's decisions on slice 2 (`docs/slices/2-quick-capture.md`) and ADR-0006:
  - default shortcuts: the capture window — **temporarily ⌃⌥N** instead of ⌥Space (on the author's machine ⌥Space and ⌥⇧Space are taken by ChatGPT and Gemini, ⌃Space and ⌃⌥Space by keyboard layout switching, ⌘Space by Raycast; 22.10.2026), saving the clipboard — ⌥⇧V; custom shortcuts are set in "Settings", a taken shortcut is shown there too (§5.1);
  - on success the clipboard hotkey blinks the menu bar icon, on failure it opens the capture window with the clipboard text and the error (§5.1);
  - appending (⌘Enter) is the first rewrite of the `body` group without an `edit` revision; the target rule: `origin = 'user'`, not older than 10 minutes, today's journal day (§5.1, §9.3, ADR-0006);
  - drafts are the local `draft` table (§5, §5.1); the capture window has its own capability (§12.3).
- **v0.9 (21.10.2026)** — for the user's decisions on slice 1 (`docs/slices/1-inbox.md`) and ADR-0005:
  - the Markdown copy is on by default, the default folder is `~/Documents/Kinige` (§2.1);
  - after sending, the entry simply appears in the feed, without a toast; undo is by deleting (slice 3) (§5; the decision "Undo after sending returns the text to the field" was **revoked by the user on 21.10.2026**: the toast after sending was removed, it shifted the feed);
  - the translation library is i18next + react-i18next (ADR-0005); the Inbox constant and the clarified set of migration 1 tables are in the ADR-0001 appendix, §C.1.
- **v0.8 (21.10.2026)** — for the user's decisions on ADR-0001 (protocol and keys):
  - Inbox has a fixed id, the same in all accounts, instead of "deterministic from the account" (§4.2, §10.2);
  - `remote_blob_id` is a client-generated UUID v7 instead of an HMAC of the hash (§4.1, §4.2, §10.2);
  - a `conflict` revision stores only the losing group (`body` or `placement`); a move conflict is shown like a text conflict; `flags` leaves no trace (§4.2, §10.2);
  - quarantine on resync is by the "the server knew the entry" criterion, not by HLC age; in the UI it is "Found old entries" (§10.2);
  - a message without a chat after a full pull is shown in Inbox instead of being hidden forever (§10.2);
  - recovery key: a "Save" button (PDF / print) instead of verification by typing, a strong warning (§10.3);
  - a full export includes the trash (§16);
  - the message field `origin` says who created it: the user, an agent, an import (§4.2, §9.3);
  - the UI is multilingual from the first slice: English, then Russian (§2.1, §15);
  - `unknown_json` → `groups_raw` (raw bytes of groups, ADR-0001) (§4.2, §10.4);
  - fixes from the guardian review (ADR-0001 rev 5 and 6): `purged` carries an HLC snapshot of all groups, children are deleted after the parent; purging a chat — first the chat, then the messages; the external Telegram import id is the `ext` field and a local index, not the entry id; a corrupted blob gets a new `remote_blob_id`; import "as a copy" reissues ids and blob keys; an agent appends only to its own entry; no `REFERENCES` in the schema; an open header in the envelope (§2.1, §4.2, §9.3, §10.2, §10.3, §16).
- **v0.7 (20.10.2026)** — for the user's feedback on the feed concepts (slice D1, ADR-0004):
  - the "warm dark paper" direction is canceled (the user does not like brown): a dark theme in the cool Nord palette; fallback options are dark blue as in Telegram or a neutral graphite (§11.2);
  - density as in Telegram: body text 13 px, days are small labels, "compact" by default (§6);
  - no duplicates: there is no chat name above the feed (it is highlighted in the sidebar), a day has one label without repeating the date (§6);
  - an input field with "file", "emoji", "microphone" buttons (§5); the emoji picker is MVP; voice notes with recording move from P2 to MVP, right after attachments; stickers and GIFs are P1 (GIF was in P2) (§2, §15);
  - every chat has its own emoji, the sidebar collapses to a strip of emoji (§2.1, §11.2);
  - from the second round of concepts: every message is a bubble without a tail (previously "no bubbles at all", §1.2 item 2); the time on every message right after the text; merging into a "page" is canceled (§2.1, §6);
  - from the live window: entry text in a grotesque (Inter) instead of Literata; the background and bubbles are darker, the text is almost white; the input field has no outline (a thin line only in focus); an entry counter on every chat in the sidebar; a profile row at the bottom of the sidebar; the feed is a ~720 px column in the center, a bubble up to ~65 characters; no tabs (§2.1, §5, §6, §11.2);
  - from ChatGPT and Claude: the background is a neutral graphite instead of the blue-graphite Nord, the accent is light blue; a "Send" button instead of the microphone when there is text; hover actions to the right of a message; sections in the sidebar (§2.1, §5, §6, §11.2, §15);
  - the journal is ready for AI agents: the MCP server `kinige-mcp` in MVP, slice 4a (§1.2 item 8, §2.1, §9.3, §12.3, §15); an AI assistant inside the app stays in P2.
- **v0.6 (19.10.2026)** — from Fable's advice (`research/advice-fable.md`) and the user's decisions:
  - the pet is cut entirely; "growth" is only the forest, and it rewards returning to entries;
  - "On this day" and replying to an old entry are in MVP; the forest is navigation through the feed;
  - Telegram import comes before daily use starts; Obsidian and Notion import is removed from the plan;
  - a continuous Markdown copy in a folder instead of a daily database snapshot;
  - quick capture with context and appending to the last entry;
  - `#tag` tags are in MVP;
  - the composer starts as a plain input field, CodeMirror when it is needed;
  - business model: paid — server hosting, the mobile client, art packs for the forest; AI on your own key;
  - rules for working with agents: a one-page slice specification, writing and checking agents, a slice budget;
  - the own server is **kept** (Fable suggested replacing it with storage — rejected by the user).
- **v0.5 (19.10.2026)** — from the independent reviews by Fable and Codex (`research/review-fable.md`, `research/review-codex.md`):
  - the sync protocol is rewritten (§10.2): the server does not merge, clients merge by field groups; deletion is an ordinary field, restore works; immutable operation envelopes; a cursor without gaps;
  - the key scheme is rewritten (§10.3): the login password is separated from the encryption key, two DEK wrappings, an honest statement about backups and metadata, a weaker guarantee for the web;
  - format versioning and compatibility of clients of different versions (§10.4);
  - identifiers are random UUID v4 (UUID v7 reveals the creation time);
  - updates, signing, notarization, local auto-backups, the IPC access matrix (§12.3);
  - we start using it earlier: quick capture right after the first slices, the design system grows slice by slice, S4 is split into three parts (§14–15);
  - contradictions left over from v0.3 are fixed.
- **v0.4 (19.10.2026)** — from the user's notes:
  - we develop the design system ourselves in the repository, we do not use Claude Design;
  - end-to-end encryption: the server and administrators do not see the data, deleting the account = deleting the keys;
  - the product is self-hosted: a free core + a paid extended tier;
  - one theme in MVP (dark), tests are not duplicated per theme;
  - import from Obsidian, Notion, Telegram;
  - semantic search is computed on the client (the server does not see the text).
- **v0.3 (19.10.2026)** — the user's decisions:
  - desktop first on Rust + Tauri 2 + React/TypeScript;
  - sync through our own server on the homelab;
  - web later, as a thin client;
  - AI: embeddings via OpenRouter instead of Apple's on-device models;
  - style: a calm "paper" journal;
  - the design system is made in Claude Design, illustrations and mockups in Codex, forest sprites in Blender.
  - CloudKit, SwiftUI, GRDB and Apple-specific capture points are removed.
- **v0.2** — Markdown as the content format, spikes, vertical slices, rules for the agent, XP as an append-only log.
- **v0.1** — the original PRD.

---

## 1. Vision

### 1.1. Essence

Write as fast as a message in a messenger. Reread it as a beautifully typeset journal. See your year as a forest where every day is a tree.

Three pillars, in order of importance:

1. **Capture** — a thought is saved in ≤ 3 actions, including from anywhere in the system via a global hotkey.
2. **Rediscover** — returning to old entries is easy and pleasant: the feed, search, "On this day", the forest.
3. **Grow** — the forest reflects the accumulated history, grows both from entries and from returning to them, and never punishes.

### 1.2. Principles

1. **Zero-friction capture.** No need to choose a document, a title, a template. By default everything goes to Inbox.
2. **Beautiful by default.** The look comes from typography, not from the user's actions. A message is a bubble without a tail, without avatars and without a split into "mine / theirs" (v0.7).
3. **Content over interface.** The interface is quieter than the content.
4. **Local-first.** Without a network and without a server everything except AI features works fully. The server is for sync, not for work.
5. **The user's data.** Full export to Markdown + JSON; the server is your own (self-hosted).
6. **End-to-end encryption.** Data is encrypted on the device; the server and its administrator see only encrypted records and metadata (§10.3). Deleting the account erases the keys and the data from the server.
7. **No punishments.** No streaks and no losses: a skipped day is just an empty cell of the forest.
8. **AI is optional, but the journal is ready for agents.** No MVP feature depends on a model. Sending text to OpenRouter and access by external AI agents via MCP (§9.3) happen only after they are explicitly turned on.
9. **Nothing is lost silently.** A conflict, a write error, running out of space — always visible and reversible.
10. **Checked by eye.** A feature is not done until it has been seen in the real app window (see §11).
11. **The design system is the source of truth for visuals.** Code implements the tokens and components from it and does not invent its own.

---

## 2. Scope

### 2.1. MVP (P0) — macOS

- Chats: Inbox (created automatically, cannot be deleted), creating, renaming, a chat emoji (chosen by the user; stored in `chats.icon`, §4.2) and color, pinning, archive. In the sidebar: the emoji, the name and the chat's entry counter (all entries except the trash; a quiet gray, not the accent — this is not "unread"; from 10 000 on — "12K" (Russian: "12 тыс.")); the sidebar collapses to a strip of emoji, the name on hover. Chats are grouped into sections with small gray headings: "Pinned", "Chats", "Archive" (collapsed) — together with pinning and archive in slice 3. At the bottom of the sidebar is a profile row (an avatar with initials + a name, by default "Me", as in Notion) with a menu: "Settings" (a separate window, ⌘,), "Export", "About"; with sync (slice 11) — "Account and sync" and a sync status dot on the avatar.
- Messages: sending text (Markdown), editing (revisions are stored, the history UI is P1), deletion with undo and a trash for 30 days, the "task / favorite" flags, changing the date, `#tag` tags.
- **"On this day":** when a day is opened for the first time, a card with entries from the same day of past years appears in Inbox (the three most recent years in detail — up to two entries per year, the rest as one line with the number of entries); an entry from the card can be replied to — the reply gets a quote of the old entry, the caption "N years later" (if at least a year has passed) and a link to it; in the Markdown copy the reply has a line `↩ [YYYY-MM-DD HH:MM](<path to the day file of the old entry>)`. The card can be hidden until the next journal day (locally, not synced).
- **Telegram import** (the JSON export of "Saved Messages" and private chats) — before daily use starts, so that the forest and "On this day" are filled right away. Deduplication by the Telegram message id: it is stored in the entry's encrypted `ext` field, a repeated import skips what was already imported (ADR-0001, "external import id").
- An emoji picker in the input field (§5).
- Attachments: images and files (paste from the clipboard, drag & drop), links (without previews — previews are P1).
- Voice notes: recording from the microphone with a button in the input field, playback in the feed (stored as an attachment; transcription is P2). They come right after attachments (§15).
- Feed: grouping by days, the time on every message, jump to a date, restoring the position; messages with a pause of less than 5 minutes stand closer to each other but stay separate (§6). Smoothness at large volumes — according to the actual measurement (§15).
- **Markdown copy:** the app continuously and one-way writes `Kinige/<chat>/YYYY/YYYY-MM-DD.md` (+ attachments) into the chosen folder; **by default the copy is on** and is written to `~/Documents/Kinige` from the first entry (the folder is changed in settings or with the `mirror.folder` key of the `settings.json` file; settings also carry the warning "the copy is plain text"). The Inbox folder is `Inbox` in any UI language. This is a backup readable without the app (Time Machine, Obsidian, grep), and an export that "is always already there". Plus a daily copy of the database file (the last 7) and an integrity check at startup.
- Search: full-text, aware of Russian word forms, filters by type, chat, date; jump to the message in context.
- Forest: the year view, growth stages, seasons; clicking a tree scrolls the feed to that day (the forest is navigation, not a separate gallery); a calendar alternative.
- Quick capture: in any program you press a keyboard shortcut (for example, ⌃⌥N; in the original version — ⌥Space) → a small input field appears on top of all windows, with the last 2–3 entries of today faintly above it → you write the thought → Enter, and it is in Inbox; ⌘Enter **appends** to the last entry (if it is less than 10 minutes old). A second shortcut saves the clipboard contents to Inbox without a window. Plus an icon in the tray / menu bar.
- Theme: **one, dark** (a neutral graphite with a light blue accent, §11.2). All colors are semantic tokens, so a light theme is added later as a second set of values without changing components and without duplicating tests. Text scaling, keyboard navigation, Reduce Motion, screen readers (basic).
- Export and import of all data.
- **UI languages:** all strings are in translation files from the first slice; the first language is English, the second is Russian; dates and numbers are formatted via `Intl` according to the system language.
- **MCP server for AI agents** (§9.3): Claude Desktop, Claude Code and other MCP clients read the journal and add entries; off by default, access per chat.

### 2.2. MVP+ — right after MVP

- Builds and acceptance on **Windows and Linux**.
- **Sync** through our own server on the homelab (2+ computers).

### 2.3. P1

- Semantic search and "similar notes" via OpenRouter embeddings (§9).
- The forest also grows from rereading: a tree gets "undergrowth" when a day is returned to (a reply, a favorite, opening from search).
- "Random entry" (⌘R) — weighted toward days with photos and favorites.
- Version history UI; moving between chats; pinning.
- Link previews — only on the client and with the setting turned on.
- Stickers: your own images and importing packs from Telegram.
- GIF: your own files and search. **External GIF search (Giphy, Tenor, etc.) reveals search queries and the IP to a third party** — it is turned on only by an explicit setting, like AI (§1.2 item 8); without it only your own GIFs are available.
- Light theme.
- A web client for the server (decrypts data in the browser).

### 2.4. P2 and later

A mobile app (Tauri 2 mobile or native — a separate decision), import from Day One and Apple Journal, video, tables, drawings, a monthly forest view, Year in Review, transcription, image search, questions to the database (RAG) and summarization via OpenRouter — an AI assistant inside the app (until then this role is played by an external agent via MCP, §9.3), a browser extension "Save to Kinige", key transfer from device to device (QR), a separate key per chat, encryption of the local database (SQLCipher).

### 2.5. Out of scope

Collaboration, public links, analytics, hosting servers for other people before a public release (these are the obligations of a personal data operator).

### 2.6. Product model

- **Self-hosted:** anyone can bring up their own server with one command (`docker compose up`: server + Postgres).
- **Free:** all the main features — writing, the feed, search, the forest, sync through your own server, export, encryption; AI on your own OpenRouter key.
- **Paid — what is not in the client's open source code:**
  - **Kinige Cloud** — turnkey server hosting: encrypted storage, versioned backups (market benchmark — $2–4/month; Obsidian Sync $4/month);
  - **the mobile app** in the store (or free with paid sync);
  - **art packs for the forest** (seasons, styles) — the Blender pipeline pays off as an asset; cosmetics without any effect on features.
- We do not make paid features inside the open client — they are easy to fork. The license (for example, AGPL for the server, MIT/Apache for the client) is Q13. Before publishing — a trademark check (the name "Forest" is taken; the name Kinige — check now).
- Hosting for others — only after a legal review (the server stores ciphertext, but accounts are personal data).

---

## 3. Architecture

### 3.1. Diagram

```text
┌──────────────────────── Tauri app (macOS / Windows / Linux) ───────────────────────┐
│  web/ (React + TS)                                                                  │
│    features/ chats · feed · composer · search · forest · settings                   │
│    ui/       design system components  ←  design/tokens (DTCG → CSS variables)      │
│    api/      KinigeApi (interface) ── TauriApi (invoke)  │ MockApi (tests, catalog)  │
│                                                          │ HttpApi (web, P1)         │
│  src-tauri/  thin layer of commands, windows, hotkey, tray                          │
└──────────────┬──────────────────────────────────────────────────────────────────────┘
               ▼
   crates/kinige-domain   rules: journal_date, day weight, forest stages, merging — no IO, compiles to WASM
   crates/kinige-store    rusqlite: migrations, FTS5, outbox, revisions, attachment files
   crates/kinige-sync     protocol (types) + push/pull client
               │  HTTPS
               ▼
   server/ (self-hosted)  axum + Postgres + attachment storage; stores only encrypted records
```

### 3.2. Boundary rules

- **The frontend does not know about Tauri.** All calls go through the `KinigeApi` interface. This gives:
  - the component catalog and UI tests run in an ordinary browser on `MockApi`;
  - Playwright tests do not depend on WebDriver for WKWebView, which does not exist on macOS.
- Rust → TypeScript types are generated automatically (`ts-rs` or `tauri-specta` — choose in slice 0), not written by hand.
- `kinige-domain` depends neither on SQLite nor on Tauri. The server reuses only the protocol types from `kinige-sync` — it does not see and does not merge the content.
- **The web client is not just `HttpApi`.** Because of encryption the logic (merging, search, forest) must work in the browser. So `kinige-domain` is built for `wasm32` from day one (checked in CI); storage for the web (SQLite in OPFS or IndexedDB) is a separate decision before the web client.
- All business logic is in Rust. The frontend has only presentation and screen state.
- **Which DB where.** SQLite only on the client (desktop), on the server — **Postgres** (many users, concurrent requests). There is still concurrency on the client — windows, sync, background tasks. Hence **a single writer**: all writes go through one thread that owns the connection in Rust, reads go through separate connections (WAL), `SQLITE_BUSY` is handled.
- **Attachment files** are imported atomically: copy to a temporary folder → hash and size check → rename → write to the DB; "orphans" are cleaned up at startup (ADR-0014).
- SQLite goes through our own `rusqlite` in the Rust core, **not** through `tauri-plugin-sql` (it is built on sqlx and moves SQL into the frontend).
- IPC security: by default Tauri commands are available to all windows. Each window has its own set of capabilities (the quick capture window can only "create a message").
- Tauri 2.12.x (the stable branch as of October 2026). Tauri v3 (alpha, with a CEF runtime) — we do not touch it until a stable release.

### 3.3. What to take from the old Kinige

The same language is enough to carry over the **techniques**, but not the model:

- transactional application of changes: "write to the DB → commit → only then the in-memory state";
- a test with injection of a write failure;
- FTS sync in the same transaction.

The block model, the egui code and `docs/01–10` are not carried over. The new project is a **new git repository** (Q8); the old code stays as an archive.

---

## 4. Data model

### 4.1. Rules

- Identifiers are **random UUID v4**, generated on the device (UUID v7 reveals the creation time to the server). The exception is `remote_blob_id`: a UUID v7, the time in it almost coincides with the upload time, which the server sees anyway (ADR-0001).
- Time is UTC in milliseconds + the device's IANA zone at the moment of creation.
- `journal_date` (`YYYY-MM-DD`) is fixed at creation from the local date, taking into account the setting "the day ends at HH:MM" (default 04:00: an entry at 00:30 belongs to yesterday); it does not change when the time zone changes; the user can change it manually.
- The feed, jump to a date, the forest and `day_activity` use **the same thing**: the order `(journal_date, created_at, id)`.
- Deletion is an ordinary `deleted_at` field (trash for 30 days, restoring clears the field); after 30 days — physical deletion and a service record `purged` for the other devices (§10.2).
- Every synced record stores HLC **per field group** (§10.2) and `schema_version`.
- Derived data (day activity, forest, tags, Markdown copy, embeddings) is cached but always recomputed from messages.
- On the device the data lies in SQLite as plain text (it is protected by the OS disk encryption: FileVault, BitLocker, LUKS); encryption of the local database is P2. Only encrypted envelopes go to the server (§10.3).

### 4.2. Schema (SQLite, draft for the ADR)

Sync service fields (`hlc_json` with `sv` per group, `server_known`, `groups_raw`, the tables `tombstone`, `quarantine`, `pending_unknown`, `change_outbox.acked_seq`) are in ADR-0001, appendix §C; the field groups are there too.

```sql
chat (
  id TEXT PRIMARY KEY,                -- UUID v4; Inbox has a fixed id, the same in all accounts (§10.2)
  title TEXT NOT NULL, icon TEXT, color TEXT,
  sort_key TEXT NOT NULL,             -- fractional index for manual ordering
  is_inbox INTEGER NOT NULL DEFAULT 0,
  ai_excluded INTEGER NOT NULL DEFAULT 0,   -- do not send the content to AI
  pinned_at INTEGER, archived_at INTEGER,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER,
  hlc_json TEXT NOT NULL,             -- HLC per field group
  schema_version INTEGER NOT NULL, groups_raw TEXT NOT NULL  -- raw bytes of the groups, incl. unknown fields and groups of newer clients
);

message (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL,
  body_md TEXT NOT NULL DEFAULT '',   -- CommonMark + GFM task lists, no raw HTML
  -- body_search: there is no such column (decided in slice 4, ADR-0008, ADR-0001 appendix §C.3): the index is the separate local tables search_fts and search_map
  journal_date TEXT NOT NULL,
  created_at INTEGER NOT NULL, created_tz TEXT NOT NULL, edited_at INTEGER,
  is_task INTEGER NOT NULL DEFAULT 0, task_done_at INTEGER,
  favorite_at INTEGER, pinned_at INTEGER,
  reply_to_id TEXT,                   -- slice 5: id of the entry being replied to (UUID v4, same chat); written into core, immutable; no FK — the old entry may be in the trash, deleted forever or not yet arrived (ADR-0001 appendix §C.5)
  origin TEXT NOT NULL DEFAULT 'user', -- who created it: 'user' | 'agent' (MCP, §9.3) | 'import_telegram'; does not change
  ext_json TEXT,                      -- external import id {"src","chat","msg"}, does not change; deduplication is the local index message_ext (ADR-0001)
  deleted_at INTEGER,
  hlc_json TEXT NOT NULL,             -- groups: core, body, flags, placement (chat_id, journal_date), deleted
  schema_version INTEGER NOT NULL, groups_raw TEXT NOT NULL
);
-- no FKs on synced tables: on pull a message may arrive before its chat
CREATE INDEX message_feed ON message(chat_id, journal_date, created_at, id);
CREATE INDEX message_day ON message(journal_date);

message_revision (                    -- synced, append-only
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,        -- 'edit': a full snapshot of the message; 'conflict': only the losing group (body or placement), ADR-0001 §3.4
  saved_at INTEGER NOT NULL,
  reason TEXT NOT NULL                -- 'edit' | 'conflict' | 'restore'
);

attachment (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL,
  kind TEXT NOT NULL,                 -- 'image' | 'file' | 'link' | 'audio' | 'video'
  sort_index INTEGER NOT NULL,
  sha256 TEXT, filename TEXT, mime TEXT, byte_size INTEGER,
  width INTEGER, height INTEGER,      -- known before loading → the feed does not jump
  duration_ms INTEGER,
  url TEXT, link_title TEXT, link_site TEXT,
  remote_blob_id TEXT,                -- id of the encrypted blob on the server: UUID v7, created together with the attachment; a re-upload gets a new id (ADR-0001 §7)
  deleted_at INTEGER, hlc_json TEXT NOT NULL, schema_version INTEGER NOT NULL
);

attachment_local (                    -- NOT synced: the state of the file on this device
  attachment_id TEXT PRIMARY KEY,
  state TEXT NOT NULL                 -- 'local' | 'uploading' | 'synced' | 'remote_only' | 'missing'
);

change_outbox (                       -- immutable prepared operation envelopes
  op_id TEXT PRIMARY KEY, entity TEXT, entity_id TEXT,
  envelope BLOB NOT NULL,             -- encrypted at the moment it is queued; a retry sends the same one
  created_at INTEGER NOT NULL
);
sync_state (key TEXT PRIMARY KEY, value TEXT);          -- cursor, server generation, device_id, HLC state

-- message_fts(body_search, extra_search) was replaced in slice 4 (ADR-0008, §C.3): search_fts (FTS5 without storing the text, only word stems) + search_map(rowid → message_id);
-- file names and link titles will be added by slice 7 with the same layout, chat titles are matched in Rust
```

Derived data (a view or a cache table, updated in the same transaction):

```text
day_activity(journal_date, note_count, media_count, weight)
```

### 4.3. Files

- Attachments lie in the app data directory: `attachments/<attachment_id>.<ext>`. **No deduplication in MVP:** a file belongs to one attachment and is deleted together with it after the trash (deduplication and a garbage collector — when mass photo import appears).
- Thumbnails are in a cache (`thumbs/<id>.jpg|png`, at most 960 × 960, EXIF stripped); they can be lost and recreated.
- Image dimensions are written at import, after the EXIF orientation is applied.
- The Markdown copy puts an entry's files in `<chat>/<year>/<YYYY-MM-DD>/` next to the day file, and the day file links them (`![IMG_0001.jpg](<2026-11-03/IMG_0001.jpg>)`, `[menu.pdf](<2026-11-03/menu.pdf>)`); on APFS on the same volume the copy is a clone and takes no extra space.

---

## 5. Input (Composer)

- At the bottom of the feed, grows up to ~40% of the window height, beyond that — an "expand" button into full-screen mode. **One editor and one format** in both modes.
- **We start with a plain input field** (90% of entries are 1–3 lines); Markdown is rendered in the feed. CodeMirror 6 is brought in for the expanded mode and live highlighting when the field starts getting in the way — spike S1 happens then too. The target implementation is CodeMirror 6 with Markdown and live highlighting: headings larger, `**bold**` in bold, syntax dimmed (the approach of Obsidian Live Preview — Obsidian itself is built on CM6 — and Bear). Formatting buttons insert syntax. A known risk: IME bugs in Safari/WKWebView next to widget decorations — so live highlighting should use mark decorations, and widget decorations (images, checkboxes) only outside the line with the cursor. Checked in spike S1.
- **Buttons as in Telegram:** on the left "attach file", on the right "emoji" and "microphone" (recording a voice note). The design draws all three from day one; in the app a button appears in the slice where its function works (emoji — slice 3, file — 7, microphone — 7a, §15) — there are no dead buttons. Stickers and GIFs (P1) open from the same window as emoji. When there is text in the field, the "microphone" is replaced by a round accent "Send" button with an arrow (as in Telegram and ChatGPT); Enter still sends. The field is a dark fill **without an outline**; in focus — a single thin line inside the field in the focus color (focus visibility, WCAG 2.4.7).
- Enter — send, Shift+Enter — new line; inside a code block Enter is a new line.
- Pasting an image or a file from the clipboard and drag & drop → attachments to the not yet sent message.
- Sending: a command in Rust → one transaction → the message in the feed < 100 ms → the entry simply appears in the feed: without a toast and without any other confirmation (as in Telegram). On a write error the text stays in the field, with the error above it. Undo is by deleting and editing from the hover actions (slice 3).
- A draft is stored for each chat and survives a restart.

### 5.1. Quick capture

| Where                                                                   | How                                                                                                                                                                                                                                                                                                                           | Result                  |
|-------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------|
| Global hotkey (default ⌃⌥N — temporarily, instead of ⌥Space, see v0.10) | An input window on top of all windows on the screen with the cursor, the last 2–3 entries of today above the field; Enter — a new entry, ⌘Enter — append to the last one (if it is the user's entry from today not older than 10 minutes; otherwise a new entry), Esc or a click in another window — close, keeping the draft | An entry in the main DB |
| Second hotkey (default ⌥⇧V)                                             | Save the clipboard text to Inbox without a window; success — a short blink of the menu bar icon, failure — the capture window with the clipboard text and the error                                                                                                                                                           | Same                    |
| Tray / menu bar                                                         | "New note", recent chats (on Linux the tray does not deliver mouse events — menu only)                                                                                                                                                                                                                                        | Same                    |
| Drag & drop onto the Dock icon (macOS)                                  | File → a new message in Inbox                                                                                                                                                                                                                                                                                                 | Check in a spike        |
| Browser extension                                                       | "Save to Kinige"                                                                                                                                                                                                                                                                                                              | P2                      |

The app is single-instance: opening it a second time activates the existing window. Closing the main window hides it, the app stays in the menu bar; quitting is ⌘Q or "Quit" in the tray icon menu. Both shortcuts are changed in "Settings" (recording by pressing, a "taken" check before saving, reset to default) or with the `shortcuts.*` keys of the `settings.json` file (ADR-0010); a shortcut the system does not accept is shown as "not assigned", everything else works.

---

## 6. Feed

- **Every message is a separate bubble** (a light fill, rounded corners, no tail and no avatar), all aligned to the left: it is immediately visible that the entries were written at different times and in different contexts. The bubble is as wide as the text, no wider than ~65 characters (a comfortable line length is 45–75 characters).
- **The feed is a column in the center** ~720 px wide, the messages inside it are aligned to its left edge; the input field is as wide as the same column (like the Telegram message column in a wide window). In a narrow window the column takes the full width.
- **One chat is open, there are no tabs.** Every entry is saved immediately in one transaction, a draft is stored for each chat (§5), so there is no reason to keep several chats open: switching is a click in the sidebar, as in Telegram. Tabs and several chats in one window are out of scope; if it is ever needed, then "open chat in a separate window".
- **The time is on every message**, small, in the tertiary color right after the text, on its last line, inside the bubble (as in Telegram), and not at the right edge of the feed: the text comes first, the time is next to it.
- The day label is a small "pill" in the center of the feed.
- **Density as in Telegram:** body text 13 px (ADR-0004), many entries fit on the screen. Density "compact" (default) and "spacious"; filters: all / media / favorites / tasks.
- **No duplicates.** Above the feed there is neither a heading nor a row with the chat name: the selected chat is highlighted in the sidebar anyway (in the collapsed one its emoji is highlighted). A row above the feed appears only if there is something useful in it (for example, a chat actions menu); for now the chat actions are in the chat's context menu in the sidebar. A day has one small label: "Today", "Yesterday", "March 14" (current year), "March 14, 2025" (past years) — without a second row with the same date.
- Messages with a pause of less than 5 minutes form a series: the gap between them is smaller than between series. They do not merge into a "page" (canceled in v0.7); each has its own menu.
- **Reply and quote (slice 5).** A reply is written from the "On this day" card: above the input field a row "Replying to the entry of <date>" appears with the first line of the old entry; in the reply's bubble there is a one-line quote at the top — a fill without a border, the caption "N years later · <date>" (if the difference is less than a year — "Reply to the entry of <date>"), a click opens the old entry in the feed. If the old entry is in the trash — "Entry in the trash", deleted forever — "The original entry was deleted"; the reply itself stays. The same for replies imported from Telegram.
- Context menu (right click): copy, edit, reply (P1; in slice 5 "Reply" exists only on the entries of the "On this day" card), task ✓, favorite, pin, move to chat, change date, delete.
- **Hover actions** (as in ChatGPT and Claude): on hover or focus, to the right of the bubble, at the level of its bottom line — a row of small line icons (it covers nothing and does not eat the density; in a narrow window — slightly above the bubble at its right edge, without covering its text): copy, edit, task, favorite, "more" (opens the same context menu). The hover zone is the whole row of the feed, the row does not fade on the way to it. In D1 "Copy" works, the rest — from slice 3.
- Clicking a `- [ ]` checkbox in the feed edits the message text (creates a revision).
- Fast scrolling shows a floating date; there is a jump to a date.
- The scroll position is saved for each chat.
- Data is loaded in pages by a cursor (keyset) from Rust; the frontend has a virtualized list with variable-height cells, the item key is the message id (spike S2). We do not use the paid `VirtuosoMessageList`.
- **Markdown rendering is safe:** raw HTML is turned off (markdown-it or remark without raw HTML), the result goes through DOMPurify, links open in the system browser, the CSP forbids inline scripts. Notes are user data, and the webview has access to Tauri commands.

---

## 7. Forest

### 7.1. Grid

**Chosen: B, weekday × week (decided 02.11.2026 after spike S3, ADR-0013).** Seven rows (weekdays counted from the first weekday), one column per week. The board is 7 × 53, or 7 × 54 in a leap year whose 1 January is the last day of the week (2012, 2040, 2068, 2096 with a Monday start; 2000, 2028, 2056, 2084 with a Sunday start): the width is a function of the year. The first column starts with 0–6 empty cells above 1 January. The ground is drawn in an oblique projection with the weeks along the horizontal axis of the screen (slice 6: the 45-degree isometry of spike S3 laid the year along a diagonal and filled a third of a wide pane), a strip of about 1 : 5 to 1 : 6; seasons are bands of about 13 columns; weekdays are visible; dates are read with month and weekday labels on the board edges. The first weekday follows the system locale (Monday by default), see ADR-0013.

Grid A (12 × 31, month × day) was rejected; it stays documented in the spike (`spikes/S3-forest/README.md`). The rest of this section applies to B.

Requirements:

- The position of a day is a pure function in `kinige-domain` (`forest.rs`): `(year, date, first_weekday) → (row, col)`, covered by tests for leap years, year boundaries and the 54-column years.
- **Hit-test by the grid, not by sprites:** point → inverse of the projection → cell. A tall tree overlaps its neighbors visually but does not intercept the click.
- The selected cell is highlighted, the trees in front of it become semi-transparent.
- Three levels of detail: dot/symbol → simplified sprite → detailed sprite. The grid does not change.
- Gestures: the wheel and pinch on the trackpad — zoom, dragging — pan, double click — zoom in, a "whole year" button, the keyboard arrows — moving between days.
- Rendering: **Canvas 2D as the base**, PixiJS v8 (WebGL) as an enhancement if Canvas does not hold 60 FPS. On Linux, WebKitGTK WebGL is sometimes "silently" software-rendered (the context is created, but without the GPU) and crashes on NVIDIA with DMABUF — so a path without WebGL is mandatory. Decided by spike S3: Canvas 2D with a layer cache, no PixiJS (ADR-0013).

### 7.2. Growth

- Day weight: `w = Σ over the day's messages min(1, len/200) + 0.5 × attachments`. Messages shorter than 3 characters without attachments do not count. The formula is in `kinige-domain`, the constants are configurable.
- Stage: `stage = clamp(round(6 × ln(1+w) / ln(1+w_max)), 0, 6)`, by default `w_max = 30` → 7 stages.
- The forest is derived: you deleted notes — the tree got smaller. The growth animation plays only when today's stage increases.

### 7.3. Seasons

- The season is determined by the cell's date: all four are visible on the year board at the same time.
- The "Southern Hemisphere" setting shifts the seasons by 6 months.
- The season switcher is a quick zoom to the corresponding part of the board, not a filter.

### 7.4. Interaction

- Hover → a day card: the date, the number of notes, photos, the first line. Click → the feed scrolls to that day; while the feed scrolls, the tree of the current day is highlighted.
- **Placement (decided 02.11.2026, slice 6):** a pane above the feed of the open chat. By default it is a **strip**, a flat weekday × week heat map (like GitHub's, 7 × 53 or 54, tinted by season, denser for fuller days, month letters on top); a button expands it into the board (an oblique projection: the 53 weeks run along the width of the screen; the feed stays below it) and collapses it back; another button hides it entirely, and "Show forest" / "Hide forest" in the profile menu is the only way back (there is no sidebar item). The pane is not shown in the trash or in search results. The state (hidden, strip, expanded; forest or calendar) is local (`app_setting`), not synced.
- **What a day is:** the live entries of all chats that are not in the trash, archived chats included. A click opens the open chat if it has entries that day, otherwise the chat with the most entries, the Inbox on a tie; the feed opens on the day's first entry the way a jump from search does. The outlined cell follows the day at the top of the feed, and when the feed crosses into another year the board and the strip switch to that year.
- The card shows only what exists in MVP.
- **An accessible alternative:** a "Forest / Calendar" switcher — an ordinary calendar with the same data; a screen reader announces "October 16, 5 notes, a large tree".

### 7.5. Art pipeline (risk #1)

1. **Codex (image generation, `$imagegen`):** style concepts — the season palette, silhouettes of the 7 stages, ground, decor. The result is `design/concepts/forest/` and an approved style sheet. Generation uses up Codex limits 3–5 times faster than ordinary requests, and consistency of a series across generations is not guaranteed — so the final sprites are not made here.
2. **Blender:** low-poly models based on the concepts. A render script (`bpy`) with a fixed camera (it must match the board's oblique projection, ADR-0013: the tree stands upright on a tilted ground, it is not a 45-degree isometric view), lighting and scale outputs all "stage × season × variant" combinations and assembles atlases. A re-render is one command.
3. **The app** loads the atlases and knows nothing about Blender.

Needed: ground, 7 stages × 4 seasons × 3 tree variants, 5–10 decor items, highlighting; later — "undergrowth" (P1) and paid art packs in the same projection. For spike S3 — simple shapes or CC0 placeholders.

---

## 8. Pet

Cut in v0.6: it is not connected to notes and pulls attention to itself rather than to the entries. The reward is the past itself — the forest, "On this day", reply threads across the years.

---

## 9. Search and AI

### 9.1. Full-text search (P0, offline)

- SQLite FTS5 over `body_search`, file names, link titles, chat titles.
- **Russian language:** when a message is written, Rust normalizes the text and stems it (Snowball for Russian and English) into `body_search`; the query is stemmed the same way. Prefix search for unfinished words. Alternatives (trigrams, a custom tokenizer) are in spike S5.
- **Decided in slice 4 (spike S5, ADR-0008):** there is no `message.body_search` column and no `message_fts(body_search, extra_search)` table. The index is a separate local FTS5 table without storing the text (`search_fts`) with a `rowid → message_id` table (`search_map`); it contains only the word stems of the entry, chat titles are matched in Rust. File names and link titles will get into the index in slice 7 with the same layout. ADR-0001 appendix, §C.3.
- Filters: type, chat, date range, favorites, tasks. A result opens in the feed with highlighting.

### 9.2. Semantic search via OpenRouter (P1, online)

- **Turned on explicitly**, the OpenRouter key is stored in the system secret store (Keychain / Credential Manager / Secret Service), not in the DB and not in git.
- **Embeddings are computed by the client**, not the server: the server does not see the text (§10.3). Vectors are stored locally and in P1 are **not synced** (each device indexes by itself — it costs cents, but there are no freshness problems).
- A vector is stored with the hash of the source text, the model and the dimensionality; an API result is accepted only if the text did not change during the request. Before every send it is checked again that AI is on and the chat is not excluded.
- Before the first indexing the following are shown: the volume (messages, tokens), a cost estimate, which chats are excluded (`ai_excluded`), where the data goes.
- API: `POST https://openrouter.ai/api/v1/embeddings`. Indexing 100k short notes costs cents; the model is chosen by quality on Russian in spike S6. Candidates (prices as of October 2026, $ per 1M tokens): `bge-m3` 0.01, `multilingual-e5-large` 0.01, `qwen3-embedding-8b` 0.01, `text-embedding-3-small` 0.02, `text-embedding-3-large` 0.13.
- Search is local, brute force: 100k vectors × 512 dimensions ≈ 0.2 GB in float32 or ≈ 50 MB in int8. Hybrid: FTS (BM25) + cosine similarity, merged via Reciprocal Rank Fusion. A model change → recomputation in the background.
- The user's query also goes to the API, so semantic search is an online feature; offline, FTS works.
- Images (P2): OpenRouter multimodal embeddings (`google/gemini-embedding-2`, `voyage-multimodal-3.5`, etc.) or a caption by a vision model.
- Privacy: Zero Data Retention (`provider.zdr: true`). But **end-to-end encryption does not extend to AI**: by turning AI on, the user sends text to OpenRouter and to the model provider — say exactly that in the UI.

### 9.3. MCP server for AI agents (P0, slice 4a)

Kinige is ready for agents: an external AI agent (Claude Desktop, Claude Code, Cursor and any MCP client) works with the journal via the Model Context Protocol — "what did I write about Raft in March?", "write the meeting summary into Kinige". An AI assistant inside the app is then not needed until P2.

- **Locally:** a separate binary `kinige-mcp`, stdio transport — the MCP client itself launches it on the same computer; there is no network port, the sync server is not involved.
- **Tools:** read — search (FTS, §9.1; semantic — once P1 exists), entries for a day and a range, "on this day", the list of chats, an entry by id; write — a new entry in Inbox or a chat, append to its own last entry (`origin = 'agent'`; the agent does not append to the user's entries — the "added by an agent" mark applies to the whole entry), mark a task.
- **Through the same rules:** reads and writes go through `kinige-domain` and the same operations as the UI (one transaction, revisions, undo, Markdown copy). The agent's entries are marked "added by an agent" in the feed (the field `origin = 'agent'`, §4.2). How the binary and the running app share the database and which MCP library to take is decided in ADR-0009: the MCP server (`rmcp`) runs inside the app, `kinige-mcp` is a thin stdio ↔ Unix socket bridge, it does not open the database; an agent's appending writes an `edit` revision (an exception from ⌘Enter, ADR-0001 appendix §C.4); if the app is not running, the bridge itself launches it in the background (an icon in the menu bar, no window).
- **Privacy and permissions:**
  - off by default, turned on in settings with a warning: **text read by the agent goes to the provider of its model**, end-to-end encryption does not extend to it (like AI, §9.2). Access can also be turned on with the `agent_access.*` keys of the `settings.json` file (ADR-0010): then there is no warning, so every such change is written to the access log ("settings.json · access changed") and is marked in the window; a file writable by group or everyone, or a symlink, does not apply these keys;
  - access per chat (a list of allowed ones; `ai_excluded` chats are never visible to the agent);
  - the "read only" and "read and write" modes are turned on separately;
  - the agent's access log: when, which tool, which entries — visible in settings (the "Access log" sub-screen);
  - in MVP the agent cannot delete, move or edit other people's entries.

---

## 10. Sync (MVP+)

### 10.1. Server

- `server/` — Rust (axum) + **Postgres** + storage for encrypted blobs (disk or S3-compatible). According to Fable, MinIO has been in maintenance mode since late 2025 — check alternatives (Garage, SeaweedFS) in S4c.
- **Self-hosted with one command:** a Docker image and `docker-compose.yml` (server + Postgres), migrations at startup.
- Multi-user: accounts, device binding, storage quotas.
- **The server does not see and does not merge the content.** It stores a log of encrypted envelopes and hands them out; all conflict semantics are on the clients.
- **Backup and restore:** a consistent snapshot of Postgres + blobs (a blob is deleted only after no record in the backups within the retention period refers to it). After a restore the server has a new "generation" — clients see it and do a full resync instead of continuing from the old cursor. A mandatory check is restoring into a clean installation.
- The first installation is our own homelab: the homelab deployer's 1 GiB volume is too small for media → blobs on NFS or the S3-compatible storage of the k3s cluster (Q9).

### 10.2. Sync protocol

**Entities and field groups.** Every synced record (chat, message, revision, attachment) is a set of field groups, each group has its own HLC. A message has: `core` (written at creation; `origin` and `ext` are immutable), `body`, `flags` (task, favorite, pin), `placement` (chat, date), `deleted`. Concurrent edits of different groups on two devices are not lost.

**HLC.** A fixed-width format, sortable as a string: `{wall_ms:013}-{counter:05}-{device_id}`; on a tie `device_id` decides. The clock state is saved in `sync_state`. If an incoming stamp is ahead of the local clock by more than 10 minutes — a warning to the user, work is not blocked. The implementation is an ADR with all the rules and convergence property tests (this is not "50 lines").

**Operations.** Every change is written to `change_outbox` as an immutable envelope with a unique `op_id`, encrypted at the moment it is queued, in the same transaction as the change. A retry sends the same envelope.

**Push.** The server accepts envelopes idempotently by `op_id`; the same `op_id` with different content is an error. The server assigns the `seq` number under an account lock: numbers are issued and committed in order (an ordinary Postgres sequence does not guarantee this — the cursor could skip an uncommitted record).

**Pull.** The client receives the envelopes after its cursor, verifies authenticity (AEAD) **before** applying, applies the page and saves the cursor in one transaction. It recognizes its own operations by `op_id` and skips them.

**Merging (on the client, in `kinige-domain`):**
- creation is conflict-free;
- for each field group the larger HLC wins;
- if a concurrent change of `body` or `placement` (a move to another chat or to another date) lost, the losing group is saved as `message_revision(reason='conflict')`, the message gets a "there is another version" mark with a choice of which one to keep; concurrent edits of `flags` are resolved without a trace (concurrency rules — ADR-0001 §3.4);
- **deletion is an ordinary `deleted` group:** a restore from the trash with a newer HLC wins over an older deletion;
- **Inbox:** a fixed id, the same in all accounts; when a local database is first bound to an account, the local Inbox merges with the server's one without a conflict, references are preserved.

**Dependency order.** A message may arrive before its chat, an attachment before its message: a page is applied in one transaction, without FKs on synced tables; a record without a parent waits for it and is not shown until pull has reached the head of the log. After that a message without a chat (the chat is deleted or unavailable) is shown in Inbox with a mark — derived, without changing the message itself. The exception is old messages of a chat being finally deleted: purge goes "first the chat, then its messages", and between the two they are hidden (ADR-0001 §3.6).

**Tombstones and old devices.** 30 days after deletion a record is physically deleted, and a `purged` operation is written to the log. It carries an HLC snapshot of all groups of the record as the deleting device saw them; the server rejects it if it knows a newer group (the record was restored or edited), and a client with a newer group of its own moves the record to quarantine instead of deleting it silently. Children (attachments, revisions, messages of a chat being deleted) are deleted only after the parent is confirmed (ADR-0001 §3.5, §3.6). The server keeps `purged` for at least 180 days. A device with a cursor older than that period gets "cursor is outdated" and does a full resync. A local record that does not exist on the server: if the server has never seen it — it is sent; if it has seen it (meaning it was deleted and the tombstone expired) — it goes to the "Found old entries" quarantine for a manual decision, without resurrecting what was deleted (the table is in ADR-0001 §6.3).

**Log compaction.** The server may delete envelopes superseded by newer ones in all field groups (this is visible from the open metadata: entity id, group, HLC), but not earlier than the tombstone retention period.

**Attachments.** A blob is encrypted as a stream (libsodium `secretstream`, 1 MB chunks), immutable after publication, the upload is resumable, integrity is verified. The id on the server is a UUID v7 created by the client together with the attachment (it lies in the attachment's encrypted data); a blob corrupted on the server is uploaded again under a new id. A blob is deleted together with its attachment after the trash; revisions do not refer to attachments.

**Off-the-shelf engines** (ElectricSQL, PowerSync, Turso Sync) require the server to see the data — they do not fit with encryption. A CRDT for text is not needed: messages are short and rarely edited.

**Criteria (S4a):** two and three clients in a simulation with random delays, duplicates and disconnects converge to the same state; a text conflict keeps both versions; a restore from the trash reaches another device; a device that comes back after 200 days does not resurrect what was deleted; a repeated push is idempotent.

### 10.3. End-to-end encryption

**Keys:**
- **DEK** (data encryption key) — random, generated at registration; all envelopes and blobs are encrypted with it.
- **Two independent keys** are derived from the password via Argon2id: `auth_key` for login (the server stores only its hash and never sees the password) and `KEK` for encrypting the DEK. The salt and the Argon2 parameters are stored on the server in the open.
- **The recovery key** (24 words, the English BIP-39 list) is shown at registration, recovery and reissue with a "Save" button (PDF or print); there is no verification by typing. You cannot proceed until the words are saved, and they go to the server only after saving. The warning is large: without the words and the password, access to the entries on the server is lost forever, nobody can restore it, including the developers. The server holds two DEK wrappings: by the password (KEK) and by the recovery key.
- **Changing the password** = a new DEK wrapping, the data is not re-encrypted.
- **A new device** in MVP+ — only by password; key transfer from a bound device (QR) is P2.
- **Revoking a device** revokes access to the server but cannot "take back" a DEK already copied to the device; DEK rotation is P2.

**Operation envelope:** `{format_version, key_id, header, nonce, ciphertext}`, the cipher is XChaCha20-Poly1305; `header` is an open JSON with the type, id, `op_id` and groups with HLC (bytes — ADR-0001 §5, appendix §A.1). The associated data (AEAD) includes: the account, entity type, id, `op_id`, groups and HLC, the `purged` flag, `schema_version`. Therefore the server cannot silently change the HLC, the deletion flag or move an envelope to another record.

**What the server sees** (honestly, as with Day One): the account, the record type, ids (random UUIDs), the time of changes (via HLC), the size of envelopes and blobs, the fact of deletion. **It does not see:** text, chat titles, `journal_date`, tags, attachments. Sizes are padded to 1 KB. Hiding the activity rhythm itself is a separate complex protocol, not planned.

**Account deletion:** keys, envelopes and blobs are deleted from the server. Old server backups contain the wrapped DEK, so the promise reads: "backups are kept for N days and then rotated; until rotation the data in them is protected by your password". Real cryptographic erasure of backups (a server-side account key outside the dump) is P2.

**Threat model:**
- **Desktop:** the server administrator cannot read the data — provided that client updates are signed and verified (§12.3).
- **Web client — a weaker guarantee:** the server serves the page code, and a malicious administrator can swap the JavaScript. This is stated directly in the web client's UI. Mitigation (a separate origin, a reproducible build, SRI) is P2.
- **AI** is not covered by the protection: by turning it on, the user sends text to OpenRouter.

**The cost of the decision:** losing the password and the recovery key = losing the data on the server (local copies remain); search and embeddings are only on the client.

Only well-known primitives (libsodium / RustCrypto), no home-made cryptography; before a public release — an external audit.

### 10.4. Versions and compatibility

- Every envelope has `schema_version` (per group, ADR-0001 §4). The client **keeps unknown fields and groups** (raw bytes of the groups, `groups_raw`) and returns them on re-sending, so an old client does not erase a new client's data.
- A record with a `schema_version` newer than the client can understand is shown read-only with an offer to update.
- The server announces the minimum protocol version; a client that is too old gets a clear error.
- Before migrating the local database the client makes a copy of it; rolling back the app after a migration is not supported — there is a warning about this.
- The envelope format and the protocol are a public contract for self-hosted users: they change only through an ADR and a version.

---

## 11. Design

### 11.1. Process and tools

We develop the design system ourselves, in the repository — we do not use Claude Design. This way it has a history in git, it is in the code right away and it carries over to other platforms through tokens.

```text
Codex ($imagegen)        →  mood, screen concepts, forest concept art (references only)
        ↓  the user's choice
design/tokens/*.json      →  tokens (DTCG format): colors, typography, spacing, radii, shadows, animations
        ↓  Style Dictionary
web/src/ui/tokens.css     →  CSS variables (later — Swift / Kotlin for a native mobile app)
        ↓
web/src/ui/*              →  React components
        ↓
Storybook (or Ladle)      →  living documentation of the design system: all components, states, screens,
                             three feed variants; visual tests capture exactly this
```

Rules:

- **Images from Codex are not a specification.** This is a lesson from the old Kinige: the mockup lived its own life, the implementation its own.
- **Visuals change only through tokens and components** in the repository; a new color or spacing first appears in the tokens.
- Tokens are named by role (`color.text.primary`, `space.4`, `radius.card`), not by value — so a theme and a platform change by replacing values.
- **Themes without duplicating tests:** components know only semantic tokens. Component visual tests run on one theme; for each theme only the token set is checked separately (contrast of "text / background" pairs, all tokens are defined) and a few screenshots of key screens are taken.
- Until the design system foundation (D1) is assembled in Storybook, slices with UI do not start; after that, components are added in their own slices.

### 11.2. Direction

**A calm journal with the density of a messenger:**

- MVP — **a dark theme: neutral graphite** (gray without a color tint, like the dark themes of ChatGPT and Claude), almost white text, a single light blue accent (nord8 `#88c0d0`). The "warm dark paper" with a brown undertone was canceled in v0.7 (the user does not like brown); the blue-graphite Nord was tried in the live window and replaced with graphite (ADR-0004). The light theme is a second token set in P1;
- a sans-serif grotesque with good Cyrillic both for entry text and for the UI (Inter), a monospace for code (JetBrains Mono). The serif (Literata) is canceled: at 13–15 px serifs add noise;
- **high contrast:** dark background and bubbles, almost white text (body text on a bubble ≥ 12:1, not "around AA");
- density as in Telegram (§6): compact text, small day labels, a minimum of empty space;
- a muted accent;
- message bubbles without tails and shadows, thin dividers;
- a minimum of chrome around the text, no duplication (§6).
- **Emoji:** every chat has its own emoji — this is user data, not decoration. We do not use emoji as UI decoration (in buttons, headings, empty states); UI icons are line icons.

References: Telegram (density, input field, sidebar), Bear, Apple Journal, Day One, Things. The dark purple style of the mockups that were sent **is not used**. From the mockups we take the screen structure, but not the style.

### 11.3. Contents of the design system

- **Foundation:**
  - colors (roles: background, paper, text ×3, accent, divider, selection, forest seasons; values for the dark theme, the light one later);
  - the type scale (day heading, H1–H3, text, small, code, metadata — with sizes and line heights);
  - spacing, radii, shadows, animation durations and curves.
- **Fonts:** embedded in the app (woff2, OFL license), with Cyrillic and **all the needed styles** (regular, italic, semibold/bold — the old Kinige had no bold). System fonts differ across the three OSes, so we cannot rely on them.
- **Components:**
  - a message in all content variants (text, headings, lists, tasks, code, quote, image, file, link);
  - the day divider, the time label;
  - the composer (empty, multi-line, with attachments, expanded) with the "file", "emoji", "microphone" buttons; the emoji picker; voice note recording;
  - a chat list item (emoji + name + entry counter), the collapsed sidebar (a strip of emoji), the profile row with a menu, search and results;
  - the settings window (slice 4b): a tree of sections on the left (the selected item is highlighted, an error mark), a single scrollable area on the right, sub-screens with a "back" heading, a "Configure ›" row; warnings are on top of the window and do not shift the content;
  - the context menu, the "Undo" toast (only for reversible actions like "Entry deleted"; it overlays and never shifts the feed content; there are no confirmations of routine actions — sending, saving);
  - the "On this day" card (slice 5: a paper fill without a border and shadow, a year row, a compact entry with "Reply"), the "Reply to entry" row above the field and the quote in the bubble (a fill, no border);
  - the forest day card, the quick capture window, buttons and fields.
- **Key screens:** the main window (sidebar + feed + composer), search, the forest, the quick capture window.
- **Feed variants** are static Codex concepts on the same data; the user chooses one, only that one is implemented in Storybook. First round: Editorial / Conversational / Immersive; after the feedback (v0.7) the compact Editorial and the compact Conversational remained (ADR-0004).

---

## 12. Process, quality and rules for the agent

### 12.1. Mandatory rules

1. **git from the first commit.** One slice = one branch = one PR. No edits of source files with `str.replace` scripts.
2. **CI before the first feature** (GitHub Actions, `.github/workflows/ci.yml`: Linux runners, plus `macos-15` for code under `cfg(target_os = "macos")`; the `.app` is built and signed locally by the `scripts/build-app.sh` script, a signed build in CI is slice 10): `cargo test`, `clippy -D warnings`, `rustfmt`, `tsc`, ESLint, frontend tests, Storybook visual tests, secret scanning (gitleaks), dependency licenses and vulnerabilities (cargo-deny).
3. **"Done" only with evidence:**
   - test output;
   - screenshots of the app's **real window**, normal and large text: they are taken by a GitHub macOS runner (`gh workflow run screens.yml --ref <branch> -f slice=<id>`, the scenario `scripts/screens/<slice>.sh`, synthetic data, input only through accessibility) and committed to the branch; the app is not launched on the user's Mac for this;
   - all of it in `docs/screens/<slice>/`.
4. **Visual acceptance is by a human.** A slice is closed when the user has looked at the screenshots or the build and agreed. The next slice does not start before acceptance.
5. **A decision = an ADR** (`docs/adr/NNNN-*.md`, half a page). A new dependency (crate or npm) only through an ADR or an explicit mention in the PR.
6. **There are few documents, and they are living:** `README.md`, `CLAUDE.md`, the PRD (`docs/plan/`), `docs/adr/`, `docs/slices/`. Separate `architecture.md`, `roadmap.md` and `forest-spec.md` were not created: their content is in §3, §7 and §15 of the PRD. Outdated material is deleted in the same PR.
7. **Code by feature**, no abstractions for future features. A file over 400 lines is a signal to split it (except migrations and generated types).
8. **Do not make up facts** about libraries and benchmarks — verify them or mark them "not verified".
9. **Visuals only from the design system:** new colors, sizes and shadows first appear in the tokens.

### 12.1a. How to work alone with agents

- **A one-page slice specification before starting:** the scenario in words, what is NOT included, what "done" looks like (a reference screenshot or a description of the window). Without it the agent does not start, and you do not accept.
- **Two agents, two roles:** one writes, the second — in a clean context, without the first one's history — checks the PR against the specification and the DoD, **builds the app itself and checks the scenario** on test data, but does not launch its window on the user's Mac: the screenshots of the real window come from the macOS runner (`screens.yml`). "Check yourself" is useless for a single agent.
- **You look at the screenshots and the live window**, the code selectively. A PR without a screenshot of the real window does not go to acceptance.
- **The slice budget is in days.** A slice that has run past double its estimate is stopped and cut down, not pushed through.
- **A spike is opened only when its result is needed by the next slice.**
- **An ADR is written after the decision, in 10 minutes**; if there is more than one a week, you are designing, not doing.
- From the start of daily use the project log ("what broke / what I want") is kept in Kinige itself — it is both the backlog and a test of capture.

### 12.2. Definition of Done for a slice

- [ ] The slice scenario works in the built app on macOS (from MVP+ on — also on Windows and Linux).
- [ ] Data survives killing the process and a restart.
- [ ] Tests: Rust unit + integration tests on SQLite, frontend tests on `MockApi`, visual snapshots of new components in Storybook (normal and large text, one theme).
- [ ] Screenshots of the real window in `docs/screens/<slice>/`, compared with the Storybook stories.
- [ ] Keyboard navigation and screen reader labels on new elements, Reduce Motion taken into account.
- [ ] README / roadmap / ADR updated if decisions changed.
- [ ] Accepted by the user.

### 12.3. Operations and security

- **Signing and updates:** CI signs builds from the first slice (ad-hoc at first). The ADR "where the signing, notarization and updater keys live" is in slice 0. The updater (`tauri-plugin-updater`, signing is mandatory) with an update server on the homelab is in slice 10. Risks and costs include an Apple Developer ID and a certificate for Windows.
- **IPC:** the app's own Tauri commands are available to all windows by default. A matrix "window → commands → allowed arguments" (`AppManifest::commands` + capabilities) and negative tests are needed: the quick capture window can only create an entry, append to the last one and read the last 3 entries of today — but not search and not arbitrary notes. Added in slice 2 (ADR-0006): the capture window gets its own draft, "hide its own window" and "fit its own window's height to the content" (`capture_save_draft`, `capture_hide`, `capture_resize`: it accepts only a number, Rust clamps the range and pins the bottom edge; the window's size and position cannot be changed otherwise); the main window gets the draft of its chat, changing shortcuts and a subscription to the "entries changed" event (`core:event:allow-listen`); no window gets the permissions of the clipboard, dialog, opener and shortcut plugins — everything goes through the app's commands. Added in slice 3: the main window gets the commands for chats, entry actions, the trash, recent emoji, database backups and recovery (`restore_backup` and `restart_for_recovery` work only while the journal is not open or the corruption mark is set), the "open chat" event from the tray icon menu (`open-chat`) and the "database corrupted" event; the capture window gets exactly its seven commands — the six from slice 2 and `capture_add_recent_emoji` (a choice in the capture window's emoji picker goes into the shared "recent" list; the command accepts neither a chat nor an entry), the rest are denied to it (negative tests `src-tauri/tests/ipc.rs`). Moving an entry to another chat (`move_message`, a decision from slice 3 acceptance) is for the main window only; the upper limit of the capture window height is raised to 640 px for the sake of the emoji picker. Added in slice 4b (ADR-0010): the `settings` window with its own capability — `get_language`, `get_settings`, `set_text_size`, `choose_mirror_folder`, shortcuts (`set_shortcut`, `reset_shortcut`, `shortcut_recording`), the six access commands of 4a, `get_settings_file`, `settings_open_entry` and a subscription to events; it has no feed, search, writing, import or recovery (through `list_agent_log_entries` it sees the texts of entries touched by the agent — deliberately, like the "Access log" section). The moved commands are revoked from the main window (`get_settings` and the `settings-changed` event remain); the capture window still has exactly seven commands. Added in slice 5: the main window gets `get_on_this_day` and `dismiss_on_this_day(date)`; `send_message`, `get_draft` and `save_draft` carry `reply_to_id` (Rust accepts only a UUID v4 of a live entry of the same chat, otherwise `reply_target_not_found`, nothing is written); the capture window does not write replies (`capture_send` does not accept `reply_to_id`), the `settings` window is denied the new commands (negative tests `src-tauri/tests/ipc_slice5.rs`). Added in slice 6: the main window gets `get_forest_year(year, first_weekday, weekday_source?)`, `get_forest_day_target(date, current_chat_id)` and `set_forest_view(view, mode)`; the capture and `settings` windows are denied them (the year table carries the start of entry texts), the capture window still has exactly seven commands (negative tests `src-tauri/tests/ipc_slice6.rs`); `weekday_source` goes only to the debug log, without a locale string. Added in slice 7 (ADR-0014): the main window gets `attach_pick(chat_id)` (the file dialog runs in Rust), `attach_paste` (a raw body, `x-chat-id` and `x-name-hint` headers), `attach_dropped(token, chat_id)` (the token comes from the `attachments-dropped` event, is single-use and expires; the paths are read from the `main` window's drop event in Rust and never reach JS), `remove_draft_attachment` and `open_attachment`, and `send_message` carries `attachment_ids`; the capture window still has exactly seven commands and the `settings` window is denied the new ones (negative tests `src-tauri/tests/ipc_slice7.rs`). The `kinige-media://localhost/{thumb|full}/<uuid>` protocol is outside the capability system: its handler answers only the webview labelled `main`.
- **MCP (§9.3):** stdio only, no network port; the agent's tools have a separate matrix "tool → chats → read/write" with negative tests (a forbidden chat is visible neither in search nor by id); every access is written to the log.
- **Links and network:** only `http`, `https`, `mailto` are opened and only in the system browser; external images in the feed are not loaded; the preview loader (P1) does not go to local addresses (SSRF protection).
- **Local backups:** the Markdown copy + a daily copy of the database file (slice 3), `PRAGMA integrity_check` at startup; the Markdown copy and the export are plain text, and the UI says exactly that.
- **Linux:** the global hotkey on Wayland may not work — the fallback is "tray + a hotkey inside the window"; if there is no Secret Service, the OpenRouter key goes into a file with 0600 permissions and a warning.
- **Write reliability:** killing the process and power loss are tested separately (on a test machine — pulling the plug on a virtual machine).

---

## 13. Open questions

| #   | Question                                                                                                                   | Status                                                                                                                                                                                                           |
|-----|----------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Q1  | Who the product is for                                                                                                     | **Decided:** first for myself, the goal is a self-hosted product with a free core and a paid tier                                                                                                                |
| Q2  | Platforms                                                                                                                  | **Decided:** desktop (macOS → Windows/Linux) on Tauri 2; web and phone later                                                                                                                                     |
| Q3  | Sync                                                                                                                       | **Decided:** our own self-hosted server with end-to-end encryption; the first installation is the homelab. Fable's alternative (device logs in S3/a folder without a server) was considered and rejected in v0.6 |
| Q4  | Forest grid                                                                                                                | **Decided (02.11.2026):** B, 7 × 53 or 54 by year, ADR-0013                                                                                                                                                      |
| Q5  | Visual direction                                                                                                           | **Decided (v0.7):** a dark theme, neutral graphite with a light blue accent, density as in Telegram (§11.2, ADR-0004); "warm paper" and the blue-graphite Nord are canceled                                      |
| Q6  | Art                                                                                                                        | **Decided:** Codex — concepts, Blender — sprites                                                                                                                                                                 |
| Q6a | Pet                                                                                                                        | **Decided (v0.6):** cut                                                                                                                                                                                          |
| Q7  | Frontend                                                                                                                   | **Decided:** React + TypeScript                                                                                                                                                                                  |
| Q8  | Where the new repository lives                                                                                             | **Decided:** `petproject/kinige-claude`, the plan is in `docs/plan/`; the old `kinige` is an archive                                                                                                             |
| Q9  | Where to store attachments on the server (NFS, MinIO) and how to expose the API to the outside (Cloudflare, device tokens) | Spike S4                                                                                                                                                                                                         |
| Q10 | OpenRouter embedding model                                                                                                 | Spike S6                                                                                                                                                                                                         |
| Q11 | Fonts (serif + sans pairs with Cyrillic)                                                                                   | **Decided (D1):** Inter (text and UI) + JetBrains Mono (code); Literata is canceled (ADR-0004)                                                                                                                   |
| Q12 | Password and recovery key: what the onboarding looks like so that data is not lost                                         | Spike S4                                                                                                                                                                                                         |
| Q13 | What exactly is paid and which license (for example, AGPL + commercial); trademark check                                   | Decide before the public release                                                                                                                                                                                 |
| Q14 | Retention period of server backups (N days) for the wording about account deletion                                         | Spike S4c                                                                                                                                                                                                        |

---

## 14. Spikes

Each spike is a separate folder `spikes/Sx-*`, time-boxed, the result is a short report + an ADR. Failure criteria are written down **before** starting.

| #   | What we check               | Options                                                                                    | Success / failure criteria                                                                                                                                                                            | Limit  |
|-----|-----------------------------|--------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|--------|
| S1  | Composer with live Markdown | CodeMirror 6 (main) vs Milkdown or another                                                 | Russian keyboard layout, IME and macOS autocorrect, undo/redo, pasting an image, 5 000 characters without lag, the cursor does not jump on highlighting, the same in WKWebView / WebView2 / WebKitGTK | 3 days |
| S2  | Feed with 100k              | TanStack Virtual (`anchorTo: 'end'`, keys by id) vs virtua vs the free `Virtuoso`          | No jank on fast scrolling, jump to a date < 300 ms, no jumps when images load and when loading more upward, memory is normal                                                                          | 3 days |
| S3  | Forest                      | **Done:** Canvas 2D + cache, grid B (not met: Linux WebKit 2× no GPU, floor; ADR-0013)     | 60 FPS on pan/zoom on macOS **and Linux (including a machine without GPU-accelerated WebGL)**, correct hit-test of all days, readability of the year in a 1280 × 800 window, growth animation         | 4 days |
| S4a | Sync protocol               | A simulation of 2–3 clients in one process, without a network and a server; property tests | Criteria from §10.2                                                                                                                                                                                   | 5 days |
| S4b | Envelope and keys           | libsodium / RustCrypto; Argon2id; onboarding of the password and the recovery key          | Forging the HLC/flag/moving an envelope is detected; password change without re-encryption; recovery from 24 words                                                                                    | 4 days |
| S4c | Server and self-host        | axum + Postgres, `seq` under an account lock, blobs, `docker compose`, backup and restore  | No plain text in the server database; the cursor does not skip records with parallel pushes; restore from a backup → clients do a full resync                                                         | 5 days |
| S5  | Search in Russian           | A stemmed column + FTS5, trigrams, a custom tokenizer                                      | Word forms are found, 100k messages < 300 ms, index size is acceptable                                                                                                                                | 1 day  |
| S6  | OpenRouter embeddings       | `bge-m3`, `text-embedding-3-small` (512/1536), `qwen3-embedding-8b`; float32 vs int8       | Quality on 50 of my own queries in Russian, the cost of indexing 100k, query latency, memory on the client                                                                                            | 2 days |

S3 report: [`spikes/S3-forest/README.md`](../../spikes/S3-forest/README.md); screenshots: `docs/screens/S3/`.

Order: S3 (forest) and S5 (search) — when the slices get to them; S1 (CodeMirror) — when the plain input field starts getting in the way; S2 (feed) — when the volume after the Telegram import shows lag. **Before slice 1** — ADR-0001 "protocol + keys" on paper (a separate task in parallel with D1) (field groups, HLC, `schema_version`, id): the local schema depends on it. S4a/S4b — before slice 9, S4c — before slice 11, S6 — before AI.

---

## 15. Roadmap (vertical slices)

| Slice    | Scenario that works end to end                                                                                                                                                                                                                                                                                                             | Depends on        |
|----------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------|
| 0        | Repository on GitHub (Gitea on the homelab at first), CI on a Linux runner (tests, linters, `kinige-domain` for wasm32, visual tests), a local build and ad-hoc signing of the `.app`, Tauri + React skeleton, `KinigeApi` + `MockApi`, Storybook, tokens → CSS, a window screenshot script; ADR "signing keys"                            | Q8                |
| ADR-0001 | Sync protocol + keys (§10) — in parallel with D1; review by `sync-crypto-guardian` and `codex-second-opinion`                                                                                                                                                                                                                              | 0                 |
| D1       | Design system foundation: dark theme tokens (graphite, compact density), fonts, slice 1 components (message, day divider, composer with buttons — appearance only, toast), the main window with an "emoji + name" sidebar in Storybook; the feed variant is chosen from static concepts. The other components are in slices 2, 3, 5, 7, 7a | 0, Codex concepts |
| 1        | Opened → wrote in Inbox (a plain field, Markdown in the feed) → sent → killed the process → opened → the entry is in place; Markdown copy in a folder; UI strings (including the D1 strings) are in translation files, English and Russian, the library is chosen in the slice specification                                               | D1, ADR-0001      |
| 2        | Global hotkey → capture window with context → a new entry or append to the last one; clipboard hotkey; tray                                                                                                                                                                                                                                | 1                 |
| 3        | Chats with emoji (emoji choice, collapsed sidebar, "Pinned / Chats / Archive" sections), emoji picker in the input field, context menu and hover actions, tags, editing (revisions), delete → undo → trash, changing the date; database backup and integrity check                                                                         | 1                 |
| 4        | Telegram import → an archive of past years in the feed; FTS with Russian word forms                                                                                                                                                                                                                                                        | 3, S5             |
| 4a       | MCP server `kinige-mcp` (§9.3): the agent searches and reads entries, adds to Inbox; turning on, access per chat, modes, access log; ADR "MCP" before starting, review by `sync-crypto-guardian` (data access)                                                                                                                             | 4                 |
| 4b       | The settings window (tree of sections, sub-screens) and the `settings.json` file as an input: text, shortcuts, copy folder, AI agent access; two-way sync of the file; ADR-0010 and review by `sync-crypto-guardian` before the code                                                                                                       | 4a                |
| 5        | "On this day" in Inbox + a "N years later" reply                                                                                                                                                                                                                                                                                           | 4                 |
| —        | **Start of daily use on macOS (4 weeks), the next slices go on in parallel**                                                                                                                                                                                                                                                               | 1–5               |
| 6        | The year's forest (placeholder shapes, three levels of detail), click → feed to the day; a calendar as an alternative                                                                                                                                                                                                                      | 5, S3             |
| 7        | Attachments: photos and files ("file" button, paste, drag & drop), the feed does not jump                                                                                                                                                                                                                                                  | 3                 |
| 7a       | Voice notes: recording with the "microphone" button, playback in the feed, the file is an attachment; microphone access is requested at the first recording                                                                                                                                                                                | 7                 |
| 8        | Feed at large volumes — if the measurement after the import showed lag                                                                                                                                                                                                                                                                     | 4, S2             |
| 9        | Final forest art from Blender                                                                                                                                                                                                                                                                                                              | 6                 |
| 10a      | Windows build ahead of slice 10: the workspace compiles on Windows, MCP is off there (`unsupported`, the settings page says so), unsigned NSIS installer in the GitHub Release, Windows CI job, screenshots of the real window from a GitHub Windows runner                                                                                | 5                 |
| 10       | Build and acceptance on Windows and Linux; updater with signing                                                                                                                                                                                                                                                                            | 5                 |
| 11       | Self-hosted server (`docker compose`), account, encryption, sync of two machines; the first installation is the homelab                                                                                                                                                                                                                    | 10, S4a–c         |
| 12+      | P1: semantic search (S6), forest "undergrowth", random entry, version history, link previews, stickers and GIFs, light theme, web client                                                                                                                                                                                                   | 11                |

The design system grows together with the slices: components are added when a slice needs them and go through the same visual acceptance.

Non-functional targets (the method, the machine and the measurement script are fixed in slice 0; the acceptance of slice 5 refers to this script): sending < 100 ms until it appears in the feed, opening a chat < 300 ms, search < 300 ms over 100k, scrolling and the forest without jank, cold start < 1.5 s. Load data is generated by a script: 100 chats, 100k messages, 10k placeholder images, 5 years.

---

## 16. Export

The Markdown copy (§2.1) is the permanent readable version; the full export below is needed for migration with all the metadata.

Trash: entries from the trash get into `data/*.jsonl` with `deleted_at` (otherwise a migration loses the 30 days for restoring); into the export's readable Markdown and into the Markdown copy — no. Finally deleted (purged) entries are nowhere.


```text
kinige-export-2026-10-19/
├── manifest.json                   # format version, counters, hashes
├── data/*.jsonl                    # full data line by line: chats, messages, attachments, revisions
├── chats/<chat-slug>/YYYY-MM.md    # readable Markdown by month, with date and time
└── attachments/<sha256>.<ext>
```

Import: checking the format version and the hashes; the modes "restore" (into an empty database), "merge" (by id, a conflict is handled as in sync, the loser goes into a revision; before binding to an account the id of such a revision is random, ADR-0001 §3.4) and "as a copy" (new ids, including new `remote_blob_id` and `blob_key` for attachments — one blob belongs to one attachment); a report on what was skipped. The export is plain text, there is a warning about it.

---

## 17. Starting prompt for the agent

> You are a senior Rust/TypeScript engineer and the tech lead of the Kinige project. Read `docs/plan/02-prd.md` (the only normative document) and `docs/plan/01-review.md` in full, as well as `docs/plan/research/`. If the review or the research disagrees with the PRD, the PRD is right.
>
> Your task right now is **not to write the app**, but to:
> 1. Find contradictions and gaps in the PRD that we missed, and ask questions.
> 2. Write the ADR "sync protocol + keys" per §10 and one-page specifications for slices 0, D and 1 (§12.1a).
> 3. After the answer to Q8 — do slice 0, then the design system (slice D).
>
> The rules from §12 are mandatory. The main ones:
> - git and CI from the first commit;
> - one slice at a time;
> - "done" only with screenshots of the real window and test output;
> - the next slice only after acceptance by the user;
> - visuals only from the design system tokens;
> - facts about libraries — verify them or mark them as not verified;
> - do not create modules and documents "for the future".
>
> Remember why the previous version of Kinige failed: the stack was chosen without testing it on the hardest part, "done" was declared without looking at the real window, and the mockup and the implementation each lived their own life. Do not repeat this.
