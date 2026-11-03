# Slice 7: Attachments: photos and files

**Budget:** ~9 h of code, ~18 h until ready for acceptance; if not ready after 36 h, stop and report (§12.1a). The work goes in this order: storage, then sending, the feed, the Markdown copy, and the Telegram media last. If the budget runs out, the Telegram media move to a follow-up PR (question 7). **Depends on:** 3, 4 (import), 6 (forest weight). Huly **KINIG-15**; branch `slice/7-attachments`, the PR and its first commit are titled `KINIG-15: …`. **Sources:** PRD §2.1, §3.2, §4.2, §4.3, §5, §6, §7.2, §7.4, §9.1, §9.3, §10.2, §12.3, §15, §16; ADR-0001 §3.5, §7, appendix §A.0, §C; ADR-0008, ADR-0009, ADR-0011; slice 4 decision 5.

## Scenario
1. In Inbox I press the paperclip ("Attach file") to the left of the field. The system dialog opens and I pick two photos and a PDF. Tiles appear above the field: two thumbnails and a file card with the name and size, each with a ×. I type a caption and press Enter. The entry shows the photos as an album, the file card below them, then the text and the time.
2. I paste a screenshot with ⌘V and drag a file from Finder onto the window. While I drag, the feed shows "Drop to attach". Both become tiles. Sending works with no text at all.
3. I restart the app or kill it with `kill -9`. The tiles of the unsent entry are still above the field.
4. I open an old chat. The images take their final size from the first frame, so nothing below them moves when they load. An image that can't be decoded and a missing file show a tile of the same size ("No preview", "File is missing").
5. A click on a photo opens it full size over the window. ←/→ move between the entry's photos, Escape closes it, and "Open in default app" opens it in Preview. A click on a file card opens it in its system app. Executables and scripts are shown in Finder, never run.
6. I delete the entry. It goes to the trash with its attachments and they show there. "Restore" brings everything back. "Delete forever" (or 30 days) removes the files from disk.
7. In `~/Documents/Kinige/Inbox/2026/` the day file has `![IMG_0001.jpg](<2026-11-03/IMG_0001.jpg>)` and `[menu.pdf](<2026-11-03/menu.pdf>)`, and the files are in `2026-11-03/` next to it (question 3).
8. Search for «меню» finds the entry by the file name `menu.pdf`. The forest counts the photos: a day with photos grows more (§7.2), and the day card says "5 notes · 3 photos".
9. I import my Telegram export again (6.8 GB). The preview says "Media: 1,240 files, 6.7 GB · not in the export: 12". Progress runs in gigabytes and can be stopped or killed. A second run picks up where the first stopped and never attaches a file twice. Afterwards the old entries show their photos and files where "[Photo not imported]" used to be (question 4).

## Included
- **Storage (`kinige-store`, PRD §4.3).**
  - Files: `<data>/attachments/<attachment_id>.<ext>`, one file per attachment, **no deduplication** (§4.3, ADR-0001 §7: one blob, one attachment). `ext` is the lowercase original extension if it matches `^[a-z0-9]{1,10}$`, otherwise one from the detected format, otherwise `bin`.
  - The copy pipeline (§3.2): stream the source in 1 MiB chunks into `attachments/.tmp/<id>.part`, compute sha256 and the byte count on the way, check the byte count against the source size, `fsync`, rename into place, then write the row in the writer transaction. A file is never loaded into memory.
  - Thumbnails go to `<data>/thumbs/<id>.jpg` (`.png` if the image has alpha). They are built lazily on first display by a pool of two threads, fit in 960 × 960 and have EXIF stripped. They can be lost and are rebuilt.
  - The startup sweep runs in the background after the first frame. It empties `.tmp/` and `open/`, deletes thumbnails that have no row, and moves files that have no row into `attachments/.orphans/`. A row whose file is in `.orphans/` gets the file back, which covers restoring an older database backup and then a newer one. Orphans are deleted after 30 days. A row without a file gets `attachment_local.state = 'missing'`.
