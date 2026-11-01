# Slice 5: “On this day” in Inbox and the “N years later” reply

**Budget:** ~3.5 h for the code, ~7 h until ready for acceptance. If the slice is not ready for acceptance after 14 h, stop and report (§12.1a). **Depends on:** 4 (the import filled the past years), 4b (accepted 31.10.2026). Huly: **KINIG-12**; branch `slice/5-on-this-day`, the PR and the first commit are `KINIG-12: …`. **Sources:** PRD §1.1, §2.1, §4.1, §4.2, §5, §6, §8, §11.3, §12.2, §12.3, §15 (line 5, non-functional targets); ADR-0001 (the `core` group, appendix §C), ADR-0009; user decision 1 of slice 4 (“Saved Messages” goes to Inbox, “‘On this day’ looks there too”).

## Scenario
1. In the morning I open Inbox. Under the “Today” label, before today's entries, there is an “On this day” card. In it, by year, from newest to oldest: “1 year ago · October 31, 2025”, “3 years ago · October 31, 2023”. Under each year, my Inbox entries from that day. The feed does not jump: the card is there from the first frame.
2. I click an entry in the card. The feed opens on it (like a jump from search), and the entry is highlighted with a fill. I scroll back down to today's entries.
3. I hover over the 2023 entry and press “Reply”. Above the input field appears the line “Replying to the entry of October 31, 2023” and the first line of that entry, with a cross on the right. The cursor is in the field.
4. I type «До сих пор так думаю» (“I still think so”) and press Enter. The entry appears in the feed. At the top of its bubble is a quote: “3 years later · October 31, 2023” and the first line of the old entry. A click on the quote opens the old entry in context.
5. I start a reply, kill the process (`kill -9`) and launch again. The text and the line “Replying to the entry of…” are in place. A sent reply is in place together with its quote.
6. I press the card's cross. The card disappears until the next journal day: a restart does not bring it back. After 04:00 of the next day, the card for the new day appears the next time Inbox is opened.
7. If there are no entries on this day in past years, there is no card. Other chats never have the card.
8. I delete the old entry to the trash. The quote in the reply shows “Entry in the trash”. After deleting it forever: “The original entry was deleted”. The reply itself stays.
9. I open a personal chat imported from Telegram. Messages that were replies in Telegram have the same quote at the top of the bubble: “Reply to the entry of March 14, 2023” and the start of the original message. There is no “N years later” label, because the difference is less than a year. A click on the quote opens the original message in context.

