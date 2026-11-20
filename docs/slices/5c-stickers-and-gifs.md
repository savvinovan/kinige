# KINIG-151: slice 5c, Stickers and GIFs

**Budget:** code 14 h, review and verification 42 h (verification has been about 3× the code): about 7 working days end to end. Stop and report at 2× (112 h). Order: the domain (the `sticker` group, its reader, the link and token rules, the Bot API response reader) and its reference bytes → migration 11 and the store (library, send, the export stickers) → the Bot API client and the token → Storybook stories of the panel and the sticker in the feed, and a story screenshot to the user before the window is built → the window → GIF playback → screens. **The natural cut:** if GIF playback is not started at 1× the budget, it moves to a follow-up PR and the stickers ship alone.
**Task:** Huly KINIG-151. **Branch:** `slice/5c-stickers-and-gifs`, cut from `origin/slice/5a-reactions`; the PR targets that branch until 5a (PR #78) merges, then `main`. One delivery with 5a. The PR title and first commit start with `KINIG-151: …`. **Sources:** the user's decisions of 20.11.2026; PRD §1.2, §2.3, §4.2, §4.3, §5, §6, §9.2, §9.3, §10.3, §12.2, §12.3, §15; ADR-0001 §A.0, §C.6, §C.8, §C.9; ADR-0007, ADR-0009, ADR-0014; the 7, 7b and 5a specs.

**Facts this spec rests on (read 2026-11-20; nothing else about Telegram is assumed).** Bot API 10.4: `getStickerSet(name)` returns `StickerSet` with `stickers[]`; each `Sticker` has `file_id`, `width`, `height`, `is_animated` (TGS), `is_video` (WEBM), `emoji`, `file_size`; neither flag means static WEBP or PNG. `getFile(file_id)` gives `file_path`, downloaded from `https://api.telegram.org/file/bot<token>/<file_path>`; files up to 20 MB. Every call carries the token in the URL. No method lists a user's installed packs. Rate limits: not verified. A static sticker has one side exactly 512 px and the other 512 px or less. The Desktop export holds stickers sent in chats (`media_type: "sticker"`, `sticker_emoji`, `file`, `thumbnail`, `mime_type` `image/webp` static or `video/webm` video, 512×512) and no pack name; the parser maps them to `TgMedia::Sticker { emoji }` (`telegram/raw.rs:267-270`), keeps a placeholder (`telegram/markdown.rs:113-131`) and drops the path (`telegram.rs:77-78`). Telegram grants no licence to sticker art and its bot terms forbid bulk collection. GIF catalogues: Giphy and Klipy forbid storing copies of their media; Tenor's API shut down on 2026-06-30.

## Scenario
1. Settings has a new section, Stickers: one field, "Telegram bot token", and the line "Kinige uses your own bot only to download the packs you name. The token stays on this computer." Until I save a token, Kinige makes no network request at all.
2. I import my Telegram export again. The preview now says "Stickers: 230 sent, 41 different · animated or video, skipped: 12". Afterwards, old entries that read "Sticker 😺 not imported" show the sticker itself: no bubble, the time in a small pill on its corner. An entry I edited after the import keeps its text and line and gets the sticker under it as a picture. Animated and video stickers stay lines.
3. I press the smile button in the field. The window now has three tabs: Emoji, Stickers, GIFs. Stickers shows a strip of pack icons (Recent, From Telegram) and a grid under pack headings. "From Telegram" holds the 41 stickers, the most recently sent first.
4. I click a sticker. It is sent at once as its own entry; the text I was typing stays in the field, and a pending "On this day" reply goes with the sticker. It is now first in Recent.
5. I press "+" at the end of the strip and paste `https://t.me/addstickers/TestPack`. Without a token the dialog says "Add your bot token in Settings → Stickers" with "Open Settings". I paste the token there once. The field then reads "Token saved · ends in …x7Qk", with "Remove".
6. Back in the dialog, "Add" shows "Downloading 12 of 30" with Cancel, then "Added «Test Pack»: 27 stickers. 3 animated or video stickers were skipped." Errors are one line in the dialog: "No pack with this name", "Telegram didn't accept the bot token: check it in Settings", "Couldn't reach Telegram", "Telegram asks to wait: try again in a minute", "This pack has only animated or video stickers". Cancel or an error adds nothing.
7. I right-click a pack icon → "Remove pack" → "Remove «Test Pack»? The stickers you sent stay in your entries."
8. I hover a sent sticker: the 5a bar, without "Edit". I put ❤️: the chip sits under the sticker. Trash and restore work as for any entry.
9. I drop my own `cat.gif` and send it. In the feed it plays while it is on screen and stops when I scroll away. A GIF over 8 MiB shows its first frame with a "GIF" badge and plays on click. With Reduce Motion on, every GIF stands still with the badge and plays on click. The GIFs tab shows the GIFs already in my journal; a click sends one again as a new entry (open question 1).
10. I `kill -9` the app during a pack download and start it: no half pack, no stray file; the library and sent stickers are in place.
11. The day file has `![sticker.webp](<2026-11-20/sticker.webp>)`. Search, "On this day" and the reply quote say "Sticker 😺". An AI agent over MCP sees an image attachment with `"sticker": true` and its emoji.

## Included
- **Data (`kinige-domain`).** The `sticker` group on `attachment` and its total reader (shape in "For the guardian"). **Display rule:** an entry is drawn as a sticker when its body is empty and its only live attachment reads as a sticker; otherwise that attachment is an ordinary image under the existing rules. The summary kind "Sticker" plus the emoji (ru/en strings) for search rows, "On this day", the reply quote and the forest card.
- **Library (`kinige-store`, local, not synced).** Migration 11: local tables for packs (source `telegram_export | telegram_link`, the link name, title, the order added) and their stickers (order, emoji, sha256, ext, size, width, height), files in `<data>/stickers/<id>.<ext>`, `app_setting.recent_stickers` (up to 20, a broken value is empty, like 5a's recents), and the columns `attachment.sticker` and `attachment.sticker_emoji` (the reader's output, `ALTER TABLE … ADD COLUMN` as in migration 9). The startup sweep removes library files without a row and `stickers/.tmp/` of other processes; a row without its file is hidden from the panel. The order of the strip: Recent, From Telegram, then packs in the order added; reordering is not included.
- **The pack "From Telegram".** The parser keeps the path of a sticker whose `mime_type` is `image/webp` (the thumbnail is ignored). With media on, the import collects static stickers that pass the probe into one pack, one item per sha256. Each sticker message gets its own `image` attachment, written as a sticker (`core.filename` `sticker.<ext>`). The line is removed by slice 7's rule (`hlc(body) == hlc(core)` and the text equals the import's rendering in ru or en): the attachment is added to an entry with none, and an edited entry keeps its line. TGS, WebM and failed files stay placeholders and are counted in the preview and the report. Idempotent and resumable as slice 7's media.
- **Packs by link.** `add_sticker_pack(link)` reads `https://t.me/addstickers/<name>`, `t.me/addstickers/<name>` or a bare name. The name must match `^[A-Za-z0-9_]{1,64}$` (our rule, because it goes into a URL path; Telegram's own rule is not verified). One `getStickerSet`, then for each static sticker one `getFile` and one download, one after another, one pack at a time. Adding a name already in the library replaces that pack atomically. An emoji that is not `is_emoji_sequence` becomes none; a title is cleaned by slice 7's filename rules and capped at 128 characters, otherwise "Sticker pack".
- **Every downloaded or imported file is untrusted input:** the same format probe as a dropped file (`classify`, `probe_image`), only WebP or PNG, a header that fits in 512 × 512, at most 2 MiB, checked against `file_size` before the request and counted while streaming, before any allocation. A failing file is skipped and counted. **Atomic per pack:** files go to `stickers/.tmp/<process-uuid>/<job>/`, then one writer job renames them and writes the pack and its items. A failure, a cancel or a crash leaves no pack and no file once the sweep has run.
- **The Bot API client (`src-tauri`, Rust only; the web view never makes the request, so the CSP does not change):**
  - HTTPS to the fixed host `api.telegram.org`, no redirects followed;
  - timeouts: 10 s to connect, 30 s per request, 10 min per pack;
  - caps: a `getStickerSet` body of at most 1 MiB, a `getFile` body of at most 16 KiB, at most 200 stickers in a pack (more is refused as a bad response);
  - `file_path` must match `^[A-Za-z0-9_./-]{1,256}$`, with no `..` segment and no leading `/`;
  - errors map by HTTP status only, because Telegram's descriptions are not verified: 401 is "token refused", 400 or 404 on `getStickerSet` is "no pack", 429 is "wait" (no automatic retry), and anything else, garbage JSON included, is "bad response";
  - the response JSON is parsed in `kinige-domain` into a validated structure; unknown fields are ignored;
  - an error is turned into a category and its text is never formatted, because an HTTP client's error text can carry the URL, and the URL carries the token;
  - the HTTP client's own log stays at `warn` even under `KINIGE_LOG=debug`.
  - The base URL can be changed only behind the `test-hooks` feature, for the fake Bot API server of the tests and screens.
- **The token.** It is read only when a pack is downloaded, never at startup. The settings window writes it (`set_telegram_bot_token`, `clear_telegram_bot_token`) and reads only `{saved, last4}` back. It must match `^[0-9]{1,20}:[A-Za-z0-9_-]{1,100}$` (our rule, which keeps the URL path safe; Telegram's format is not verified). Where it lives: "For the guardian".
- **Sending.** `send_sticker(chat_id, sticker_id, reply_to_id?)` copies the library file through `attach_bytes`' pipeline and writes the entry in one writer transaction: an empty body, one `image` attachment with the sticker group, the index (the stem `sticker`), `mirror_dirty` and the recents. The text draft and the pending tiles are untouched, and the pending reply is used and cleared, as by `send_voice`. `send_gif_again(chat_id, attachment_id, reply_to_id?)` does the same for a live, uncovered GIF of the journal (open question 1).
- **IPC (§12.3).** `main` gets the library read (with `has_token: bool`), `add_sticker_pack`, `cancel_sticker_pack`, `remove_sticker_pack`, `send_sticker`, `list_journal_gifs`, `send_gif_again` and a progress event (counts only). `settings` gets only the three token commands. `capture` keeps exactly its ten commands. MCP gets no tool. The protocol gets `kinige-media://localhost/sticker/<uuid>` for `main` only: library files, WebP or PNG, at most 2 MiB, with slice 7's headers.
- **The panel (through `design-system`, tokens only).** The emoji window (`EmojiPopover`, ADR-0007) gets tabs Emoji · Stickers · GIFs in the main window only. Stickers: the pack strip with "+", the grid with pack headings, a sticker is a button labelled "Sticker 😺", arrows and Enter as in the emoji grid. GIFs: the journal's GIFs, newest first, one per sha256, without trashed or hidden (7b) ones, still frames, plus "Choose a GIF file…" (the paperclip's picker). The add-pack dialog and the remove question use the shared `Modal`. Empty states: "No stickers yet: import your Telegram export or add a pack by its link"; "No GIFs yet: drop, paste or attach a GIF".
- **A sticker in the feed.** The bare frameless picture bubble of KINIG-132 (no fill), the box fitted to a token size from `width`/`height` so nothing jumps, the time pill on its corner; 5a's chips go under it as for a frameless picture. A reply sticker has its quote above it in its own fill. Not offered for a sticker: "Edit", "Details…", the spoiler and the viewer (a click does nothing). The trash shows it read-only.
- **GIF playback (main window only, the feed and the trash).** A GIF is an `image` whose detected format is GIF; storage, thumbnails and the protocol are unchanged. A cell shows the thumbnail (the first frame). One shared `IntersectionObserver` switches its `src` to `full` when at least half of it is visible, the window is visible and it is ≤ 8 MiB, and back to the thumbnail when it leaves; the box is fixed, so the swap moves nothing.
  - 8 MiB is a guess at a screen of playing GIFs that stays within normal memory (WebKit keeps decoded frames, and the file size is the only bound we have without parsing frames). It is measured, not verified.
  - 8–64 MiB: the badge, and a click plays it until it leaves the screen.
  - Over 64 MiB: ADR-0014 serves no `full` (the responder holds the whole file in memory), so the GIF never plays in the app: the badge, and a click opens the viewer with "Open in default app".
  - Reduce Motion (followed live): every GIF is still with the badge, and a click plays that one until it leaves the screen.
  - A playing GIF's click opens the viewer, where it plays. A covered GIF (7b) loads nothing until it is revealed.
- **Elsewhere, by the image rules.** Markdown copy: `![sticker.webp](<day/sticker.webp>)` with slice 7's clash names. Search: the stem `sticker` (no transliteration: «стикер» does not find it). MCP (ADR-0009): the image attachment plus `"sticker": true` and `"emoji"` (absent when not a sticker or no emoji). The forest's photo count leaves stickers out; the entry still counts as a note.
- **Documents.**
  - PRD v0.30:
    - §2.3: stickers by the two sources, static only; GIFs as your own files. Catalogue search waits for a provider whose terms allow keeping a copy in the journal, Klipy only after its written approval, and is then turned on only by an explicit setting. Tenor goes;
    - §4.2, §4.3, §5 (the tabs), §6, §9.1, §9.3;
    - §10.3 (what the server and Telegram learn);
    - §12.3 (the commands, the first outbound request, the token);
    - §15 (row 5c; row 12+ loses stickers and own GIFs);
    - the history line.
  - ADR-0001 revision 14 (§A.0, a new §C.10); ADR-0009; ADR-0014 (the `sticker` path, GIF playback and the 64 MiB rule); new ADR-0021 (the Bot API client, its crate, limits and token storage); `CLAUDE.md` status in the first commit.

## Not included
- Animated (TGS) and video (WebM) stickers; custom emoji; sticker or GIF reactions (5a's list).
- A Telegram account login, and listing your installed packs: no Bot API method does it.
- Syncing the pack library; sticker search; favourite stickers; reordering packs; a pack preview from a sent sticker.
- **GIF catalogue search.** Both live providers, Giphy and Klipy, forbid storing copies of their media, and a journal keeps everything it shows; Tenor's API shut down on 2026-06-30. It waits for Klipy's written approval.
- Stickers and GIFs in the quick capture window. Making a sticker from your own picture (PRD §2.3 lists it; the user's decisions name two sources: later).
- Telegram "animation" messages stay placeholders. Autoplay of animated WebP or APNG in the feed. Proxy settings for Telegram. Sending a sticker with text in the same entry.

## Reference (UI work)
**Telegram desktop.** The smile button opens one panel with Emoji, Stickers and GIFs; stickers sit under pack headings with a strip of pack icons and Recent first; a click sends the sticker at once as its own message; in the chat a sticker has no bubble, the time in a small pill at its corner and reactions under it; GIFs autoplay in the chat, and with autoplay off they show a "GIF" label and play on click (sizes not verified).
- **Copied:** one panel with three tabs, the pack strip, click-to-send, no bubble, the time pill, autoplay while visible, the badge.
- **Changed:** static stickers only; no sticker or GIF search; the GIFs tab holds your own GIFs; Reduce Motion and the 8 MiB rule govern autoplay instead of a setting; no pack preview; no "Edit" on a sticker.

## Decisions left to the implementer
- **Ruled by the guardian before code:** everything under "For the guardian".
- **Listed in the PR:**
  - the command names, the component names, the token sizes (recommended: the sticker's long side about 160 px), the badge;
  - the HTTP client crate, a new dependency through ADR-0021. Recommended: `reqwest` with rustls and default features off; it is already in `Cargo.lock` through `server/`, so `cargo deny` has seen it;
  - the keychain crate (`keyring` is the candidate; version, licence and behaviour not verified);
  - where the Bot API reader lives (recommended: `kinige-domain`, pure, so the garbage tests run without a network).

## What "done" looks like
A 1280 × 800 window, the feed column ~720 px.
- **The emoji window** (the existing popover, its width unchanged), tabs at the top. The Stickers tab: a 32 px pack strip with "+", a grid of 5 columns of stickers, pack headings in the secondary text. Empty states as above. The GIFs tab: a 3-column grid of still frames with the badge.
- **The add-pack dialog:** a `TextField` for the link, Add and Cancel, the progress line with Cancel, the one-line result or error.
- **Settings → Stickers:** a password-style `TextField`, Save, then "Token saved · ends in …x7Qk" with Remove, and the two notes: what Telegram learns (the bot, the names you add, your IP address), and that macOS may ask once after an update for access to the keychain (not verified).
- **The feed:** a bare sticker with its pill, one with chips, one as a reply, one in an edited imported entry (a picture in the bubble); a playing GIF; a large GIF with the badge; an album with one GIF.
- **States:** Russian and English; large text; a 640 px window; a 200-sticker pack; a pack title of 128 characters cut with an ellipsis.
- **Storybook** (en and ru, normal and large text): `StickerPanel`, `GifPanel`, `AddPackDialog` (each state), the token row, the sticker bubble, `GifCell` playing, still, with the badge and under Reduce Motion.
- **Screenshots in `docs/screens/5c/`:** `scripts/screens/5c.sh`, `DRIVER=1`, the macOS runner. `seed_demo` with `KINIGE_SEED_STICKERS=1` draws synthetic stickers and GIFs with the `image` crate: no real sticker or GIF in the repo, tests, Storybook or shots (rules 9, 10). A synthetic export, and a fake Bot API server on the runner's localhost.
  - Shots: 01 the Stickers tab; 02 the dialog without a token; 03 Settings → Stickers; 04 the progress; 05 the result with skips; 06 the feed with stickers; 07 the re-imported entries; 08 the import preview; 09 the GIFs tab; 10 the feed with a playing GIF and a badged one; 11 after `kill -9`; 12 search and the trash; 13 en; 14 large text; 15 640 px.
  - Texts: `mirror.txt`, `mcp.txt`, `requests.txt` (the fake server's request count before and after "Add", with the token shown as `<token>`), `gif-memory.txt`, `cold-start.txt`.

## Checks
| Scenario step / rule             | Test or evidence that proves it                                                       |
|----------------------------------|---------------------------------------------------------------------------------------|
| 1. No request without a token    | fake Bot API server: no request at startup, on opening the panel or without a token   |
| 2. Export stickers, placeholders | store, synthetic export: one item per sha256, TGS and WebM counted, edited entry kept |
| 2. Re-import is idempotent       | store: a second run attaches nothing twice and adds no item; stop and kill -9 mid-run |
| 3-4. Panel, send, recents        | vitest on MockApi: tabs, grid keys, a send keeps the draft; store: one transaction    |
| 5-6. Link, token, download       | IPC tests on the fake server: link and token vectors, progress, cancel; shots 04-06   |
| 6. Garbage responses             | fake server (rule 12): not JSON, 1 GiB body, lying length, stall, redirect, bad path  |
| 6. One pack, all or nothing      | store: a fault after 10 of 30 files, and kill -9 mid-download: no pack, no stray file |
| 7. Remove a pack                 | store: rows and files gone, sent stickers byte-identical; vitest of the question      |
| 8. A sticker in the feed         | domain: the display rule; Storybook: bare, reactions, quote, trash; shot 07           |
| 9. GIF playback                  | Playwright visual: src swaps with visibility, Reduce Motion stays still, box equal    |
| 9. GIF memory                    | peak memory of the web content process with six 8 MiB GIFs playing: gif-memory.txt    |
| 10. Survives kill -9             | screens: the library, a sent sticker and a GIF after kill -9; shot 11                 |
| 11. Copy, search, MCP            | mirror.txt, mcp.txt; store: the sticker summary, the agent's sticker and emoji        |
| The sticker group                | domain: reference bytes, the read rules, arbitrary JSON, older-client vector          |
| The token never leaks            | log_privacy at debug on the fake server: no token, link, pack name or title           |
| Where the token lives            | not in the database, a backup, settings.json, the copy, MCP or any view (a test)      |
| IPC matrix                       | ipc_slice5c.rs: library and network main only, token settings only, capture ten       |
| CSP unchanged                    | config test: connect-src and img-src as on main                                       |
| Migration 11                     | pinned hash after 5a's; rows of version 10 unchanged; fresh database                  |
| Accessibility                    | vitest: labels, grid arrows, Enter, Escape; shot 14 (large text)                      |
| The branch                       | scripts/pre-pr-check.py; CI green on every job                                        |

The garbage set also has a 100k × 100k PNG header (refused before decoding), a JPEG named `.webp`, `ok:false`, 401, 429, a TLS failure, a 201st sticker and a `file_path` with `..` or a full URL. The link and token vectors live in `fixtures/api-contract/`, run by Rust and by MockApi.

## For the guardian
- **The group.** `sticker` = `{"emoji":<string|null>,"sticker":<bool>}`, `sv = 1`, canonical JCS, a public contract in §C.10. Reference bytes: `{"emoji":"😺","sticker":true}`. **It is written on every new attachment** (send, voice, the media import, a sticker), under the creation HLC like 7b's three groups, as `{"emoji":null,"sticker":false}` when the attachment is not a sticker. So the open header does not tell a sticker from a photo at creation. It is additive (§4 rule 1): `schema_version` stays 1 and `core` is not touched.
- **It is never rewritten or added later, so it is not on the add-group allowlist.** Nothing in this slice adds it to an existing row; a re-imported sticker is a new attachment. Keeping it off means no code path can turn an existing photo into a sticker. This differs from the coordinator's note "reuse 7b's add-group"; if the guardian prefers it on the list for symmetry, it goes on with no caller. A row written before 5c has no group and is not a sticker; there is no backfill.
- **The total reader** never errors. It reads "sticker" only when the group is well-formed with `"sticker":true`, at most 1 KiB, the supported `sv`, both maps, on `kind = image` with a detected WebP or PNG. Anything else, including absent, `false`, mistyped, an unknown `sv` or another kind, reads "not a sticker", and the bytes are kept verbatim. The failure shows an ordinary picture and hides nothing. An `emoji` that is not `is_emoji_sequence` (at most 40 bytes) never reaches the view, the copy, MCP or the log.
- **An older client** keeps the group raw and resends it (§4 rule 1). It shows a sticker as an ordinary picture in a bubble. No released client syncs before slice 11.
- **What the server learns:** nothing new from the header. Through the padded blob size, it can guess that a small image is a sticker.
- **Migration 11:** local tables, `app_setting.recent_stickers`, and two read-output columns on `attachment`. No row of a synced table changes. Its hash is appended to `PINNED` after 5a's.
- **The library is device-local:** not synced, not in `settings.json`. A sent sticker travels as its attachment only.
- **The placeholder rewrite** of §C.6 (no `edit` revision) now also covers the sticker line.
- **The token is stored in the OS secret store** (Keychain, Credential Manager, Secret Service), as PRD §9.2 rules for the OpenRouter key.
  - **Not in `app_setting`:** the database is not encrypted, and it is copied into the 7 daily backups, so a removed token would live on in them.
  - If the keychain crate fails the dependency check, or Linux has no Secret Service, the token goes to a 0600 file outside the database with a warning (PRD §12.3's Linux rule). Never to the database.
  - It is never in the log, the Markdown copy, `settings.json`, MCP, the capture window or the main window; the settings window gets `{saved, last4}` only. Under `test-hooks` an in-memory store replaces it.
- **The network:**
  - one main-window command, the fixed host, the token only in the paths of requests to that host;
  - the caps and timeouts above;
  - log lines carry event kinds and categories, never the link, the pack name, the title or the token.
  - **Telegram learns** the bot token (so the bot), the names of the packs you add, your IP address and when. It never learns anything of the journal.

## Touches sensitive areas
**Yes.** Data schema: migration 11. Sync: a new synced group on every new attachment, its reader, and §C.6's exception extended. Server storage: the group's name in the header, unchanged by stickers. IPC: new main and settings commands and a protocol path. A secret in the OS store, and the app's first outbound request. MCP: two new read fields. `sync-crypto-guardian` reviews this spec before code and the PR before merge.

## Open questions
1. **What the GIFs tab holds.** Without GIF search the tab can show (a) the GIFs already in your journal, so a click sends one again as a new entry, plus "Choose a GIF file…", or (b) only the "Choose a GIF file…" button, the same as the paperclip. Recommended: (a), which works like Telegram's saved GIFs. The spec is written for (a); with (b) the list and `send_gif_again` go.
   **Decided by the coordinator on 20.11.2026 (reversible, the user is told):** (a).
