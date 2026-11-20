# KINIG-136: slice 7b, attachment details

**Budget:** code 10 h (1.25 days), review and verification 30 h (3.75 days; verification has been about 3× the code). Stop and report at 2× (80 h in total). Re-estimated on 19.11 after the user's answers: the special cases for hidden photos in search, MCP and the Markdown copy, and their tests, are gone. Order: the domain (groups, the read function, add-group, counting) and its reference bytes, migration 9 and the store commands, then the cover and the dialog, search, the copy, MCP, screenshots.
**Task:** Huly KINIG-136. **Branch:** `slice/7b-attachment-details`. The PR title and first commit start with `KINIG-136: …`. **Depends on:** 7, 7a, KINIG-132 (frameless picture bubbles), KINIG-135 (the 144 px pending tiles and the viewer opened from a tile), all on `main`. **Lands before 5a** (KINIG-138), which reuses the add-group function. **Sources:** the user's request of 18.11.2026 (after Discord's attachment editor) and answers of 19.11.2026; the `sync-crypto-guardian` pre-review of this spec; PRD §4.2, §4.3, §6, §9.1, §9.3, §10.2–§10.4, §12.3, §16; ADR-0001 §3.2, §3.3, §4, §5, §10, appendix §A.0, §C.6, §C.7; ADR-0009, ADR-0014.

The coordinator decided the scope and the technical points below (three fields, a cover that does not depend on the picture, the honest limits); the user answered the product questions on 19.11.2026 (see "Product decisions").

