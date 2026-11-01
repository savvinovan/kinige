# ADR-0004: Design system foundation (D1)

Date: 2026-10-20. Status: **accepted** on 20.10.2026. PRD v0.7 (§2, §5, §6, §11.2, §15) was brought in line.

## Context
Slice D1 (`docs/slices/D1-design-foundation.md`): dark theme tokens, fonts, slice 1 components, and choosing a feed variant from the Codex concepts. The user looked at the first round of concepts (`design/concepts/feed/`, three variants in "warm dark paper" per §11.2 v0.6) and rejected the direction: brown "like old furniture", fonts too large, the chat name and the dates duplicated.

## Decisions

**Palette — neutral graphite with a light-blue accent** (decision of 20.10.2026, based on the dark themes of ChatGPT and Claude, which the user likes). Surfaces are gray without a color tint; the nord8 accent `#88c0d0` and the error `#d89fa4` stay; the contrast targets are the same (below). The blue-graphite Nord described below was an intermediate step: it was replaced by graphite after the live window.

*History:* **Palette — Nord** (the file `nord.json` was replaced by `design/tokens/palettes/graphite.json`; sources and adjustments — `design/tokens/palettes/README.md`).
- The "warm dark paper" principle (§11.2 and §2.1 v0.6) is canceled. From the official Nord, what remained is the tint, the accent (nord8) and the error (an adjusted nord11, `#d89fa4`); the surfaces and text are adjusted (below, "Revision"), details are in `palettes/README.md`.
- Fallbacks if Nord does not work in the live window: dark blue as in Telegram, or neutral graphite. There are no such palettes right now; they are made only on request.
- **Revision based on the live window (20.10.2026): darker and more contrast.** The user felt that the background was too light and the text gray. By the numbers, the body text on a bubble was 8.7:1, but the bubble `#3b4252` is light. Decision:
  - the background and bubbles are noticeably darker, the Nord tint is kept, and the official Nord values for surfaces no longer match;
  - the text is almost white: body text on a bubble ≥ 12:1, time and error on a bubble ≥ 5:1 (previously the margin was in hundredths);
  - exact values are tuned in the tokens, and deviations are noted in `palettes/README.md`.

**Fonts — Inter** (entry text and the UI) + **JetBrains Mono** (code), woff2 under the OFL in `web/src/ui/fonts/`.
- Literata (chosen earlier for a 17 px editorial layout) is canceled based on the live window: at 14 px the serifs are noisy and reading is harder. The reference is Telegram, Notion, Zed — all sans-serif.
- The Literata files are deleted.

**Density as in Telegram.** "Compact" is the default (§6).
- Body text is 13 px (below, "Text size") instead of the previously approved 17 px / 1.65. Line height is recalculated for the new size; exact values go into the `type.json` tokens during implementation and are approved from a screenshot.
- Time and day labels are small (~12 px).
- Still in force: the large-text multiplier ×1.25 and the separator visibility threshold of 1.2:1.

**No duplicates** (§6):
- above the feed there is neither a heading nor a line with the chat name: the selected chat is highlighted in the sidebar. The line will come back only if something useful appears in it (for example, a chat actions menu);
- a day has one label ("Today", "Yesterday", "March 14", "March 14, 2025"), without a second line with the same date.

**Input field with buttons** "file", "emoji", "microphone", as in Telegram (§5). In D1 the buttons are only drawn. In the app, each one appears in the slice where its function works:
- emoji picker — slice 3;
- attachments — slice 7;
- voice notes with recording — a new slice 7a, right after attachments: moved from P2 to the MVP.
- The field is a dark fill without an outline (based on the live window: an outline with a focus ring looked like a double light-blue line). In focus — one thin line inside the field in the focus color: a fill change alone (1.15:1) and a blinking cursor are not enough for focus visibility (WCAG 2.4.7, D1 review). The same for search.
- Stickers (custom images, import from Telegram) and GIFs — P1; GIFs moved from P2 to P1. External GIF search reveals queries to a third party and is turned on only by an explicit setting (§2.3).

