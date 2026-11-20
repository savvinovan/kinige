# KINIG-138: slice 5a, Reactions

**Budget:** code 6 h, review and verification 18 h (verification has been about 3× the code): about 3 working days end to end. Stop and report at 2×. Re-estimated after the guardian's pre-review: the per-emoji merge is out (slice 11 decides it), and the group reader, the key function and the rebuild of the derived rows are in. Order: the domain (key function, group reader, writer) and its reference bytes → store, migration, IPC → Storybook stories, and a story screenshot to the user before the window is built → the window, the copy, MCP → screens.
**Task:** Huly KINIG-138. **Branch:** `slice/5a-reactions`. The PR title and first commit start with `KINIG-138: …`. **Depends on:** 3 (hover actions, context menu, emoji picker), 5 ("On this day"), and **7b merged first** (KINIG-136): 5a uses 7b's generic add-group function, takes the migration number, the ADR-0001 revision and the PRD version after 7b's. **Sources:** the user's request of 18.11.2026 (a Discord hover bar: three quick emoji, a picker button, reply, forward, "more"); PRD §2.1, §4.1, §4.2, §4.3, §6, §9.3, §10.2, §10.3, §12.3, §15; ADR-0001 §3.3, §3.5, §4, §A.0, §B, §C; ADR-0007; ADR-0009; the 7b spec (add-group).

**Why "5a":** slice 5 is "On this day" and replies, the "message from a later self", and follow-ups are named `<slice>a` (4a, 7a). **The PRD has no reactions today.** It knows only the favorite (the star, the `flags` group, the favorites filter), which stays as it is. Scope below is the coordinator's decision on the user's request; the user can overrule any of it at acceptance.

## Scenario
1. In Inbox I hover today's entry «Сдал экзамен!». The action bar to the right of the bubble now starts with three emoji, ❤️ 😂 😢, and a "smile with a plus" button, then a thin divider and the usual copy, edit, task, star, "more".
2. I click ❤️. A small ❤️ chip appears under the text, inside the bubble, and the time moves to the right end of that chip row, as in Telegram. Nothing flies, nothing bounces, no toast. The bubble grows upwards: the bar stays under my pointer, and the entries above move up by one row.
3. I hover the chip: "Put today at 21:05". I click ❤️ in the bar again, and the chip goes. I put it back.
4. I click the smile button. The emoji picker of the input field opens, with my recent reactions on top. I type «огонь» and pick 🔥. A second chip appears after ❤️. On the next hover the bar shows 🔥 ❤️ 😂: my latest reactions come first.
5. The next morning the "On this day" card shows an entry from 2025. Next to its "Reply" there is now the smile button: a small popover with the three quick emoji and "More". I pick 🥲. The chip appears under that entry in the card, and in the feed when I open it. It reads "🥲 1 y"; its tooltip says "Put on November 19, 2026 · 1 year later".
6. From the keyboard: Tab into the entry's action bar, arrows to ❤️, Enter. Or Shift+F10 on the entry → "Add reaction…" → the same popover.
7. I click a chip: the reaction is removed, as in Telegram.
8. An entry holds at most 10 reactions at a time, and at most 32 different emoji over its whole life. With 10, or after 32 different ones, the buttons that would add a new emoji are disabled and say why; the emoji the entry has had can still be put and removed.
9. I `kill -9` the app and start it: the reactions and their times are in place.
10. Search for «экзамен» shows the entry with its chips; search does not look inside reactions. The trash shows them but does not let me change them. Restoring the entry brings them back.
11. `~/Documents/Kinige/Inbox/2026/2026-11-18.md` has, under the entry's text, the line `❤️ 2026-11-18 · 🔥 2026-11-18`.
12. An AI agent that reads the entry through MCP sees its reactions with their times. It cannot put one.

