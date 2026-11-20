# KINIG-151: slice 5c, Stickers and GIFs

**Budget:** code 16 h, review and verification 48 h (verification has been about 3× the code): about 8 working days end to end. Stop and report at 2× (128 h). Re-estimated after the guardian's review: the token's failure paths, the keychain check across builds and the writer and legacy-row tests are in. Order:
1. the domain (the `sticker` group, its reader, the link, token and `file_id` rules, the Bot API response reader) and its reference bytes;
2. migration 11 and the store (library, send, the export stickers);
3. the keychain check across two builds, before any UI;
4. the Bot API client;
5. Storybook stories, and a story screenshot to the user;
6. the window, GIF playback, the screens.

**The natural cut:** if GIF playback has not started at 1× the budget, it moves to a follow-up PR and the stickers ship alone.
**Task:** Huly KINIG-151. **Branch:** `slice/5c-stickers-and-gifs`, cut from `origin/slice/5a-reactions`; the PR targets that branch until 5a (PR #78) merges, then `main`. One delivery with 5a. The PR title and first commit start with `KINIG-151: …`. **Sources:** the user's decisions of 20.11.2026; the `sync-crypto-guardian` review of this spec (folded in); PRD §1.2, §2.3, §4.2, §4.3, §5, §6, §9.2, §9.3, §10.3, §12.2, §12.3, §15; ADR-0001 §A.0, §C.6, §C.8, §C.9; ADR-0007, ADR-0009, ADR-0014; the 7, 7b and 5a specs.

**Facts this spec rests on (read 2026-11-20; nothing else about Telegram is assumed):**
- **Bot API 10.4.** `getStickerSet(name)` returns `StickerSet` with `stickers[]`. Each `Sticker` has `file_id`, `width`, `height`, `is_animated` (TGS), `is_video` (WEBM), `emoji` and `file_size`; neither flag means a static WEBP or PNG. `getFile(file_id)` gives `file_path`, downloaded from `https://api.telegram.org/file/bot<token>/<file_path>`; files are up to 20 MB. Every call carries the token in the URL. No method lists a user's installed packs. Rate limits: not verified.
- **A static sticker** has one side exactly 512 px and the other 512 px or less.
- **The Desktop export** holds the stickers sent in chats (`media_type: "sticker"`, `sticker_emoji`, `file`, `thumbnail`, and `mime_type` `image/webp` for static or `video/webm` for video, 512×512) and no pack name. The parser maps them to `TgMedia::Sticker { emoji }` (`telegram/raw.rs:267-270`), keeps a placeholder (`telegram/markdown.rs:113-131`) and drops the path (`telegram.rs:77-78`).
- **Rights.** Telegram grants no licence to sticker art, and its bot terms forbid bulk collection.
- **GIF catalogues.** Giphy and Klipy forbid storing copies of their media; Tenor's API shut down on 2026-06-30.
- **Crates in `Cargo.lock`:** `reqwest` 0.13.5, and `security-framework` 3.7.0 (it has the generic-password functions).

## Scenario
1. Settings has a new section, Stickers. It has one field, "Telegram bot token", and the line "Kinige uses your own bot only to download the packs you name. The token is stored on this computer and sent only to Telegram." Until I save a token, Kinige makes no network request at all.
2. I import my Telegram export again. The preview now says "Stickers: 230 sent, 41 different · animated or video, skipped: 12". Afterwards, old entries that read "Sticker 😺 not imported" show the sticker itself: no bubble, the time in a small pill on its corner. An entry I edited after the import keeps its text and line, and gets the sticker under it as a picture.
3. I press the smile button in the field. The window has three tabs: Emoji, Stickers, GIFs. Stickers shows a strip of pack icons (Recent, From Telegram) and a grid under pack headings, the most recently sent first.
4. I click a sticker. It is sent at once as its own entry; the text I was typing stays in the field, and a pending "On this day" reply goes with the sticker.
5. I press "+" at the end of the strip and paste `https://t.me/addstickers/TestPack`. Without a token the dialog says "Add your bot token in Settings → Stickers", with "Open Settings". I paste the token there once; the field then reads "Token saved", with "Remove".
6. In the dialog, "Add" shows "Downloading 12 of 30" with Cancel, then "Added «Test Pack»: 27 stickers. 3 animated or video stickers were skipped." Errors are one line in the dialog: "No pack with this name", "Telegram didn't accept the bot token: check it in Settings", "Couldn't reach Telegram", "Telegram asks to wait: try again in a minute", "This pack has only animated or video stickers", "The pack is too large", "A pack is already downloading". Cancel or an error adds nothing.
7. I right-click a pack icon → "Remove pack" → "Remove «Test Pack»? The stickers you sent stay in your entries."
8. I hover a sent sticker: the 5a bar, without "Edit". I put ❤️, and the chip sits under the sticker. Trash and restore work as for any entry.
9. I drop my own `cat.gif` and send it. In the feed it plays while it is on screen and stops when I scroll away. A GIF over 8 MiB or 2 megapixels shows its first frame with a "GIF" badge and plays on click. With Reduce Motion on, every GIF stands still with the badge and plays on click. The GIFs tab shows the GIFs already in my journal; a click sends one again as a new entry.
10. I `kill -9` the app during a pack download and start it: no half pack, no stray file; the library and the sent stickers are in place.
11. The day file has `![sticker.webp](<2026-11-20/sticker.webp>)`. Search, "On this day" and the reply quote say "Sticker 😺". An AI agent over MCP sees an image attachment with `"sticker": true` and its emoji.

## Included
- **Display rule (`kinige-domain`).** An entry is drawn as a sticker when three things hold:
  - its body is empty;
  - its only live attachment reads as a sticker ("For the guardian");
  - that attachment is not covered (7b). A covered one is drawn as 7b's cover.

  Otherwise the attachment is an ordinary image under the existing rules. The summary for search rows, "On this day", the reply quote and the forest card is "Sticker" plus the emoji (ru/en).
- **Library (`kinige-store`, local, not synced), migration 11:**
  - local tables for packs (source `telegram_export | telegram_link`, the link name, the title, the order added) and their stickers (order, emoji, sha256, ext, size, width, height);
  - the files in `<data>/stickers/<id>.<ext>`;
  - `app_setting.recent_stickers` (up to 20; a broken value is empty);
  - `attachment.sticker` and `attachment.sticker_emoji`, which are derived columns.

  The strip shows Recent, From Telegram, then packs in the order added. A row without its file is hidden from the panel.
- **The pack "From Telegram".** The parser keeps the path of a sticker whose `mime_type` is `image/webp`. With media on, the import puts the static stickers that pass the probe into one pack, one item per sha256. Each sticker message gets its own attachment (`core.filename` `sticker.<ext>`). Slice 7's rule decides the placeholder line: the attachment goes to an entry with none, and the line is removed only when the entry is unedited since import. TGS, WebM, animated and failed files stay placeholders and are counted. Idempotent and resumable as slice 7's media.
- **Packs by link.** `add_sticker_pack(link)` takes `https://t.me/addstickers/<name>`, `t.me/addstickers/<name>` or a bare name.
  - **Our input rules** (Telegram's own are not verified): a name matches `^[A-Za-z0-9_]{1,64}$`, and a `file_id` matches `^[A-Za-z0-9_-]{1,256}$`.
  - **The calls:** one `getStickerSet`, then one `getFile` and one download per static sticker, one after another.
  - **One pack job at a time.** A second `add_sticker_pack` is refused as busy, and cancel is idempotent.
  - **Caps:** 200 stickers per pack, 2 MiB per file and 64 MiB per pack in total. Over a cap is "The pack is too large", and nothing is added.
  - **A name already in the library** replaces that pack.
  - **Cleaning:** an emoji that is not `is_emoji_sequence` becomes none. A title gets slice 7's filename rules and is capped at 128 characters; otherwise it reads "Sticker pack".
- **Every downloaded or imported file is untrusted input.** It gets the same probe as a dropped file (`classify`, `probe_image`), plus three rules:
  - WebP or PNG only, with animation flags refused (animated WebP and APNG count as skipped);
  - a header that fits in 512 × 512;
  - at most 2 MiB, checked against `file_size` before the request and counted while streaming, before any allocation.

  A failing file is skipped and counted.
- **A pack is written all or nothing.**
  - **The download:** files go to `stickers/.tmp/<process-uuid>/<job>/`.
  - **The write:** one writer job renames the files into `stickers/` first, then writes the rows in one transaction, last. A replaced pack's old files are deleted after the commit.
  - **The sweep** is a writer job after the first frame, like slice 7's `attachment/sweep.rs`, so it cannot interleave with a pack write. It removes `stickers/.tmp/` of other processes and library files without a row, and never touches `attachments/`.
- **The Bot API client (`src-tauri`, Rust only).** The web view never makes the request, so the CSP does not change.
  - **Requests:** `reqwest` 0.13.5 to the fixed host `api.telegram.org`. Every URL is built with a URL builder (path segments, query pairs), never by concatenation. No redirect is followed: a 3xx is a bad response. No `Accept-Encoding` is sent; `Content-Encoding: identity` is tolerated, and any other value is refused. The User-Agent is `kinige/<version>` and nothing else identifying (CLAUDE.md rule 14 covers development requests; ADR-0021 states this one).
  - **Trust anchors** are the OS store. A root CA installed on the machine can therefore read the token; PRD §10.3 states this.
  - **The system proxy is honoured.** The user's network reaches Telegram only through a proxy or a VPN. An HTTP proxy tunnels HTTPS by CONNECT, so the token stays inside TLS.
    - This needs reqwest's `system-proxy` feature (`hyper-util/client-proxy-system`), which reads the macOS and Windows system settings. With default features off, only environment variables are read, and an app started from Finder has none.
    - Expected to work: HTTP and HTTPS proxies from the system settings. SOCKS: not verified; the implementer checks it. A system VPN needs nothing.
  - **The TLS provider's Windows build** is checked first. If it fails, the fallback is `rustls-no-provider` with `ring`.
  - **Timeouts:** 10 s to connect, 30 s per request, 10 min per pack.
  - **Response caps:** 1 MiB for `getStickerSet` and 16 KiB for `getFile`. A `file_path` must match `^[A-Za-z0-9_./-]{1,256}$`, with no `..` segment and no leading `/`. The JSON is read in `kinige-domain` into a validated structure; unknown fields are ignored.
  - **Errors map by HTTP status** (Telegram's descriptions are not verified): 401 is "token refused", 400 or 404 on `getStickerSet` is "no pack", 429 is "wait" (no retry), and anything else is "bad response". The first step of the mapping is `without_url()`. After that only a category remains, and no error text is formatted. The HTTP client's own log stays at `warn` even under `KINIGE_LOG=debug`.
  - **Test-only switches:** the base-URL override and the in-memory token store exist only under the compile-time `test-hooks` feature, and `scripts/check-test-hooks.sh` is extended to catch them. A test token passes our loose rule but not the real Telegram shape, because gitleaks flags the real shape.
- **The token.** It is read only when a pack is downloaded: never at startup, and never on opening the panel or Settings.
  - It must match `^[0-9]{1,20}:[A-Za-z0-9_-]{1,100}$` (our rule; Telegram's format is not verified).
  - The settings window writes it (`set_telegram_bot_token`, `clear_telegram_bot_token`) and reads back only `{saved}`. `clear_telegram_bot_token` always deletes the keychain item, whatever the flag says.
  - `saved` is a plain flag in `app_setting`. It can disagree with the keychain, for example after a database restore; the next read corrects it. Where the token lives: "For the guardian".
- **Sending.** `send_sticker(chat_id, sticker_id, reply_to_id?)` re-probes the library file and copies it through `attach_bytes`' pipeline. One writer transaction then writes the entry: an empty body, the attachment with `"sticker":true`, the index (the stem `sticker`), `mirror_dirty` and the recents. A file that fails the probe refuses the command: it is never sent as a photo. The text draft and the pending tiles are untouched, and the pending reply is used and cleared, as by `send_voice`.
- **Sending a GIF again.** `send_gif_again(chat_id, attachment_id, reply_to_id?)` does the same for a live GIF of the journal. The copy gets a fresh `remote_blob_id` and `blob_key`, and 7b's details are not copied. The store refuses a covered source.
- **A sticker entry** (an entry drawn as a sticker, so never a covered one) is never an append target (`can_append`). A covered one is excluded so that a cover which came from another device can still be removed with `set_attachment_details` and `spoiler: false`. The store itself refuses `edit_message` and `set_attachment_details` on it, and the window hides "Edit", "Details…" and the spoiler.
- **IPC (§12.3).**
  - `main` gets the library read (with `has_token: bool`), `add_sticker_pack`, `cancel_sticker_pack`, `remove_sticker_pack`, `send_sticker`, `list_journal_gifs`, `send_gif_again` and a progress event (counts only).
  - `settings` gets only the token commands.
  - `capture` keeps exactly its ten commands. MCP gets no tool.
  - The protocol gets `kinige-media://localhost/sticker/<uuid>`, for `main` only: library files, WebP or PNG, at most 2 MiB, with slice 7's headers.
- **The panel (through `design-system`, tokens only).** The emoji window (`EmojiPopover`, ADR-0007) gets tabs Emoji · Stickers · GIFs, in the main window only.
  - Stickers: the pack strip with "+", and a grid under pack headings. Each sticker is a button labelled "Sticker 😺", with arrows and Enter as in the emoji grid.
  - GIFs: the journal's GIFs, newest first, one per sha256, as still frames, without trashed or covered ones, plus "Choose a GIF file…".
  - The add-pack dialog and the remove question use the shared `Modal`.
  - Empty states: "No stickers yet: import your Telegram export or add a pack by its link"; "No GIFs yet: drop, paste or attach a GIF".
- **A sticker in the feed.** It uses KINIG-132's bare frameless picture bubble, with its box fitted to a token size from `core.width` and `core.height`, so nothing jumps. The time pill sits on its corner, and 5a's chips go under it. A reply sticker has its quote above it, in its own fill. A click does nothing (there is no viewer).
- **GIF playback (main window, the feed and the trash).** A GIF is an `image` with `core.mime = image/gif`; storage, thumbnails and the protocol are unchanged. A cell shows the thumbnail, the first frame. One shared `IntersectionObserver` sets its `src` to `full` while at least half of the cell is visible and the window is visible, and back to the thumbnail when it leaves. The box is fixed, so the swap moves nothing.
  - **Autoplay** needs `byte_size` ≤ 8 MiB and `width × height` ≤ 2,000,000 pixels. Both figures are guesses at a screen of playing GIFs that stays within normal memory, and the slice measures them.
  - **A larger GIF up to 64 MiB:** the badge, and a click plays it until it leaves the screen.
  - **Over 64 MiB:** ADR-0014 serves no `full`, so the GIF never plays in the app; a click opens the viewer with "Open in default app".
  - **Reduce Motion** (followed live): every GIF stands still with the badge, and a click plays that one.
  - A playing GIF's click opens the viewer. A covered GIF loads nothing until it is revealed.
- **Elsewhere, by the image rules.**
  - The Markdown copy writes `![sticker.webp](<day/sticker.webp>)`.
  - Search uses the stem `sticker`; «стикер» does not find it.
  - MCP (ADR-0009) shows the attachment plus `"sticker": true` and `"emoji"` (absent otherwise).
  - The forest's photo count leaves stickers out.
- **Documents.**
  - **PRD v0.30:**
    - §2.3: two sticker sources, static only, and GIFs as your own files. Catalogue search waits for a provider whose terms allow keeping a copy, Klipy only after its written approval, behind an explicit setting. Tenor goes.
    - §4.2, §4.3, §5, §6, §9.1, §9.3.
    - §10.3: what the server and Telegram learn, and the OS trust anchors.
    - §12.3, §15 (a 5c row; row 12+ loses these), and the history line.
  - **ADRs:** ADR-0001 revision 14 (§A.0, a new §C.10); ADR-0009; ADR-0014 (the `sticker` path, GIF playback); a new ADR-0021 (the client, the TLS provider, the proxy, the User-Agent, the limits, the token storage).
  - **`CLAUDE.md`** status, in the first commit.

## Not included
- Animated (TGS) and video (WebM) stickers; animated WebP and APNG; custom emoji; sticker or GIF reactions.
- A Telegram login, and listing your installed packs: no Bot API method does that.
- Syncing the library; sticker search; favourites; reordering packs; a pack preview; a proxy setting inside Kinige (the system proxy is used).
- **GIF catalogue search.** Giphy and Klipy forbid storing copies of their media, and a journal keeps what it shows. Tenor's API shut down on 2026-06-30. Search waits for Klipy's written approval.
- A file fallback for the token on Linux without Secret Service: it comes with the Linux build (slice 10). Until then the token cannot be saved there.
- Stickers and GIFs in the quick capture window. Stickers made from your own picture: PRD §2.3 lists them, but the user's decisions name two sources, so they come later. Telegram "animation" messages stay placeholders.

## Reference (UI work)
**Telegram desktop.** One panel holds Emoji, Stickers and GIFs. Stickers sit under pack headings, with a strip of pack icons and Recent first. A click sends the sticker at once as its own message. A sticker has no bubble: the time is in a small pill at its corner, and reactions go under it. GIFs autoplay, and with autoplay off they show a "GIF" label and play on click (sizes not verified).
- **Copied:** the three tabs, the pack strip, click-to-send, no bubble, the time pill, autoplay while visible, the badge.
- **Changed:** static stickers only; no search; the GIFs tab holds your own GIFs; Reduce Motion and the size rules govern autoplay instead of a setting; no pack preview; no "Edit" on a sticker.

## Decisions left to the implementer
Listed in the PR:
- the command names, the component names and the error names;
- the token sizes (recommended: the sticker's long side about 160 px) and the badge;
- the secret-store crate for Windows and Linux (`keyring` is not verified; macOS may need only `security-framework`);
- the TLS provider, after its Windows build is checked.

Everything under "For the guardian" is ruled.

## What "done" looks like
A 1280 × 800 window.
- **The emoji window** keeps its width, with tabs at the top. The Stickers tab has a 32 px pack strip with "+", a grid of 5 columns and pack headings in the secondary text. The GIFs tab is a 3-column grid of still frames with the badge.
- **The add-pack dialog:** a `TextField`, Add and Cancel, the progress line, and the one-line result or error.
- **Settings → Stickers:** a password-style `TextField` and Save, then "Token saved" with Remove. Two notes:
  - what Telegram learns;
  - that macOS may ask once after an update for keychain access (not verified).
- **The feed:**
  - a bare sticker; one with chips; one as a reply;
  - one in an edited imported entry, as a picture in the bubble;
  - a covered sticker, drawn as the cover;
  - a playing GIF, a badged GIF, and an album with a GIF.
- **States:** ru and en, large text, a 640 px window, a 200-sticker pack, a 128-character title cut with an ellipsis.
- **Storybook** (en and ru, normal and large text): `StickerPanel`, `GifPanel`, `AddPackDialog` (every state), the token row, the sticker bubble, and `GifCell` (playing, still, badged, Reduce Motion).
- **Screenshots in `docs/screens/5c/`:** `scripts/screens/5c.sh`, `DRIVER=1`, on the macOS runner. `seed_demo` with `KINIGE_SEED_STICKERS=1` draws synthetic stickers and GIFs with the `image` crate. No real sticker or GIF goes into the repo, the tests, Storybook or the shots (rules 9 and 10). A synthetic export, and a fake Bot API server on the runner's localhost.
  - Shots: 01 the Stickers tab; 02 the dialog without a token; 03 Settings; 04 the progress; 05 the result with skips; 06 the feed with stickers; 07 the re-imported entries; 08 the import preview; 09 the GIFs tab; 10 the GIFs in the feed; 11 after `kill -9`; 12 search and the trash; 13 en; 14 large text; 15 640 px.
  - Texts:
    - `mirror.txt`, `mcp.txt`;
    - `requests.txt`: the fake server's requests and their headers, with the token as `<token>`;
    - `gif-memory.txt`, `cold-start.txt`.

## Checks
| Scenario step / rule            | Test or evidence that proves it                                                                      |
|---------------------------------|------------------------------------------------------------------------------------------------------|
| 1. No request, no keychain read | fake server, counting secret store: nothing at startup, on opening the panel or Settings, no token   |
| 2. Export stickers              | store: one item per sha256; TGS, WebM, animated WebP, APNG counted; edited entry kept; idempotent    |
| 3-4. Panel, send, recents       | vitest on MockApi: tabs, grid keys, draft kept; store: one transaction; a failing re-probe refuses   |
| 5-6. Link, token, download      | fake server: link, token and file_id vectors, progress, a second job busy, cancel twice; shots 04-06 |
| 6. Network garbage              | not JSON, gzip body, 1 GiB, lying length, stall, 401, 429, TLS failure, a 201st sticker, over 64 MiB |
| 6. Network garbage, URLs        | file_id with & # %2e%2e, a file_id twice, Location to another host and to http, exact header set     |
| 6. One pack, all or nothing     | a fault between renames and rows, a fault in a replacement, kill -9 mid-job: no pack, no stray file  |
| Sweep                           | a writer job after the first frame, beside a pack job; never touches attachments/                    |
| 7. Remove a pack, purge         | rows and files gone, sent stickers intact; purge: a sticker's own file goes, the library's stays     |
| 8. Sticker entry actions        | store refuses edit_message and set_attachment_details; never an append target; vitest: hidden        |
| 8. A sticker in the feed        | Storybook: bare, chips, quote, trash; a covered sticker is drawn as a cover; shot 07                 |
| 9. GIF playback                 | Playwright visual: src swaps with visibility, Reduce Motion still, box equal, 8 MiB and 2 Mpx rules  |
| 9. GIF memory                   | web content process peak with six 8 MiB GIFs and one of 2 Mpx playing: gif-memory.txt                |
| 9. Send a GIF again             | store: a new remote_blob_id and blob_key, no 7b details copied, a covered source refused             |
| 10. Survives kill -9            | screens: the library, a sent sticker and a GIF after kill -9; shot 11                                |
| 11. Copy, search, MCP           | mirror.txt, mcp.txt; store: the sticker summary, the agent's sticker and emoji                       |
| Reader (domain)                 | the table case by case, arbitrary JSON, a group on a legacy, large or covered photo, JPEG, audio     |
| Creation                        | every creation path writes the group at the creation HLC; a sticker and a photo name the same groups |
| No writer                       | after every attachment command the bytes and HLC are unchanged; add_group, rewrite_or_add_* refuse   |
| Legacy rows                     | version 10 rows byte-identical after migration 11; a details edit adds only that group; pinned hash  |
| Derived columns                 | rebuilt on a rules-version mismatch; the fingerprint test fails until the version is bumped          |
| Hostile emoji                   | absent from the columns, the serialized view, the Markdown copy and MCP                              |
| Token on failure paths          | a trace-level capture logger over connect refused, TLS, timeout, 3xx, 401, 429, garbage, cancel      |
| Token on failure paths          | no token, link, pack name or title in any log line, IPC error or event payload                       |
| Where the token is not          | after save and clear: the database, each backup, the data folder, the copy, MCP; a positive control  |
| Keychain across builds          | a second ad-hoc-signed build reads the first one's item (before the UI; a ruling if it errors)       |
| IPC matrix                      | ipc_slice5c.rs: `sticker/id` and the library main only, token commands settings only, capture ten    |
| Build for people                | scripts/check-test-hooks.sh: no base-URL override and no in-memory token store                       |
| CSP unchanged                   | config test: connect-src and img-src as on main                                                      |
| Accessibility                   | vitest: labels, grid arrows, Enter, Escape; shot 14 (large text)                                     |
| The branch                      | scripts/pre-pr-check.py; CI green on every job                                                       |

## For the guardian
- **The group.** `sticker` = `{"emoji":<string|null>,"sticker":<bool>}`, `sv = 1`, canonical JCS, a public contract in §C.10. Reference bytes: `{"emoji":"😺","sticker":true}`.
  - **Every new attachment gets it** at the creation HLC, like 7b's three groups: send, voice, the media import, a sticker and a GIF sent again. A non-sticker gets `{"emoji":null,"sticker":false}`. So the open header does not tell a sticker from a photo.
  - **It is additive:** `schema_version` stays 1 and `core` is untouched.
  - **No writer after creation.** It is never rewritten or added, and it is not on the add-group allowlist. Rows from before 5c have no group, and there is no backfill.
- **Writer rule.** `"sticker":true` is written only by `send_sticker` and by the export import, and only after this device's probe of the bytes passes. The probe is re-checked at send time.
- **The reader reads only the record.** It does no local probe, and it never errors.
  - **Core bounds:** a sticker needs `core.kind = image`, `core.mime` `image/webp` or `image/png`, `core.width` and `core.height` each 1 to 512, and `core.byte_size` ≤ 2 MiB.
  - **The group itself** must be at most 1 KiB, canonical, and in both maps.
  - **Every failure reads as an ordinary picture,** which hides nothing, and the bytes are kept verbatim.
  - **An emoji** that is not `is_emoji_sequence` (at most 40 bytes) never reaches the columns, the view, the copy, MCP or the log.

  | `sticker` group as stored                                                | Drawn as                   |
  |--------------------------------------------------------------------------|----------------------------|
  | absent from both maps (every row written before 5c)                      | a picture                  |
  | `sticker` is `false`                                                     | a picture                  |
  | `sticker` is `true`, `emoji` is a valid emoji sequence                   | a sticker with its emoji   |
  | `sticker` is `true`, `emoji` is `null`, absent, mistyped or not an emoji | a sticker without an emoji |
  | `sticker` is absent or not a boolean; the group is not an object         | a picture                  |
  | an unsupported or missing `sv`; in only one of the two maps; over 1 KiB  | a picture                  |
  | the group is not canonical JCS                                           | a picture                  |
  | any value, when `core` fails the bounds above                            | a picture                  |
- **Derived columns.** `attachment.sticker` and `sticker_emoji` come from one function, used by the write, migration 11, the rebuild on open and, later, pull. `app_setting.sticker_rules_version` triggers a rebuild on a mismatch, with a fingerprint test, as 5a's `reaction_rules_version` does. The fingerprint includes the emoji table it shares with 5a.
- **Older clients** keep the group raw, resend it, and show a sticker as a picture in a bubble.
- **What the server learns.** Nothing new from the header. From the padded blob size it may guess that a small image is a sticker.
- **Migration 11.** Local tables, the recents and the two columns; no synced row changes. Its hash goes after 5a's in `PINNED`. KINIG-160 runs in parallel and removes no table; whichever lands second renumbers if needed.
- **§C.6's placeholder rewrite** (no `edit` revision) also covers the sticker line.
- **The token lives in the OS secret store**, as PRD §9.2 rules for secrets.
  - **Where:**
    - macOS: a generic-password item, service `me.savvinov.kinige`, account `telegram-bot-token`, not synchronizable;
    - Windows: Credential Manager, the same name;
    - Linux: Secret Service.
  - **Debug builds** use the service `me.savvinov.kinige.dev`, so a plain development run never touches the real item.
  - **A refused or locked keychain** is its own error category. There is no file fallback.
  - **Never** in `app_setting` (the database goes into 7 daily backups), the log, `settings.json`, MCP, the capture window or the main window.
  - **Before the UI is built,** the implementer verifies that a second ad-hoc-signed build reads the first build's item. If it errors, they come back for a ruling.
- **The network.**
  - one main-window command to a fixed host, URLs built with a URL builder;
  - the token only in the paths of that host;
  - the OS trust anchors and the system proxy, both stated in PRD §10.3;
  - log lines carry event kinds and categories only.
- **What Telegram learns:** the bot token, so the bot and the Telegram account that created it; the names of the packs you add; your IP address (or your proxy's); the app's name and version (the User-Agent); and when. It never learns anything of the journal.

## For slice 11
- The library, the recents and the token do not travel; each sent sticker is its own blob.
- A pulled sticker is drawn from `core` and the group before its blob arrives, in the box from `core.width` and `core.height`.
- Pull writes the two derived columns with the same reader function.
- Two devices that re-import the same export produce duplicate sticker attachments (the §C.3 limitation).
- A peer can send a `sticker` group on a legacy row; the reader's bounds are the defence.

## Touches sensitive areas
**Yes.** It touches:
- the data schema: migration 11 and its derived columns;
- sync: a new synced group on every new attachment, its reader, and §C.6's exception extended;
- server storage: no new header information;
- IPC: new commands and a protocol path;
- a secret in the OS store, and the app's first outbound request;
- MCP: two read fields.

`sync-crypto-guardian` reviewed this spec before code and reviews the PR before merge.

## Open questions
1. **What the GIFs tab holds.** Without GIF search the tab can show (a) the GIFs already in your journal, so a click sends one again as a new entry, plus "Choose a GIF file…", or (b) only the "Choose a GIF file…" button, the same as the paperclip. Recommended: (a), which works like Telegram's saved GIFs.
   **Decided by the coordinator on 20.11.2026 (reversible, the user is told):** (a).