## Scenario
1. In Inbox I attach three photos and a PDF. Each tile in the composer has two round buttons in its top right corner: "Details" (a pencil) and ×. A click on a photo's tile still opens it in the viewer. I press "Details" on the first photo: the "Attachment details" dialog opens with the picture, the name `IMG_0001` followed by a fixed `.jpg`, an empty "Description" and a "Hide as spoiler" checkbox. I rename it to `beach`, write "Beach, July", tick the box and press Save. The tile now shows the cover. For the PDF the dialog has a name and a description and no checkbox.
2. I press Enter. The entry shows the album with the first photo covered, and the PDF card under its new name with my description as a second line.
3. The cover is a quiet grey shape of the photo's own size with a crossed-out eye and "Hidden" (only the eye in a small album cell). Nothing of the picture shows through it, and moving the pointer over it changes nothing. A single hidden photo with no text sits in a frameless bubble like any picture, and the time pill stays on the cover.
4. I click the cover: that photo, and only that one, appears, with a small "Hide" button in its corner on hover or focus. Nothing around it moves. I switch to another chat and back: it is covered again. Closing the window to the menu bar covers it too.
5. I click the revealed photo: the viewer opens with "Beach, July" under the picture. ← and → move through the album; a covered photo shows the cover with a "Show" button in the viewer and in the details dialog as well.
6. On an older entry I right-click a photo and choose "Hide as spoiler": it is covered at once, and nothing else about it changes. "Details…" in the same menu (or "Attachment details…" in the entry's menu) opens the dialog. The entry does not become "edited".
7. I kill the app with `kill -9` and start it: every spoiler photo is covered, the names and descriptions are kept, nothing is revealed.
8. Search for "beach" finds the entry by the photo's description and new name. Search results show no pictures, so the line is the entry's text or "Photo". Opening it shows the photo covered in the feed. Search for `IMG_0001` finds nothing: a renamed photo is found by its new name only.
9. `~/Documents/Kinige/Inbox/2026/2026-11-18/` holds `beach.jpg`, and the day file has `![Beach, July](<2026-11-18/beach.jpg>)`, as for any photo. The PDF line is `[<its description>](<2026-11-18/<new name>.pdf>)`.
10. An AI agent over MCP sees both attachments with their names and descriptions; the photo also carries `"spoiler": true`, so the agent can tell it is hidden in the window.

## Included
- **Three details per attachment.**
  - **Name:** the display name. The dialog edits only the stem; the original extension is a fixed suffix. The stored file `attachments/<id>.<ext>` is never renamed: it is named by the id. `core.filename` stays the original and immutable. An empty stem, or a name equal to the original, is stored as `null` (the original).
  - **Description:** plain text, never Markdown or HTML, at most 1,024 Unicode scalar values (Discord's alt-text limit, not verified). Empty means none.
  - **Spoiler:** a flag, offered only for `image`. A file card has no picture to cover, and voice notes have no details at all (below).
- **Counting and normalizing are Rust's (`kinige-domain`).** The UI counter is advisory; MockApi repeats the rules from `fixtures/api-contract/`.
  - Name: the IPC takes the stem; Rust trims it, refuses C0, DEL, C1, path separators and bidi overrides (slice 7's filename rules), appends the extension of `core.filename` and checks the whole at most 255 UTF-8 bytes. One extension function serves the name rule, the Markdown copy and "Open in default app" (today `open.rs` and `stored_ext` each have their own): it compares bytes or ASCII-folded bytes, never Unicode lowercasing.
  - Description: CRLF and a lone CR become LF, a TAB becomes one space, then leading and trailing whitespace is trimmed. Any C0 left other than LF, DEL or C1 is refused. The count is taken after this, in scalar values. A lone surrogate cannot reach Rust as a `String`: the IPC layer refuses it.
  - Errors: the new `attachment_details_invalid` (any rule above); `message_in_trash` for a trashed entry, as 5a; `attachment_not_found`; `read_only`. Nothing is written on an error.
- **Data (`kinige-domain`, `kinige-store`), see "For the guardian".**
  - Three new synced groups on `attachment`: `name`, `description`, `privacy`. `new_attachment` writes all three for every new attachment (send, `send_voice`, the Telegram media import) under the creation HLC, with the draft's values or the defaults.
  - **Invariant: no code path writes a default-valued group to an existing row.** There is no backfill in the migration and no repair on read or on pull. An absent group means its default.
  - **One total read function** in `kinige-domain`, from the merged record to `(display_name, description, spoiler)`. It never errors. The columns hold only its validated output; the raw bytes stay verbatim in `groups_raw`.
  - **Migration 9** (`ALTER TABLE … ADD COLUMN`, no rewrite): `attachment.display_name`, `attachment.description`, `attachment.spoiler INTEGER NOT NULL DEFAULT 0` (the read function's output), and the same three on the local `draft_attachment`. Its hash is appended to `PINNED` (`schema/tests.rs`). Rows written before 7b keep their bytes. Backup before migrating as in §C.2. **Promote rule:** when a draft attachment is sent, a draft name or description that fails the rules becomes `null`; the spoiler is kept.
  - **Commands take changes, not the whole triple.** Each field is "leave" (the key is absent) or a value (`null` clears the name or the description; the spoiler takes `true` or `false`, never `null`). Rust writes a group only for a field the caller sent whose normalized value differs from the stored one. MockApi and the api-contract fixture use this shape from the start.
    - `set_draft_attachment_details(attachment_id, changes)`: a local write to `draft_attachment`, no HLC.
    - `set_attachment_details(attachment_id, changes)`: one writer transaction. A changed field whose group exists is rewritten with `rewrite_group`, under a new HLC strictly greater than before. On an older row whose group is absent, the group is added. The transaction re-indexes the entry when the name or the description changed, and marks the day dirty. No revision and no `edited_at`: the entry's own groups are untouched. Equal values write nothing.
  - The context menu's "Hide as spoiler" / "Show without spoiler" sends only the spoiler.
- **The add-group function (generic, `kinige_domain`; 5a reuses it).**
  - A per-entity allowlist of the groups that may be absent: `attachment` {`name`, `description`, `privacy`}; 5a adds `message` {`reactions`}. Any other missing group stays an error: a message without `body` is corrupt.
  - "Absent" means absent from both `groups_raw` and `hlc_json`. A group present in one of them and not the other is `Corrupt`.
  - The new group's HLC is strictly greater than every HLC of the record. This rule is shared with 5a. The other groups, unknown groups and the extra fields of other `hlc_json` entries stay byte-identical.
  - Appendix §A.0's "a missing group … is an error (not a re-creation)" is amended to name this exception.
- **The cover (`design-system`, tokens only), only in the main window.**
  - **Where:** every place that draws a picture today. That is the feed's `Album` (`AlbumCell`, a single photo and the grid), the trash (`TrashView` through `MessageAttachments`), the viewer (`Lightbox`, opened from the feed and, since KINIG-135, from a pending tile), the pending tile (`PendingTiles.tsx`, `ImageTile`) and the details dialog. Search results, "On this day", the reply quote, the forest card and the capture window show only text summaries, so they draw no picture and need nothing; a future surface that shows a thumbnail must use the same cover.
  - **It does not load the picture at all:** no `img` element and no `kinige-media` request until it is revealed. Thumbnails are built lazily on the first request (slice 7), so a never-revealed photo has no file in `thumbs/`.
  - **Look.** It is drawn from tokens only, in the picture's own box: the box from `width`/`height` that `Album` and the 144 px tile already reserve, so the aspect ratio is all it shares with the photo. The Lucide `EyeOff` icon (not in the set yet: added to `Icon.tsx` and `icons/README.md`, ISC) and "Hidden" / «Скрыто»; the icon only in a cell or tile too small for the word. Neither blur nor animation (see Reference).
  - **In a frameless picture bubble (KINIG-132, `message-bleed.css`):** the cover fills the cell edge to edge, and the bubble's corners clip it like the picture. In a bubble with no text the time pill stays on the cover at the bottom right and takes no click, so a click on it reaches the cover. The narrow-picture floor and its side strips stay as they are. In a bubble with no fill (`message--bleed-bare`) the cover's token must stand out from the feed, as the preview tile's fill does there; its contrast is checked like every pair.
  - **On a pending tile (144 px high, 108–256 px wide):** the cover fills `.pending-tile__open`. The tile gets a second round button, "Details" (Lucide `Pencil`, already in the set as `edit`), at the same size and style as ×, to its left. Both stay on top of the cover. A hidden tile's first click reveals it in place; a click on a revealed tile opens the viewer, as today.
  - **Reveal.** The cover is a button: a click, Enter or Space reveals that one picture. The reveal state lives in memory per chat view. The photo is covered again when I leave the chat or the trash, close the main window, restart the app or press "Hide". The viewer and the dialog share the state.
  - **Screen reader:** a covered photo is "Hidden photo, press to show". Revealed, the label is the description if any, otherwise slice 7's "Photo beach.jpg, 1280 by 960". A covered photo's description is neither read nor shown until it is revealed.
- **Description and name on screen (main window).**
  - **A single photo:** the description is one caption line under the picture, in the text's place of the KINIG-132 layout. A picture bubble with a caption is laid out like a picture bubble with text: the time at the end of that line, no pill. The caption is cut with an ellipsis, and the full text is in the viewer.
  - **A hidden single photo with a description** shows the muted line "Description hidden" / «Описание скрыто» in that place until revealed. Revealing swaps the text in the same line, so nothing moves on reveal.
  - **An album:** captions only in the viewer, under the picture.
  - **A file card:** the display name, and the description as a second line.
  - **The details dialog** edits one attachment; from an entry with several, ← and → in the dialog move between them.
- **Other windows.** The capture window shows attachments only as text summaries (`CapturePanel.tsx`), and the settings window's access log shows no attachments. Neither draws a picture, so neither needs the flag or the cover. They receive the shared `AttachmentView` with its new fields, like the main window. `AttachmentSummary.name` uses the display name; `summaryOf` and its fixture follow.
- **Search:** the index takes the stems of the display name and the description of every live attachment, hidden or not. The original name is not indexed once renamed. Toggling the spoiler changes nothing in the index. Snippets keep today's rules. `SEARCH_INDEX_VERSION` stays 2: no existing row is renamed or described.
- **"Open in default app" (ADR-0014):** the open-or-reveal verdict and the clone's extension come from `core.filename`, as today. The display name gives at most the clone's stem: a `README` renamed `README.doc` must not open in Word.
- **Markdown copy:** a hidden photo is written like any other photo. The day-folder file takes the display name (slice 7's clash rules; the copy removes the file it wrote under the old name). The link text is the description, otherwise the name. Newlines become spaces, and `\`, `[`, `]`, `` ` ``, `<` and `>` are escaped, so no description can break the day file or start a heading.
- **MCP (ADR-0009):** `AgentAttachment.name` becomes the display name, and it gains `description` and `spoiler` (present as `true` only on a hidden photo, absent otherwise, like the other optional fields). A hidden attachment is otherwise returned like any other.
- **IPC (§12.3), `main` only:** the two commands; `capture` keeps exactly seven commands and `settings` is denied both. The capability files and the app manifest are updated. `MessageView.attachments[]` and `get_draft` carry `name`, `description` and `spoiler`; `filename` stays the original. The `kinige-media` protocol is unchanged.
- **Documents:**
  - ADR-0001, revision 12 (5a takes 13): §3.2 (the attachment's groups), §10 (what the server learns), appendix §A.0 (the three groups and the add-group exception), and a new §C.8 (the bytes, the read function, the limits, the reference vectors; 5a takes §C.9). The bytes of the three groups become a public contract there.
  - ADR-0009: the attachment's `description` and `spoiler` fields.
  - ADR-0014: "Open in default app" decides from `core.filename`.
  - PRD v0.27 (v0.26 is KINIG-134's; 5a takes v0.28):
    - §4.2, §4.3, §6, §9.1, §9.3;
    - §10.2: two concurrent edits of the same detail keep the later one, without a trace;
    - a §12.3 paragraph and the history line.
  - `CLAUDE.md` status in the first commit.

### What the spoiler is
It is a cover in the Kinige window against a glance over the shoulder, and nothing else. The dialog says so in one line under the checkbox. Everywhere else the photo is an ordinary photo:
- the file and its thumbnail in the app's data folder;
- "Open in default app", which shows it in Preview;
- the Markdown copy in Documents: the picture, its name and its description;
- search, which finds it by its name and description;
- MCP: an AI agent gets its name, its description and the flag (never the bytes, as for every attachment);
- database backups; restoring a backup made before the photo was hidden shows it again;
- the full export (§16, not built yet).

The sync server (slice 11) cannot see which photo is hidden or what is in it, but it can tell when a photo was hidden or shown (see "For the guardian").

## Not included
- Image editing (crop, rotate for good, drawing); spoilers for text; a lock or a password on the app; hiding whole chats.
- A spoiler by default for a chat (the user, 19.11: not now). A spoiler for file cards, videos and voice notes; details of `audio` attachments (the voice player shows no name).
- Telegram's own spoiler flag in the export: whether the export carries it is not verified; imported media come uncovered.
- Stickers and GIFs (P1). Several captions in one album shown in the feed. Undo of a details edit (edit it again).
- Sync itself and applying groups on pull (slice 11).

## Reference (UI work)
- **Telegram desktop:** in the "Send photos" box, the right-click menu of a photo has "Hide with spoiler"; in the feed such media is blurred under moving particles and a click reveals it (Telegram 9.2 and later; the version is not verified). Telegram has one caption per message and no per-photo description or rename.
- **Discord:** a pending attachment has small buttons on its corner, and the pencil opens an editor with the file name, a description (alt text) and "Mark as spoiler". A spoiler is blurred with a "SPOILER" label and revealed by a click. The description is alt text only, not shown.
- **We copy:** Discord's pencil on the pending tile and its three fields in one small dialog; Telegram's frameless picture with the time pill, now on the cover too; the entry's menu after sending; reveal by click.
- **We change:** no blur, because a strong blur of a small thumbnail still shows colours and shapes; no particles, because the UI stays quiet (no gratuitous motion); the description is also shown as a one-line caption, because in a journal it is part of the note.

## Decisions left to the implementer
- How "leave" and "clear" look in Rust and TypeScript (an absent key against `null`), as long as the semantics above hold and garbage is refused.
- The menu placement: recommended "Details…" and "Hide as spoiler" / "Show without spoiler" at the top of the context menu of a right-clicked photo or file card, and "Attachment details…" in the entry's menu (hover "more") for an entry with attachments.
- The cover's tokens (`color.attachment.cover`, the icon size per cell size, the Details button's offset): the `design-system` agent adds them.
- Where the reveal state lives (a hook keyed by chat view), as long as no reveal is stored anywhere.

## What "done" looks like
A 1280 × 800 window.
- **The details dialog** (the existing `Modal` of `web/src/ui/settings/`, moved to a shared place if needed, with its focus trap):
  - the picture on the left, covered when the attachment is hidden;
  - "Name": a `TextField` with the fixed extension as a suffix;
  - "Description": a multi-line variant of `TextField`, which has none today, with a counter from 900 characters on;
  - "Hide as spoiler": a `Checkbox`, images only, with its one-line note;
  - Cancel and Save (`Button`).

  Enter in the name saves, Escape cancels. An entry with several attachments shows "2 / 4" and ← →.
- **The composer:** a row of 144 px tiles with a covered photo, a revealed one and a file, each with "Details" and ×.
- **The feed:**
  - a covered single photo in a bare frameless bubble with its pill on the cover;
  - a covered photo with the "Description hidden" line and the time at its end;
  - a covered cell in an album of four;
  - a revealed photo with its caption line and its "Hide" button;
  - a file card with a new name and a two-line description.
- **States:** a covered photo whose file is `missing` shows the cover (never "File is missing" with a name); a description at 1,024 characters cut to one line; a tall screenshot strip on the narrow-picture floor, covered; Russian and English; large text; a 640 px window.
- Storybook stories for the dialog, the cover in a bare picture bubble (`PictureBubble.stories.tsx`), in an album cell and on a pending tile, and the caption line, in en and ru, normal and large text.

## Checks
| Scenario step / rule         | Test or evidence that proves it                                                        |
|------------------------------|----------------------------------------------------------------------------------------|
| 1. Details on a pending tile | vitest `DetailsDialog` and the tile's Details button; draft values survive a reopen    |
| 2. Sent with its details     | store: the three groups hold the draft values; bad draft values become null; shot 05   |
| 3. Cover, nothing loaded     | vitest: no `img` or `thumb` URL; real window: no `thumbs/` file, the neighbour has one |
| 4. Reveal and re-cover       | vitest: one cell at a time; leaving the chat or window covers; shots 02, 07            |
| 5. Viewer and dialog         | vitest: both show a hidden photo covered, then the caption; shot 06                    |
| 6. Edit after sending        | store: only the sent field's group, new HLC, rest byte-identical; `message_in_trash`   |
| 7. Restart and kill -9       | screens: covered after `kill -9` and start, details kept; shot 09                      |
| 8. Search                    | store: found by description and new name, hidden or not; old name not found; shot 10   |
| 9. Markdown copy             | domain: backtick, `<`, `](`, newline then `# ` keep the day file intact; `mirror.txt`  |
| 10. MCP                      | store: a hidden photo has its name, description and `spoiler: true`; `mcp.txt`         |
| Pill on the cover            | Playwright `visual`: the cover fills the frameless bubble, the time pill stays on it   |
| Convergence                  | property over all merge orders: rename keeps the spoiler, hide keeps name and text     |
| Add-group function           | domain: absent, half-present, not allowed; other bytes and `hlc_json` extras identical |
| Total read function          | property over arbitrary JSON, a 900 KiB description included; the spoiler table        |
| Counting vectors             | `fixtures/api-contract/` run by Rust and vitest: 1,024/1,025 astral, CRLF, 255 bytes   |
| Open in default app          | store: `reveal_only` and the clone's extension unchanged by a rename                   |
| Fault before commit          | store: groups, columns, index and `mirror_dirty` unchanged                             |
| Migration 9                  | pinned hash in `PINNED`; rows of version 8 unchanged; fresh database                   |
| Garbage input, windows       | `ipc_slice7b.rs`: garbage per command; `capture`, `settings` denied                    |
| Accessibility                | vitest labels and keyboard; shot 12 (large text)                                       |
| Log privacy                  | `log_privacy`, failure paths too: no name or description in the log                    |

- **Convergence property:** a legacy row (no new groups), details edits from two devices, every merge order. A rename never changes the effective spoiler, and a hide never changes the name or the description.
- **Add-group:** absent from both places adds; present in only one is `Corrupt`; a group not on the allowlist is refused. The other groups and the extra fields of other `hlc_json` entries stay byte-identical.
- **Read function:** a property over arbitrary JSON in the three groups (a 900 KiB description included) never errors and yields only valid values. A local rewrite of a mistyped `name`, `description` or `privacy` group succeeds. The spoiler table:

  | `privacy` group as stored                                       | Shown as  |
  |-----------------------------------------------------------------|-----------|
  | absent                                                          | uncovered |
  | `{"spoiler":false}`                                             | uncovered |
  | `spoiler` `true`, `null`, absent or mistyped; not an object     | covered   |
  | an unsupported `sv`                                             | covered   |
  | any value on a kind other than `image` (ignored, kept verbatim) | uncovered |
- **Counting vectors** in `fixtures/api-contract/`, run by Rust and by vitest: astral characters at 1,024 and 1,025, CRLF and a lone CR, a TAB, a multibyte name at exactly 255 bytes with its extension and one byte over, a lone surrogate.
- **Garbage per command:** not a UUID, another chat's or a purged id, a 1 MiB description, 1,025 characters, NUL, ESC, a bidi override in the stem, a stem with `/`, `spoiler: null`, `spoiler: true` on a file card, an unknown key.
- **Fault injection** before the commit of `set_attachment_details`: the groups, columns, index and `mirror_dirty` are unchanged.
- **Log privacy** on the failure paths too: serde's error text quotes an unexpected string, so no deserialization error reaches the log with its content.
- **Markdown copy:** a description with a backtick, `<`, `](` and a newline followed by `# ` leaves the day file's structure intact.
- **No jump on reveal:** vitest and the Playwright `visual` job measure the bubble before and after a reveal (a hidden photo with a description, one with none): equal.
- **Screenshots in `docs/screens/7b/`** (`scripts/screens/7b.sh`, macOS runner, AX input or the screens driver, synthetic images from `seed_demo`):
  - 01 the feed with a covered photo in a bare bubble with its pill, a covered photo with "Description hidden", and a covered album cell;
  - 02 one revealed, with its caption and "Hide";
  - 03 the dialog from a pending tile's "Details";
  - 04 the composer with a covered pending tile, its "Details" and ×;
  - 05 sent;
  - 06 the viewer with a caption;
  - 07 back in the chat, covered again;
  - 08 the dialog from an older entry's menu;
  - 09 after `kill -9`;
  - 10 search;
  - 11 en;
  - 12 large text;
  - 13 640 px;
  - 14 the trash with a covered photo.

  Texts:
  - `thumbs.txt`: a seeded, never-revealed hidden photo has no file in `thumbs/` after the feed shows, and its neighbour has one;
  - `mirror.txt`: the day file and the folder;
  - `mcp.txt`: the agent's answer for that entry;
  - `rows.txt`: the three groups and `hlc_json` of one attachment before and after an edit, never `core`, which holds `blob_key`.

  Nothing here is platform-specific, so there is no Windows scenario.

## Touches sensitive areas
**Yes.** `sync-crypto-guardian` reviewed this spec before code (the rulings are folded in) and reviews the PR before merge. It touches:
- migration 9;
- three new synced groups on `attachment` and the write paths for them;
- the generic add-group function and its amendment to §A.0;
- new bytes for every new attachment (send, voice, import);
- two IPC commands with free text;
- the new fields MCP returns.

### For the guardian
- **Groups.** `name` = `{"name":<string|null>}`, `description` = `{"description":<string|null>}`, `privacy` = `{"spoiler":<bool>}`, each `sv = 1`, JCS bytes, a public contract in §C.8. They are additive per ADR-0001 §4 rule 1 (new groups that change no other field's meaning), so there is no `sv` bump and `schema_version` stays 1. `core` is not touched.
- **Why three groups.** A rename, a description edit and a hide on different devices must not undo one another. The precedent is the chat's `privacy` group, kept apart "so that it is not lost on a concurrent rename" (§A.0).
- **Older rows and the invariant.** A row without a group shows its default. The first edit of a pre-7b row adds only the group whose value changed. No code path writes a default-valued group to an existing row. Otherwise two devices that each add all three groups would let one device's rename uncover the other's hidden photo, or its hide drop the other's rename.
- **Limits.** `name`: `null` or 1–255 UTF-8 bytes including the extension, slice 7's filename rules, the extension equal to that of `core.filename` (bytes or ASCII fold). `description`: `null` or 1–1,024 scalar values after normalization, at most 4,096 bytes. An attachment envelope stays far below 1 MiB (§5.3).
- **Encryption.** All three values are inside the encrypted payload. The open header carries only the group names and their HLCs, as for every group.
- **Conflicts.** Last writer wins per group by HLC (§3.3), with no revision, like the chat's `meta`: two concurrent edits of the same detail keep the later one, and the other is lost without a trace (PRD §10.2 says so). Equal HLC with different bytes follows §3.3.
- **Reading another client's values (slice 11).** The total read function applies. A bad `name` shows the original and a bad `description` shows none; both are kept verbatim. The spoiler follows the table under Checks: when the value is unclear, the photo stays covered.
- **An older client.** It keeps the groups raw and resends them (§4 rule 1). It shows the picture uncovered and the original name. No released client syncs before slice 11, so every syncing client knows these groups.
- **What the server sees.** It sees the attachment id, the group names, their HLCs and the padded size, but no name, description or flag value. Still:
  - it sees **every change of `privacy` and when**;
  - on a pre-7b row, the first appearance of a `privacy` group means "hidden", because nothing else adds it;
  - a description of up to 4,096 bytes can raise the padded size by up to about four 1 KB steps;
  - creating an attachment does not reveal whether details were set, except through that size.

## Product decisions (the user, 19.11.2026, asked twice with the same answers)
1. **Reveal:** by click only, never by hover.
2. **Outside the Kinige window, a hidden photo is an ordinary photo.** The Markdown copy, search and MCP treat it like any other: the copy writes the picture with its description, search finds it by its name and description, and an AI agent gets its name, description and a `spoiler` flag. The user chose this over the recommendation to keep hidden photos out of those places.
3. **Spoiler by default for a chat:** not now.

## Open questions
None.