**Chat emoji.**
- Each chat has its own emoji; the user chooses it in slice 3. Stored in `chats.icon`; the schema does not change.
- The sidebar by default is emoji + name; it collapses to a strip of emoji, and the name is visible on hover.
- Emoji as UI decoration are still not used; icons are line icons.
- **Entry counter** on the right of a chat row, the user's idea (like Telegram's unread counter, but for read ones):
  - all of the chat's entries are counted, except the trash;
  - the color is a quiet gray, not the accent: it does not mean "unread, deal with it" (§1.2 item 7);
  - from 10,000 it is abbreviated: «12 тыс.» ("12K");
  - in D1 it is only the look on demo data; real numbers appear in slices 1 (Inbox) and 3 (chats).

**Profile row** at the bottom of the sidebar, as in Notion: an avatar with initials + a name (by default "Me") ▾, a menu "Settings", "Export", "About". In D1 it is only the look; settings — slice 1 (Markdown copy folder, text size), "Account and sync" and the status dot on the avatar — slice 11.

**The feed is a centered column**, ~720 px, a bubble no wider than ~65 characters (was ~80), the input field the width of the column; in a narrow window — full width. This is how Telegram holds its message column in a wide window; a comfortable line length is 45–75 characters.

**From ChatGPT and Claude** (the user's choice, 20.10.2026):
- when the field has text, the "microphone" is replaced by a round accent "Send" button with an arrow; Enter still sends;
- actions on hover or focus: copy, edit, task, favorite, "more"; in D1 only "Copy" works, the rest from slice 3. The row sits **to the right of the bubble** at the level of its bottom line, and in a narrow window slightly above the bubble at its right edge (on the top border, as in Slack): the bubble's text is not covered, and the mouse is usually near the top of the message anyway. The "below the bubble" option covered the text of the next message, and reserving space below each one would eat into the density;
- sidebar sections with small gray headings ("Pinned", "Chats", "Archive") — slice 3, together with pinning and archiving;
- a "pill" input field floating above the bottom of the window was not taken.

**There are no tabs.** One chat is open; an entry is saved immediately, each chat has a draft, and switching is a click in the sidebar.

**Messages are bubbles, each with its time** (from the second round of concepts, 20.10.2026):
- In a continuous stream without separation, it is unclear that the entries were written at different times and in different contexts. So each message is a bubble with a light fill and rounded corners, without a tail or an avatar, all aligned left. The "no bubbles" principle (§1.2 item 2) is softened to "no tails, no avatars, and no split into mine / theirs".
- The time is small, right after the text, on the last line, inside the bubble, as in Telegram. Not at the right edge: the text comes first.
- Merging messages with a pause < 5 min into a "page" is canceled. Such messages stand closer to each other, but each has its own time.
- The day label is a small "pill" in the center.

**Feed variant — compact Conversational** (`design/concepts/feed-v2/conversational-compact-2.png`), approved by the user on 20.10.2026.
- The second round included compact Editorial and compact Conversational; the user did not take Immersive.
- About the first Conversational: the day pill was liked, the time at the right edge and the continuous stream were not; hence the bubble decisions above.
- The concept is a reference for the layout, not the sizes. The generator does not hold sizes, and the 1586 px image depicts a 1280 px window, so in Preview everything looks larger.

**Text size — 13 px** (the user's choice on 20.10.2026 on a live screen in Storybook out of 13 / 14 / 15; 14 came second). The root 1rem = 13 px, and all density (UI, time, spacing, icons) is proportional to it — exactly as in the "13" candidate in Storybook. Candidates 14 and 15 are removed; "Large text" ×1.25 stays.

**Hover actions do not fade on the way to them:** the hover zone is the whole feed row at the height of the bubble, not only the bubble, plus a short hide delay; otherwise the row disappeared while the mouse moved from the bubble to the icons (the user's remark).

## Consequences
- `active.json` is set to `nord`. The palettes umber, moss, dusk, catppuccin-* are deleted together with the tests and stories that list them.
- `type.json` is recalculated; the visual baselines are taken again.
- Next, D1 finishes:
  - the message, the day label, the time;
  - the composer with buttons;
  - the toast;
  - the main window with the "emoji + name" sidebar.
- The first round of concepts stays in `design/concepts/feed/` as the history of abandoning the warm palette.
