# Batch KINIG-59 + KINIG-47: chat colour and feed speed

Budget: ~6 h of code, ~10 h to acceptance; stop and report at 20 h. Depends on: D2 (accepted 02.11.2026) and slice 7 on `main`. Branch from `main` **after** slice 7 merges: slice 7 rewrote `useComposerDraft`/`MainWindow` draft wiring (tiles), `Message.tsx`, `api.rs` (`SearchHit`, `TrashItem`), `command_names.rs` and added migration 7. Huly: KINIG-59, KINIG-47; branch `feat/chat-color-feed-speed`, PR and first commit `KINIG-59, KINIG-47: …`. Release: a patch (ADR-0012). Sources: PRD §2.1, §4.1, §4.2, §11, §12.3, §15; ADR-0001 details §A.0 (`meta` = `title`, `icon`, `color`); D2 spec ("Chat icon", "Open questions").

## Scenario
1. I open the app. Every chat without an emoji keeps exactly the letter-tile colour it had in D2; nothing recolours.
2. I right-click "Проекты" → "Colour" ▸. Seven rows: the chat's own letter tile in blue, teal, green, orange, pink, violet, grey, each with its name; the current one has a check. I pick "Grey": the sidebar tile turns grey at once, without a toast.
3. I search a word from "Проекты": the chat row and the hit rows show its grey letter tile. I delete an entry: the trash plate shows the same tile. "Move to chat" on an entry lists chats with their tiles (Inbox and emoji chats keep their emoji).
4. I create a new chat without an emoji and import a Telegram chat: both get an automatic colour at once, stable across restarts.
5. `kill -9`, launch: "Проекты" is still grey.
6. I type a long note in a chat with 2000 entries: typing stays instant, the feed does not redraw on each key. An MCP agent searching at the same time does not slow the feed or the tray "recent chats".

## Included
- **Colour rule (`kinige-domain`, single source):** `ChatColor` = `blue | teal | green | orange | pink | violet | grey` (the existing `color.tile.*` names, serde snake_case, ts-rs). `meta.color` `null` = automatic: 32-bit FNV-1a over the id bytes modulo the six hues in today's `TINTS` order (identical to D2 `tintForId` for ASCII ids, so no recolour). Inbox is always `grey` and cannot be changed. A stored value that is not one of the seven names (another client) shows as automatic and stays verbatim in `groups_raw`.
- **Backfill without writes:** no migration, no rewrite of existing chats, no new HLC. New and imported chats still write `"color": null` (creation bytes unchanged). `meta.color` is written only by an explicit pick (that is what syncs). The resolved colour is computed on read.
- **Store and IPC:** `Store::set_chat_color(id, color)` rewrites `meta` via `rewrite_group` (title, icon, unknown fields kept); same stored value → no write; Inbox → `InboxProtected`; missing → `ChatNotFound`; read-only → `ReadOnly`. No Markdown-copy change. `ChatSummary.color`, `SearchHit.chat_color`, `TrashItem.chat_color` (resolved, never null). Command `set_chat_color` for `main` only (`command_names.rs`, `build.rs`, capability, `lib.rs`); capture keeps seven commands; `settings` denied.
- **Frontend:** `toSidebar` (`mainWindowParts.ts`) and `toSidebarChat` (`chats.ts`) set `tint` from `color`; `tintOf` drops the id fallback; the FNV port moves to `MockApi` (emulation only). New token `color.tile.grey` (neutral, `text.primary` on it ≥ 5:1, ≥ 1.2:1 from background, hover and selected), Lucide `Palette` icon (ATTRIBUTION/icons README line). Chat menu: Rename, Change emoji, **Colour ▸**, Pin, — Archive; the item is shown only for non-Inbox chats displayed as a letter tile. `MenuItem` gets a leading `ChatTile` slot (replaces `emoji` in "Move to chat") and a checked state (`done` icon, `aria-checked`, `menuitemradio`). `ChatTile` in search (chat and hit rows), trash plates, "Move to chat". ru/en strings.
- **KINIG-47.1 feed re-render:** the typed text lives below `MainWindow` (a composer pane that owns it); `MainWindow`, `Feed` and plates do not re-render on a key press; `Message` in `memo`; feed actions, `onOpenReply`, `onOpenLink`, `onFeedPointerDown` and the other plate props stable (`useCallback`/`useMemo`). Draft saving per chat, flush on chat switch, reply strip and tiles behave as before. `MainWindow.tsx` stays ≤ 400 lines.
- **KINIG-47.2 MCP read connection:** `Inner` gets a second `query_only` connection `agent_reader` (no pool); `with_reader` (`agent_read.rs`), the agent settings read (`agent.rs`, `self.read(load_settings)`) and the agent search heal retry use it; same panic and poison handling. Verified on `main`: `Inner` at `lib.rs:127`, shared `reader` at `:129`, used by `read_unnoted` (`:287`) and `with_reader` (`agent_read.rs:86`).
- **KINIG-47.3 recent chats:** verified at `chats.rs:329-351`: `ORDER BY (SELECT MAX(m.created_at) … deleted_at IS NULL)` walks every entry of every chat (`message_feed` has `journal_date` between `chat_id` and `created_at`, `deleted_at` needs a row lookup). **No new index:** "last entry" becomes the last entry in feed order `(journal_date, created_at, id)` (PRD §4.1), one backward seek per chat on the existing `message_feed` (correlated `ORDER BY … DESC LIMIT 1` or a loop in Rust, chosen by `EXPLAIN QUERY PLAN`); the `EXISTS` clause folds into it.
- **Docs:** PRD v0.19 (v0.18 went to slice 7): §12.3 (`set_chat_color`, main only), history. ADR-0001 details: the `meta.color` values convention (seven names, `null` = automatic by id, unknown → automatic, kept verbatim). `seed_demo`: one chat with an explicit colour.