## Included
- **Data (`kinige-domain`).** A new field group `reactions` on the message, its key function and its reader (the three promises in "For the guardian"). An entry carries several different emoji, each at most once, each with the time and time zone it was put (PRD §4.1). Writing a reaction is not an edit: no revision, `edited_at` unchanged, the favorite untouched. **No merge function:** two versions of the group meet only in slice 11, which chooses the merge rule (below).
- **Store (`kinige-store`).**
  - `set_message_reaction(id, emoji, on)` in one writer transaction. Input rules of the store: a UUID v4; the entry exists (`message_not_found`); not in the trash (`message_in_trash`); not read-only (`read_only`); the stored group is well-formed (`corrupt`, as §C.7 does); a valid emoji (new `reaction_invalid`); a put is refused with new `reaction_limit` when the entry has 10 live reactions, or when it would add a 33rd different key. `on = true` for an emoji already live writes nothing, so its time is kept; `on = false` for an absent emoji writes nothing. Then the new group, the derived rows, `mirror_dirty` for the entry's day and the recent list. Returns the updated `MessageView`, like `set_message_flag`.
  - **The first reaction adds the group through 7b's generic add-group function** (its allowlist gains `message` {`reactions`}; the new group's HLC is above every HLC of the record). Later writes use `rewrite_group`. 5a writes no add-group code of its own.
  - **Migration** (the number after 7b's): a local derived table `message_reaction(message_id, key, emoji, put_at, put_tz)` with an index on `message_id`. **One function builds its rows from the group bytes**, used by the write, by purge, by `after_migration`, and later by pull. A rules version in `app_setting` (like `search_index_version`) rebuilds all rows on open when it differs. Nothing synced changes shape.
  - `MessageView.reactions: [{emoji, putAt, putTz, yearsLater}]`, ordered by the time put, filled for the feed page, the "On this day" card, search results and the trash by one join on the page's ids. `yearsLater` is slice 5's rule: full years between the entry's `journal_date` and the reaction's journal date in its `tz` (04:00 boundary, chrono), clamped at 0.
  - **Quick set:** `app_setting.recent_reactions`, up to 8, broken value → empty. **Local on purpose:** not synced and not in `settings.json`, so each device's bar follows what was used on it. `Settings.quickReactions` is three emoji: the recent ones, then the fixed set ❤️ 😂 😢 without repeats (by key). Removing a reaction does not touch the list.
- **Markdown copy.** Under the entry's text and attachments, one line: each live reaction as `<emoji> <YYYY-MM-DD>` (its journal date in its `tz`), in the order put, joined by ` · `. No line without reactions. The format does not depend on the UI language.
- **MCP read (ADR-0009).** `AgentEntry.reactions: [{"emoji","put"}]`, `put` in the form of `created` (local time in the reaction's `tz`, with offset); the field is absent when empty. No write tool (open question 1).
- **IPC (§12.3):** `set_message_reaction` for `main` only; `capture` keeps exactly seven commands; `settings` is denied. MockApi and `fixtures/api-contract/` repeat the input rules, the caps and the key reference vectors.
- **UI (through `design-system`, tokens only; a new role first in `design/tokens/`):**
  - the hover bar: three quick emoji buttons (pressed when the emoji is live on the entry, `aria-pressed`, a click toggles), the "Add reaction" button (Lucide `smile-plus`, a line in `icons/README.md`) that opens the picker, a divider, then the existing buttons; one toolbar, one Tab stop, as today. **The order of the quick emoji is taken when the bar appears and stays while the pointer is on that entry**, so a click never moves the next emoji under the pointer;
  - `ReactionRow` and `ReactionChip`: inside the bubble under the text (under the album or the voice player for an entry without text), left-aligned, wrapping within the bubble's ~65 characters; the time sits at the right end of the row. The chip is a button: the emoji, plus a tertiary "N y" when `yearsLater ≥ 1`; tooltip and screen reader label "❤️, put on November 18, 2027, 1 year later. Remove";
  - `ReactionPopover` (three quick emoji and "More", which opens the picker) for the "On this day" card and the context menu item "Add reaction…";
  - the picker in reaction mode shows the recent reactions in its "Recent" section; a pick adds and closes the picker;
  - read-only places (trash, search rows, a read-only entry): chips without the button role.
- **No jump.** The feed is `column-reverse` (`Feed.tsx`), so with the scroll offset kept, a growing row keeps its bottom edge and grows upwards: the bubble's bottom line, and with it the hover bar, stays under the pointer, and the entries above move up. The slice keeps the offset unchanged on a reaction write and proves the bar does not move (Checks). In a narrow window the bar sits over the bubble's top corner and moves up with it: accepted, the narrow window is second-class.
- **Documents:**
  - PRD v0.29 (7b took v0.27, KINIG-153 took v0.28): §2.1; §4.2 (the derived table); §4.3 (the copy line); §6 (chips, bar, menu, the limits of step 8 and their cost); §9.3; §10.2 (the `reactions` group; the merge is chosen in slice 11); §10.3 (what the server sees, below); §12.3; §15 (row 5a); history; the recent list is local-only on purpose.
  - ADR-0001 revision 13 (7b takes 12): §A.0 (the group; `message` {`reactions`} on 7b's add-group allowlist), a new §C.9 (the bytes, the reader, the key function, the caps, the reference vectors), and the open item for slice 11 in §B (below).
  - `CLAUDE.md` status in the first commit.

## Not included
- **The per-emoji merge.** 5a ships the stored shape and its promises; no merge function, no merge write, no merge tests. Slice 11 chooses between the per-emoji merge and plain group last-writer-wins; the stored rows allow either. **What plain last-writer-wins costs the user:** when two devices react to the same entry before they sync, one device's reactions are lost, as with the star today.
- Sticker and GIF reactions (the stickers slice, P1, not specified yet), custom emoji. Nothing here blocks them: a later kind is a new optional field of an item, kept by the reader as an unknown field.
- Filtering or searching by reaction, mood statistics, reactions in the forest (day weight, card, "undergrowth"), reactions on chats.
- Reactions in the reply quote, in the capture window's context, in the forest card.
- A double-click reaction, a burst animation, a toast, an undo.
- Turning ❤️ into the favorite or the favorite into a reaction: they stay separate.
- An agent putting reactions (open question 1). Importing reactions from the Telegram export (open question 3).

## Reference (UI work)
**Telegram desktop.** Reactions sit inside the bubble under the text, as rounded pills of emoji and count, your own pill tinted with the accent; the time moves to the right end of that row. Hovering a message shows a small reaction button; it opens a strip of quick reactions with an arrow to the full set. A pick plays an animated burst. A click on your pill removes it; a double click puts the default reaction.
- **Copied:** chips inside the bubble under the text, the time at the end of their row; a click on a chip removes it; the full set from the same picker.
- **Changed:** no count (one author) and no accent tint (every chip is yours: a quiet neutral fill); no burst and no animated emoji; no double-click reaction; the quick emoji live in our existing hover bar, as in the user's Discord screenshot, not in a popup of their own; the quiet "N y" age on a late chip.

**Discord (the user's screenshot).** Three quick emoji first in the hover bar, then the picker, then the other actions: copied. Its reactions as a separate row under the message with counts: not copied, the in-bubble row keeps an entry one unit.

Per the `screens` skill, the spec's side-by-side is this text; the story screenshot of the bar and the chips goes to the user before the window is built.

## Decisions left to the implementer
Ruled by `sync-crypto-guardian` in the pre-review: a group on the message, not a record per emoji; the three promises; `tz`; the derived table with one builder and a rules version; the trash refusal as a store input rule only; reuse of 7b's add-group; no merge in 5a. Left to the implementer and listed in the PR: the command name, the component names, the recent list size (8), the tooltip component (the one `IconButton` uses), the chip's token roles (with `design-system`), the name and value of the rules-version key.

## What "done" looks like
A 1280 × 800 window, the feed column ~720 px.
- **An entry without reactions** looks exactly as today: no empty row, the existing visual baselines unchanged.
- **The bar:** three emoji buttons at the size of the small `IconButton`, the smile-plus, a 1 px divider, then copy, edit, task, star, more. A pressed emoji has a quiet fill.
- **The limits and their cost.** With 10 live reactions, the buttons that would add are `aria-disabled` with "At most 10 reactions on an entry". After 32 different emoji ever used on one entry, a 33rd new one cannot be put on it, even if most of those were removed, with "This entry has had 32 different reactions"; the 32 it has had still toggle. A group over the caps that came from elsewhere is shown in full, refuses a put and allows a removal.
- **The chip row:** chips about 20 px high, fully rounded, a 4 px gap, the emoji at the body size; the "N y" label in the tertiary color; the time at the row's right end. Ten chips wrap inside the bubble.
- **States:** one chip; ten; a late chip; a long text; a photo album; a voice note; a read-only entry; the trash; a search row; the "On this day" card with the popover open; a group that is not well-formed (no chips, no error in the feed).
- **Storybook** (en and ru, normal and large text): the bar (none, one pressed, ten, after 32), `ReactionRow` in each state above, `ReactionPopover`. A play test in the Playwright `visual` job measures the bar before and after a click.
- **Screenshots in `docs/screens/5a/`** (`scripts/screens/5a.sh`, `DRIVER=1`, the macOS runner). `seed_demo` with `KINIGE_SEED_REACTIONS` puts synthetic reactions on today's entries and on past-year entries, one put a year after its entry, with dates from the run's `journal_date` (chrono). Shots: 01 the hover bar; 02 after ❤️; 03 the picker; 04 several chips with a late one and its tooltip; 05 the card popover and its chip; 06 after `kill -9`; 07 search; 08 the trash; 09 "Add reaction…" in the context menu; 10 ten reactions; 11 en; 12 large text; 13 at 640 px. Texts: `mirror.txt` (a synthetic day file), `bench.txt`, `cold-start.txt`.

## Checks
| Scenario step / rule                  | Test or evidence that proves it                                                         |
|---------------------------------------|-----------------------------------------------------------------------------------------|
| Steps 1-2: bar, chip, no toast        | vitest on MockApi (MessageActions, Message, ReactionRow); shots 01, 02                  |
| Step 2: no jump under the pointer     | Storybook play test in the Playwright `visual` job: the bar's box moves under 1 px      |
| Steps 3, 7: remove by bar and by chip | vitest; store test: removal marker (`at` and `tz` null, new `h`), derived row gone      |
| Step 4: picker and recent order       | vitest; store tests of `recent_reactions` (order, cap 8, broken stored value, defaults) |
| Step 5: card popover, late chip       | vitest of `OnThisDayCard`; domain test of `yearsLater` with `tz`; shot 05               |
| Step 6: keyboard and context menu     | vitest: toolbar arrows and Enter; Shift+F10, "Add reaction...", popover focus           |
| Step 8: 10 live, 32 ever              | store: the 11th live and the 33rd emoji refused, nothing dropped; vitest of disabled    |
| Step 8: a group over the caps         | store: a well-formed group over the caps is shown, refuses a put, allows a removal      |
| Step 9: survives kill -9              | store test: injected fault before commit leaves no half state; shot 06 after `kill -9`  |
| Step 10: search, trash, restore       | store tests: index unchanged, `message_in_trash`, restore keeps the group; shots 07, 08 |
| Step 11: Markdown copy                | domain test of the line (order, dates in `tz`, empty); `mirror.txt`                     |
| Step 12: MCP                          | `agent_read` tests: field present, absent when empty, hidden chats stay hidden          |
| Promise 2: the group reader           | domain: each well-formedness rule, garbage, 1 MiB group, unknown fields kept            |
| Promise 2: hostile items              | store: bad `e`, `at`, `tz` in `groups_raw`: nothing in view, copy, MCP, log; no panic   |
| Promise 3: the key function           | reference vectors in the domain and `fixtures/api-contract/`; comparisons by key only   |
| Untouched items                       | store: a put or removal leaves every other item byte-identical                          |
| First reaction's HLC                  | store: above every HLC of the record (7b's add-group); other groups byte-identical      |
| Derived rows                          | store: rebuilt rows equal incremental rows; a rules-version mismatch rebuilds on open   |
| Garbage IPC input (rule 12)           | IPC tests: empty, text, two emoji, lone VS16 or ZWJ, controls, 1 MiB, bad id            |
| Older client                          | store test: a `flags` rewrite keeps `reactions` bytes verbatim; reference vector        |
| Purge                                 | store test: `purged_of` lists `reactions`; derived rows deleted with the entry          |
| Log privacy                           | `log_privacy`: no reaction emoji in the log, error texts included                       |
| IPC matrix                            | `src-tauri/tests/ipc_slice5a.rs`: main only; capture keeps seven; settings denied       |
| Migration                             | pinned hash after 7b's; previous version to new keeps rows; fresh database              |
| Speed                                 | Inbox on synthetic 100k, reactions on 10 % of entries: open p95 < 300 ms (`bench.txt`)  |
| The branch                            | `scripts/pre-pr-check.py`; CI green on every job                                        |

## For the guardian
**Shape.** Group `reactions`, `sv = 1`, canonical JCS: `{"items":[{"at":<ms>|null,"e":"<emoji>","h":"<HLC>","tz":"<IANA zone>"|null}, …]}`. Items sorted by key, one per key. `at` is the wall time it was put and `tz` the device's IANA zone then (PRD §4.1); both are `null` on a removal marker. `h` is the HLC of the item's last change: a write gives the group a new HLC strictly greater than before, and the changed item gets it as `h`. `at` is content, like `created_at`: a wrong clock gives a wrong date until the reaction is put again; `yearsLater` clamps at 0. A `tz` is invalid when it is not a string, is empty, is over 64 bytes or holds a control character; a valid name this device does not know reads as UTC.

**Three promises, constants of `sv = 1`; changing any of them is an `sv` bump.**
1. **A removal marker is never dropped.** The caps are input rules only: a put is refused with `reaction_limit` when the entry has 10 live items (shown or not), or when it would add a 33rd key. Toggling the keys it has had still works. A stored group over the caps (from a peer, later) is shown in full, refuses a put and allows a removal. Nothing trims a group.
2. **Only well-formed groups are written, and one pinned reader decides what is well-formed:** canonical bytes; at most 8 KiB; `items` is an array of objects; `e` is a string; one item per key; `h` is a strict HLC not above the HLC of the group that carries it; `at` is `null` or an integer in `0..=9_999_999_999_999`; unknown fields are allowed and kept. A group that is not well-formed shows nothing, and a local write on it is refused as a corrupt group (as §C.7 does). An item whose `e` is not an emoji (`is_emoji_sequence`, at most `MAX_ICON_BYTES`), or whose `at` or `tz` is invalid, never reaches the view, the Markdown copy, MCP or the log. A put replaces an item with the same key whose `e` is invalid.
3. **The key is its own pinned function with reference vectors:** `e` without U+FE0F, compared in UTF-8 byte order. It is not the picker's `emoji_identity`, which has changed before. Items are compared by key, never by position or raw bytes. So ❤ and ❤️ are one reaction, and `e` keeps the form the picker gave.

**Writes.** A write changes one item and leaves every other item byte-identical. The first reaction adds the group through 7b's add-group function. The store refuses a write to an entry in the trash; that is an input rule only, because a pull must later accept a peer's change to a trashed entry.

**Encryption and the server.** The group lives in the encrypted payload (ADR-0001 §5.3) like every group; the envelope format and keys do not change. The open header names the group and its HLC, so the server learns that an entry's reactions changed and when, the same class as `flags`. Padding does not hide this. This line goes into PRD §10.3.

**Trash and purge.** Restore brings reactions back. Purge removes the group with the record and its derived rows; `purged_of` covers it, and a reaction written on another device during the purge window is a newer group, so the record goes to quarantine under the existing rule. No cascade: reactions are not records.

**Older clients.** No released app syncs before slice 11. In general (§4 rule 1) an older client keeps the unknown group raw in `groups_raw`, merges it by HLC, resends it verbatim, and does not mark the record read-only (`sv = 1`). An older app opening a database migrated here refuses it with `too_new`, as for every migration.

**Open item for slice 11 (ADR-0001 §B):** the merge of `reactions` is plain group last-writer-wins or the per-emoji merge. If the per-emoji merge is chosen, its obligations are the guardian's findings:
- the merge write is made once, at the end of a cycle, after a pull that reached the head;
- none for a record in the trash, in quarantine, under a tombstone, or read-only;
- it changes no item's bytes and no item's `h`;
- a merge write that cannot be made falls back to the winner;
- it is restamped on leaving `clock_skew`;
- reactions are added to the S4a simulation with properties 1, 5, 6 and "a removed reaction stays removed";
- the compaction note: a version that lost the group contest can be compacted away before every device has merged it.

**The migration and 7b.** A new local table only; no row of a synced table changes. Independent of 7b's migration in content; 7b lands first, and 5a takes the next number and appends its hash to `PINNED` after 7b's.

## Touches sensitive areas
Yes. **Data schema:** a migration with a derived local table and its rules version. **Sync:** a new synced group on the message, its reader, key function and caps as constants of `sv = 1` (ADR-0001 §A.0, §C.9), and the open item in §B. **What the server stores:** the server learns when an entry's reactions change (PRD §10.3). **IPC:** one new main-only command. **MCP:** a new read field. Not touched: the envelope bytes, keys, the server code. `sync-crypto-guardian` reviewed this spec before code (the rulings above) and reviews the PR before merge.

## Open questions
1. **Can an AI agent put reactions on your entries?** If yes, an agent could, for example, mark the entries it summarized. Recommended: no, not in this slice. The agent only sees reactions, and the feed never shows a reaction you did not put yourself.
2. **What a click on a reaction does.** As in Telegram, it removes the reaction, and the date it was put goes with it: putting it back gives today's date. The other option: a click only shows when it was put, and removing is done from the hover bar. Recommended: as in Telegram, removal is rare and deliberate.
3. **Reactions from the Telegram import.** Your Telegram export may hold the reactions you put on your own messages (not verified on a real export). Recommended: not in this slice; a small follow-up after it, if you want them.

**Answered by the user on 19.11.2026:** 1: no, an agent only sees reactions. 2: a click on a chip removes the reaction. 3: later, if needed.

## After 7b's review (20.11.2026)
Notes of `sync-crypto-guardian` from the review of PR #75, to settle in this slice and list in the PR under "Decisions for the guardian":
- The "rewrite or add" choice (the group is present → `rewrite_group`; absent → add-group) lives in the store in 7b. Move it into the domain in this slice's first code commit, so 5a and 7b share one dispatch.
- 7b's `rewrite_field` allowlist is by group name, not by entity. `message` {`reactions`} goes on the add-group allowlist by entity and group; don't widen `rewrite_field` for it.
- 7b's whole-record refusal (the two maps name different groups → `Corrupt`) covers the add path, so the first reaction inherits it. A test shows it for a message.