## Included
- **Selection (Rust, `kinige-domain` + `kinige-store`).** Today is the `journal_date` with the 04:00 boundary (`DAY_BOUNDARY_MINUTES`). The card takes Inbox entries that are not in the trash, with the same month and day and a year less than the current one. February 29 matches only February 29, as in `on_this_day` in MCP: the domain function is shared. The query lists the `YYYY-MM-DD` dates of past years (from the year of the oldest Inbox entry) and goes through the `message_feed` index. `substr` over the whole table is not used.
- **Card contents** (user decision 3; adjust at acceptance on the real Inbox of 2016–2026 if needed). The three newest years are shown in detail, up to two entries per year (text up to three lines, time). Below them, “N more entries on this day ›”; the link opens the day in the feed. The other years are one line each: “October 31, 2019 · 5 entries ›”. The “N years ago” label uses i18next plural forms (ru: 1 год, 2 года, 5 лет, 11 лет, 21 год).
- **Hiding:** `app_setting.on_this_day_dismissed = <journal_date>`, local, not synced. The key is not written to `settings.json`: it is not a setting. While the window is open, the day change is checked when Inbox is opened and when the window gets focus.
- **Reply.** `send_message` gets an optional `reply_to_id`. Rust checks: a UUID v4, the entry exists, it is not in the trash. Otherwise the error `reply_target_not_found`, nothing written, the text stays in the field with an error above it (§5). `reply_to_id` is written into the `core` group, as for the import: the bytes `{"created_at","created_tz","origin":"user","reply_to_id":"<id>"}`. The group is immutable; the link is neither edited nor removed. The reply goes to the current chat, that is, Inbox. Only the card's entries have “Reply”.
- **Showing a reply**: for any entry with `reply_to_id`: new replies and those imported from Telegram (user decision 1). `MessageView` gets `reply_to: { id, chat_id, journal_date, excerpt, state: live | trash | missing, years_later }`. Rust builds it in the same feed page query through a `LEFT JOIN` by id, without an FK. `years_later` is the number of full years between the `journal_date` of the old entry and of the reply (`chrono`). When it is 0, the “N years later” label is not shown, leaving “Reply to the entry of · date”. `excerpt` is the plain text of the entry, line breaks replaced with spaces, cut to 120 characters. For someone else's message from the import, the first line is the author's name in bold, so the quote gets the name and the start of the text.
- **A draft with a reply.** Migration 6: `ALTER TABLE draft ADD COLUMN reply_to_id TEXT`. The table is local; the synced tables do not change. `get_draft` and `save_draft` carry `reply_to_id`. The draft is deleted only when both the text and the reply are empty. Sending clears the draft in the same transaction, as now.
- **Markdown copy.** Under the reply's `## HH:MM` heading, the line `↩ [YYYY-MM-DD HH:MM](<relative path to the day file of the old entry>)` is placed. The format does not depend on the UI language. If the old entry does not exist, the line stays without a link. The reply's file is recomputed only when its own day is edited: the link can go stale after the old entry is moved, and this is accepted.
- **IPC (§12.3):** `main` only: `get_on_this_day`, `dismiss_on_this_day(date)`, the extended `send_message` and drafts. The capture window still has exactly seven commands. The `settings` window is denied the new commands.
- **Components** (through `design-system`, tokens only; a new color first appears in `design/tokens/`):
  - the “On this day” card: fill `surface.card` (a new palette role, the same value as the bubble: “paper” is the background of the feed itself, and a card with the same fill would dissolve), no border and no shadow;
  - the year row;
  - a compact entry with the “Reply” action (the `Reply` icon from Lucide, ISC, already in `icons/README.md`);
  - the “Replying to the entry” line above the input field;
  - the quote in the bubble: a fill, no border.
- **Documents:**
  - `CLAUDE.md` in the first commit: in “State”, “slice 4b (settings window, settings.json) accepted 31.10.2026”;
  - PRD v0.13: §4.2, the `reply_to_id` comment (“slice 5”), §11.3, components, §12.3, matrix, version history;
  - ADR-0001 appendix §C.5: migration 6, the `core` bytes of a user reply, the `missing`/`trash` states.

## Not included
- “Reply” on any entry in the context menu and on hover (§6, P1).
- A back-mark on the old entry (“↪ reply October 31, 2026”, jumping to the replies) and a reply thread. Per user decision 2, we come back to this together with the forest “undergrowth” (P1).
- Entries from other chats in the card. Replies from the capture window and from an agent.
- MCP changes: `reply_to_id` is still neither given to nor accepted from an agent (ADR-0009, `agent_reply.rs`).
- Forest “undergrowth” from replies (P1), “Random entry” (P1), notifications about “On this day”, a day boundary setting and “Southern hemisphere”.
- Jump to date and feed virtualization (slice 8). Full export (§16).

## What “done” looks like
A 1280 × 800 window, a feed column of ~720 px.
- **The card** sits in the feed flow at the start of today. If there are no entries today, at the bottom of the feed, above the input field. At the top, a small secondary header “On this day” and a cross (`IconButton`, label “Hide until tomorrow”).
- **Year rows** in the tertiary color. A long entry is truncated with an ellipsis on the third line.
- **No motion:** no appearance animation, no toasts, no outline on the selected item. The focus ring only after a key press (`useInputModality`).
- **Keyboard:** Tab goes through the card's entries, “Reply”, “N more” and the cross; Enter opens an entry. Screen reader labels: “Reply to the entry of October 31, 2023”, “3 years later, reply to the entry of October 31, 2023”.
- **The quote** is one line, as wide as the bubble (no wider than ~65 characters).
- **Storybook stories** (en and ru, normal and large text):
  - card: one year; three years and the rows of older years; long text;
  - reply line: normal; entry deleted;
  - quote: `live`, `trash`, `missing`, `years_later = 0`, an imported reply to someone else's message (the author's name in the quote).
- Narrow window: everything is reachable, compromises are acceptable.