## Not included
- KINIG-47 rest: the Playwright measurement of opening a chat with 200/2000 entries, the virtualisation spike S2 (slice 8), the FTS rebuild measurement, the derived per-day forest table (ADR-0013), writer contention from MCP writes.
- Colour outside tiles (bubbles, feed accent, forest), in the tray menu, MCP tools, the Markdown copy, `settings.json`; an "Automatic" reset item; a custom colour; changing Inbox colour; renaming `orange/pink` to `amber/red`; a light-theme tile set; reading Telegram's own chat colour.

## What "done" looks like
1280 × 800 main window. The chat context menu as in D2 with "Colour" (palette icon) and a chevron; the submenu (two levels in one panel, Esc/← back) has seven menu rows (`layout.row.height`): a `layout.tile.size` tile with the chat's letter in that colour, the colour name, a check on the current one. Tiles in search rows, trash plates and the "Move to chat" submenu use the sidebar `ChatTile` and slot. States: long chat names in the Move submenu, emoji chat (no "Colour" item), Inbox (menu unchanged), large text, en/ru. Stories: the colour submenu (current = automatic, current = grey), search results, trash plate and Move submenu with tiles; baselines via `visual-update.yml`.

## Checks
- **Rust domain:** shared vectors (`""` → teal, `"a"` → pink, eight UUIDs taken from the TS function) give the same colour in Rust and vitest; Inbox → grey; unknown stored → automatic; `set_meta` keeps `color`, colour rewrite keeps title, icon, unknown fields.
- **Rust store:** pick → `chat.color` column and `meta` bytes; same value → bytes and HLC unchanged; Inbox, missing, read-only errors write nothing; new and imported chats `"color":null` and a resolved colour; `kill -9` after a pick keeps it. Recent chats: order by feed-order last entry, trash and archive excluded, an entry moved to an old date does not bump its chat; `EXPLAIN QUERY PLAN` uses `message_feed` and has no full scan of `message`; `import_bench synthetic 100000`: recent chats before/after in `bench.txt`. Agent connection: a test hook holds an agent read open while `list_messages` and `recent_chats` return; the existing agent panic test also asserts main reads work.
- **IPC (`src-tauri/tests/ipc_kinig59.rs`):** allowed for `main`, denied for capture and `settings`, capture still seven; garbage: `"red"`, `""`, `"#fff"`, `null`, `1`, a 10 KB string, bad and unknown ids → error, nothing written.
- **Frontend on `MockApi`:** typing 20 characters → 0 renders of `Message` and `Feed` (counted with `vi.mock` wrappers and a `Profiler`); after a send only the new plate and its series neighbour re-render; drafts, switch-during-send and attachment tests unchanged and green; menu item visibility rules; pick → tile updates; tiles in search, trash, Move.
- **Screenshots `docs/screens/chat-color/`** (`scripts/screens/chat-color.sh`, macOS runner, AX input): 01 sidebar unchanged colours; 02 colour submenu; 03 after picking grey; 04 search with tiles; 05 trash plate; 06 Move submenu; 07 en; 08 after `kill -9`.

## Touches sensitive areas
**Yes, `sync-crypto-guardian` before merge.** Synced `chat.meta` gets real `color` values (a protocol convention in ADR-0001 details); the group layout does not change, so a concurrent rename and colour pick on two devices resolve by `meta` LWW like title vs icon today. New IPC command and matrix. Store connections (second read connection). **No schema change:** no migration, no new column or index (slice 7's migration 7 stays the latest). No new crates or npm packages; one Lucide icon.

## Open questions
None.

## Decisions made by the coordinator on 04.11.2026 (the user reviews them in the morning)
1. Colour names stay the existing tokens `blue/teal/green/orange/pink/violet` (+ new `grey`), not `amber/red`; the hash and order are D2's, so no chat changes colour.
2. Automatic = computed on read when `meta.color` is `null`; only an explicit pick is stored and synced; no backfill write, no migration.
3. Automatic picks only the six hues; grey is pick-only; Inbox is fixed grey and has no "Colour" item.
4. "Colour" is a submenu after "Change emoji", shown only for chats displayed as a letter tile (an emoji hides the colour); rows preview the chat's own letter tile; no "Automatic" row.
5. Imported Telegram chats get the automatic colour; Telegram's colour is not read.
6. MCP gets one dedicated read connection, not a pool.
7. Recent chats order by the last entry in feed order instead of `MAX(created_at)`, to use the existing index and avoid a migration.