- **Formats** (decided by content, not by extension; a file that can't be read gets `attachment_unreadable`):

  | Input                                 | kind    | In the feed                               |
  |---------------------------------------|---------|-------------------------------------------|
  | JPEG, PNG, WebP, GIF that decode      | `image` | thumbnail; GIF as a still frame; lightbox |
  | HEIC, AVIF, TIFF, BMP, SVG, RAW       | `file`  | file card (question 2)                    |
  | video by extension or MIME            | `video` | file card; opens in the system player     |
  | audio by extension or MIME            | `audio` | file card; playback comes in 7a           |
  | anything else, or an image that fails | `file`  | file card; opens in the system app        |

  For an `image`, only the header is read when it is attached. `width` and `height` are stored after the EXIF orientation is applied (swapped for 90° and 270°). Crate: `image` 0.25.10, already in `Cargo.lock` through tauri with PNG only. Add `default-features = false` and the features `jpeg`, `png`, `gif`, `webp`. Verified in the crate source: `ImageReader::limits` and `into_dimensions`, `ImageDecoder::orientation`, `DynamicImage::apply_orientation` and `thumbnail`. Licenses: `image` is MIT/Apache-2.0, `zune-jpeg` and `zune-core` are MIT/Apache-2.0/Zlib (verified). `gif` and `image-webp` are **not verified**; `cargo deny` decides, and if either fails, that format becomes `file`. Decode limit: `Limits::max_alloc` 512 MiB. A larger image is still an `image` with a "No preview" tile. No HEIC decoder exists in this crate.
- **Limits (question 1):** 2 GiB per file, 10 attachments per entry, 64 MiB per paste. Errors appear above the field and nothing is written: `attachment_too_large`, `attachment_limit`, `attachment_unreadable`, `disk_full`. A dropped folder is skipped, with "Folders are not attached".
- **Schema, migration 7 (ADR-0001 appendix §C.6, new):**
  - `attachment` is a **synced** table: the PRD §4.2 columns plus `blob_key`, `server_known`, `groups_raw`, and an index on `message_id`. Its groups are `core`, `order` (`sort_index`) and `deleted` (§A.0). All three are written under one HLC of their own with `sv = 1`.
  - `core` is immutable. It carries `remote_blob_id` (`Uuid::now_v7()`, uuid feature `v7`) and `blob_key`, 32 bytes from the OS CSPRNG (`getrandom`, already in the tree; this is what secretstream `keygen` does). Both are created now, in the send transaction (ADR-0001 §7): they can't be added later. `blob_key` and `sha256` are encoded as lowercase hex. The exact JCS bytes go into §C.6.
  - Local tables: `attachment_local(attachment_id, state, file_name)` with `state` limited to `local | missing` until slice 11, and `draft_attachment(draft_key, attachment_id, sort_index, kind, filename, mime, byte_size, width, height, sha256, file_name, created_at)`. A draft is deleted only when its text, reply and attachments are all empty.
  - The pre-migration backup is taken as in §C.2.
- **Sending.** `send_message(…, attachment_ids)`. Rust accepts only the chat's own draft attachments, with no duplicates and in their order; otherwise `attachment_not_found` and nothing is written. The text may be empty if there is at least one attachment. One transaction writes the message, the `attachment` rows, the search index (the body plus file-name stems; `SEARCH_INDEX_VERSION` goes up, so the index is rebuilt on open) and `mirror_dirty`.
- **Trash and purge.** Trashing an entry hides its attachments with it; their own `deleted` group is not touched. Purge writes tombstones `('attachment', id, purged_of, …, cascade_of = message id)` in the same transaction as the message (ADR-0001 §3.5, as for revisions). The file and the thumbnail are deleted after the commit; if the app crashes in between, the sweep removes them. There is no adding or removing attachments on an entry once it is sent.
- **The webview reads files only through a custom protocol,** `kinige-media://localhost/{thumb|full}/<uuid>`, registered with `register_asynchronous_uri_scheme_protocol`. The handler answers only when `UriSchemeContext::webview_label() == "main"` (verified in tauri 2.12.5) and gets 403 otherwise. A bad id, a purged id or any other path gets 404. A URL never carries a filesystem path. The asset protocol is not used: its scope is not per window. CSP `img-src` adds `kinige-media:` and the Windows origin `http://kinige-media.localhost` (**not verified**; the Windows screenshots check it).
- **IPC (§12.3), `main` only:**
  - `attach_pick(chat_id)`: the dialog runs in Rust (`blocking_pick_files`, verified in plugin-dialog 2.8.3).
  - `attach_paste(chat_id, name_hint, raw bytes)`: `InvokeBody::Raw`. The name is only a hint and is sanitized; a nameless paste is named `pasted-HHMMSS.<ext>`.
  - `attach_dropped(token, chat_id)`: Rust takes the paths from `WindowEvent::DragDrop::Drop` of `main` and gives JS a one-time token, never the paths.
  - `remove_draft_attachment`, `open_attachment(id)`. `open_attachment` clones the file into `<data>/open/<id>/<original name>` (`fs::copy` = `fclonefileat` on macOS per the std docs) and opens the clone with the opener plugin's `open_path`, or `reveal_item_in_dir` for executables. Edits in the system app never touch the stored file.
  - `get_draft` returns the draft's attachments; `MessageView` gets `attachments[]` (id, kind, filename, mime, size, width, height, state).
  - `capture` keeps exactly seven commands. `settings` is denied all the new commands and the protocol. A drop on those windows does nothing.
- **Feed and components** (`design-system`, tokens only; new `layout.attachment.*` tokens). One image goes up to the bubble width (~65ch) and 400 px high; 2–10 images form an album grid like Telegram's. The box is sized from `width`/`height` before load. The file card shows an icon, the name with a middle ellipsis, and the size with the extension. Also: the pending tile with ×, the drop overlay, and the lightbox as a dialog with focus trap. No zoom animation; Reduce Motion has nothing to remove. Screen reader label: "Photo IMG_0001.jpg, 1280 by 960". An entry without text gets a summary instead of an excerpt in quotes, "On this day", the forest card, search and the capture context ("Photo", "3 photos", "menu.pdf"). Rust returns a summary structure; the strings are ru/en.
- **Forest:** `NoteStat.attachments` counts live attachments, and the day card gets "N photos" (§7.4, deferred by slice 6).
- **Telegram media (Huly KINIG-15).**
  - The parser keeps `photo` and `file` (the paths), `file_name`, `mime_type`, `file_size`, `width`, `height` and `duration_seconds`; these are in the `real-saved.json` fixture. A "File not included…" value means the file is not in the export.
  - The session keeps the folder of the chosen `result.json`, in Rust only. A path must be relative, contain no `..` and, after canonicalization, stay inside the folder as a regular file; otherwise it counts as "skipped: bad path".
  - Attaching is idempotent by `ext`: an entry found through `message_ext` that already has any attachment row (live or in the trash) is skipped.
  - A batch is committed after 100 entries or 256 MiB copied, whichever comes first. The stop flag is checked between files.
  - `kill -9` loses at most one batch, whose files are swept up later.
  - Disk full: the batch is rolled back and the report says "stopped: not enough disk space". `result.json` is still read whole (≤ 1 GiB, slice 4); media are streamed.
- **Documents:**
  - ADR-0014 "Attachment files": the custom protocol, the crate features, the paths and the sweep, opening a clone, the hex encoding, the placeholder rewrite.
  - ADR-0001 appendix §C.6.
  - PRD v0.18: §4.3 (the Markdown copy layout), §9.3 (if question 6 changes it), §12.3 (the matrix and the protocol), history.
  - `CLAUDE.md` "State" in the first commit.

## Not included
- Voice recording and playback (7a). Inline video and audio players (P2 video; audio in 7a). Stickers, GIF search, link previews and `link` attachments (P1). Telegram stickers and animations stay placeholders.
- Editing, rotating or cropping images; per-image captions; changing a sent entry's attachments. Dock drop (§5.1, a spike). HEIC conversion (question 2).
- Upload, sync, blob encryption, the `uploading`/`synced`/`remote_only` states (slice 11). Deduplication and the garbage collector (§4.3). The full export (§16).
- The feed "media" filter (§6) and the search "type" filter (§9.1): a separate task. Thumbnails in quotes, "On this day", search results or the forest card. Virtualization (slice 8). Attachments in the capture window and through ⌥⇧V (question 5).

## What “done” looks like
A 1280 × 800 window. The composer shows the paperclip `IconButton` ("Attach file", Lucide `Paperclip`, ISC; add it to `icons/README.md` if it is missing) and a row of pending tiles above the field. In the feed, an album of 1, 2, 3, 4 and 10 photos, a portrait photo with EXIF rotation, a transparent PNG, a file card with a long name, video and audio cards, and the "No preview" and "File is missing" tiles. The lightbox fits the window over a scrim token, with the counter "2 / 4" and the "Open in default app" button. The drop overlay is a quiet outline with no animation. Storybook stories exist for each of these in en and ru, normal and large text; there is no reference concept. A 640 px window keeps everything reachable.

## Checks
- **Rust (domain):** kind by format, extension rules, mirror naming and clashes, the summary, the weight with attachments, the `core` bytes against the reference (§C.6).
- **Rust (store):**
  - pipeline: `abort` between rename and commit, then sweep, gives no orphans and no half rows;
  - a source that changes size while copying gives `attachment_unreadable`;
  - ENOSPC, injected, rolls back;
  - 0-byte file, a 2 GiB + 1 file (sparse), a truncated JPEG, a decompression bomb (100k × 100k header) gives "No preview" without OOM, an EXIF orientation of 6;
  - send: garbage `attachment_ids` (not a UUID, 1 MiB, another chat's, duplicates, 11 ids);
  - trash, restore and purge: tombstones, files gone after the commit;
  - backup restore: the `missing` state and the return from `.orphans/`;
  - migration 6 → 7 keeps drafts;
  - the Markdown copy files and links;
  - search by file name;
  - `log_privacy`: no file names or paths in the log.
- **Telegram:** a synthetic export folder built in code. Re-import attaches once; stop and repeat give no duplicates; `kill -9` mid-file loses nothing; `../`, an absolute path, a symlink out and a missing file are handled; the placeholder is removed only from text that has not been edited since import.
- **IPC and protocol (`ipc_slice7.rs`):** allowed for `main`, denied for `capture` and `settings`; the protocol answers 403 to other labels and 404 to `..`, `%2e%2e` and unknown ids; capture still has seven commands.
- **Frontend on `MockApi`:** button, paste, drop token, ×, the send errors, the lightbox keyboard, a box height equal before and after load, ru/en keys. Visual baselines through `visual-update.yml`.
- **Screenshots in `docs/screens/7/`** (`scripts/screens/7.sh`, macOS runner, AX input only). `seed_demo` with `KINIGE_SEED_ATTACH=1` draws synthetic images with the `image` crate (no third-party assets) and attaches them through the store API, plus a synthetic Telegram export imported by the store. Shots:
  - 01 the feed with albums and cards;
  - 02 the paperclip dialog open, then cancelled by AX;
  - 03 a synthetic PNG put on the pasteboard with `osascript` and pasted with the AX "Edit → Paste" menu item (whether WKWebView hands a pasted Finder file to `clipboardData.files` is **not verified**; the user checks it by hand);
  - 04 after `kill -9`, the tiles are still there;
  - 05 sent;
  - 06 the lightbox;
  - 07 the trash;
  - 08 restored;
  - 09 Telegram entries with their media;
  - 10 search by file name;
  - 11 the forest card with photos;
  - 12 en;
  - 13 large text;
  - 14 640 px.

  Drag and drop has no AX path: the Rust handler test and vitest cover it, and the user drags by hand at acceptance. Texts: `no-jump.txt` (the AX `frame` of the last bubble before and after the thumbnails load, cold thumbnail cache, must be equal), `mirror.txt` (the tree and the day file), `files.txt` (`ls` of `attachments/` before and after the purge), `bench.txt`, `cold-start.txt`. **Windows** (`7-windows.ps1`): 01 the feed with images and 02 the lightbox, to check the protocol origin and the CSP.
- **Performance (§15)** on `import_bench synthetic 100000 --images 10000` (10k synthetic JPEGs of about 1280 × 960 with mixed aspect ratios):
  - opening Inbox p95 < 300 ms;
  - search by file name p95 < 300 ms;
  - `get_forest_year` p95 compared with slice 6;
  - cold start < 1.5 s (the sweep is not on the path);
  - thumbnail build p95 per image;
  - the media import on a synthetic 2 GB export: throughput and peak RSS < 300 MB (`/usr/bin/time -l`) → `import-media.txt`.

  The user scrolls the real feed at acceptance to judge jank by hand (as in slice 6).

## Touches sensitive areas
**Yes, `sync-crypto-guardian` before merge:**
- migration 7 and the first synced `attachment` rows: the groups, the immutable `core` with `blob_key` (CSPRNG) and `remote_blob_id` (v7), the hex encoding;
- the purge cascade and its tombstones;
- rewriting `body` to remove the Telegram placeholder: no revision, because the old text is the new text plus a known line; this is an exception like ADR-0006 and goes into §C.6;
- the new IPC commands and the custom protocol, which sits outside the capability system (the label check);
- paths from drop and from the Telegram folder;
- new crate features (`image`, `uuid/v7`, `getrandom`).

## Product decisions (the user, 03.11.2026: every recommended default below was chosen)
1. **Limits:** 2 GiB per file (Telegram's free limit), 10 per entry, no total quota.
2. **HEIC and other formats with no preview** (AVIF, TIFF, RAW, SVG): stored unchanged as a file card that opens in Preview, with no conversion.
3. **Markdown copy:** files go into a folder per day next to the day file (`Inbox/2026/2026-11-03/IMG_0001.jpg`), with standard relative links. On APFS on the same volume the clone takes no extra space; on another disk the 6.7 GB doubles.
4. **Telegram media:** import photos, files, videos and voice/audio (videos and audio as file cards until there are players), and leave stickers and animations as placeholders. When a file is attached, remove its "[… not imported]" line if the text has not been edited since import.
5. **Quick capture:** no attachments in the capture window and ⌥⇧V stays text-only in slice 7 (the PRD is silent; §12.3 keeps that window to text).
6. **AI agent (MCP):** the agent sees each entry's attachment metadata (kind, file name, size, image size), never the bytes or paths, and search by file name works for it too.
7. **Scope:** the Telegram media stay in slice 7 and are done last. If the budget runs out, they go into a follow-up PR (7b) after acceptance of the rest.