## Checks
- **Rust (domain):**
  - past-year dates: February 29, year boundaries, the year of the oldest entry;
  - `years_later`: leap dates, a difference of less than a year, a reply earlier than the original (date change) gives 0;
  - `excerpt`: Markdown, empty lines, long lines, emoji, the author's bold name as the first line;
  - the `core` bytes of a reply match the reference.
- **Rust (SQLite):**
  - selection: Inbox only; the trash, the current year and other chats are excluded; the card's order and limits, the “N more” counter;
  - garbage `reply_to_id`: not a UUID, an empty string, 1 MiB, an entry of another chat, an entry in the trash. Error, nothing written;
  - `reply_to_id` survives an edit, a move, a date change and an append;
  - deleting the old entry forever gives `missing`;
  - importing a personal chat with replies: a reply has `reply_to` with `state = live` and `years_later = 0`, and a reply to a message that was not imported (`reply_to_id = null`) has no quote;
  - migration 5 → 6 keeps the drafts;
  - `abort` with a reply draft: the draft is in place;
  - hiding survives a restart;
  - the `↩` line in the Markdown copy;
  - measurement: 100k entries over 5 years (`import_bench synthetic 100000`), `get_on_this_day` p95 and opening Inbox < 300 ms.
- **IPC (mock):** the new commands are allowed for `main` and denied for `capture` and `settings`; `capture_send` does not accept `reply_to_id`.
- **Frontend on `MockApi`:**
  - the card and its absence; the cross; the day change on focus;
  - the whole reply: the line, cancel, the `reply_target_not_found` error;
  - a click on a quote (including in an imported chat) and on a card entry opens the entry in context;
  - keyboard and labels; en and ru keys match, Russian plural forms;
  - visual baselines through `visual-update.yml`.
- **Screenshots in `docs/screens/5/`:** `scripts/screens/5.sh` on the macOS runner (`gh workflow run screens.yml --ref slice/5-on-this-day -f slice=5`), input through AX only.
  - Data: `seed_demo` is extended with Inbox entries on today's `journal_date` 1, 3, 4, 6 and 9 years ago (several entries each, one long, one in the trash). The dates are computed with `chrono` from the `journal_date` of the run, not from UTC. The scenario puts the reply draft in place through `sql`, because there is no keyboard input. `seed_demo` also imports a synthetic personal Telegram chat (the JSON is built in code, as in `import_bench`, and goes through `parse_export` and the store import). The chat has own and other people's messages and several replies, including a reply to someone else's message.
  - Screenshots: 01 Inbox with the card; 02 after “Reply” (the line above the field); 03 the reply draft after a restart; 04 after “Send”: the reply with a quote; 05 the old entry opened from the quote; 06 the quote “Entry in the trash”; 07 after the cross; 08 after `kill -9` (no card, the reply with the quote is in place); 09 the chat «Книги» (“Books”) without the card; 10 en; 11 large text; 12 the imported personal chat with quotes; 13 the original message opened from a quote in this chat.
  - Texts: `mirror-reply.md` (the day file with a reply from a synthetic copy); `cold-start.txt`: `scripts/measure-cold-start.sh 10` with `KINIGE_DATA_DIR` on a synthetic 100k (target < 1.5 s, §15); the measurement of `get_on_this_day` and of opening Inbox goes into `bench.txt` (`import_bench synthetic 100000`).

## Touches sensitive areas
**Yes, `sync-crypto-guardian` before merge:**
- the first non-null `reply_to_id` in the `core` of a user entry, and its check;
- migration 6 (the local `draft` table);
- `reply_to` without an FK: a link to an entry that has not arrived yet during pull, was deleted forever or lives in an `ai_excluded` chat;
- new IPC commands.

The protocol, the envelope, HLC and encryption do not change: `reply_to_id` is already in the `core` group (ADR-0001, appendix), and the server does not see it. MCP does not change. No new crates or npm packages.

## User decisions (31.10.2026)
1. Quotes on replies imported from Telegram are shown the same way as on new replies. If the difference is less than a year, there is no “N years later” label.
2. There is no back-mark on the old entry in slice 5. We come back to it together with the “undergrowth” (P1).
3. The three newest years are shown in detail, the rest as a line. Adjust at acceptance if needed.

## Open questions
None.
