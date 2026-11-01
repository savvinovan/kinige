# Brief for Codex, round 2: a compact feed in Nord (slice D1)

Status: based on the user's feedback on the first round (`brief.md`, `feed/`), 20.10.2026. The decisions are PRD v0.7 (§5, §6, §11.2) and ADR-0004. The result is `design/concepts/feed-v2/`.

## What these images are

- **A mood reference, not a specification** (§11.1). The goal: pick one of two feed variants, compact Editorial or compact Conversational.
- The model draws Cyrillic with mistakes; this is expected. The texts below are used verbatim.
- **Budget: 2 images** (one per variant) plus no more than one regeneration per variant.

## Common to both

**Window**: macOS, 1280 × 800 (logical points), without the window frame, shadows, captions or watermarks. One image is one variant, not a collage.

**Nord palette**: the same on both images, exact values:

| What                                        | Color     |
|---------------------------------------------|-----------|
| Sidebar background                          | `#252a34` |
| Feed, chat bar and input field background   | `#2e3440` |
| Selected chat, code block, "pill" day label | `#3b4252` |
| Primary text                                | `#eceff4` |
| Secondary text (captions, chat names)       | `#d8dee9` |
| Time, placeholders, icons                   | `#a7aebd` |
| Accent: links, checked checkboxes, cursor   | `#88c0d0` |
| Thin dividers                               | `#4c566a` |
| "Not sent" error                            | `#d89fa4` |

No brown, ochre, beige or warm paper. Flat fills, no gradients, vignette, glow, glass or shadows.

**Density as in Telegram Desktop.** The fonts are small, many entries fit on the screen:
- entry text: a book serif with Cyrillic (like Literata), **15 px**, line height ~1.45;
- interface (sidebar, chat bar, input field): a sans-serif (like Inter), 13–14 px;
- time and day labels: **12 px**;
- code: monospaced (like JetBrains Mono), 13 px.

**Sidebar** on the left, ~260 px wide: at the top a "Поиск" (Search) field (a line magnifier icon), below it a list of chats in rows of ~36 px each, with **an emoji to the left of the name** for every chat:
- 📥 Входящие (Inbox) (selected: fill `#3b4252`)
- 💡 Идеи (Ideas)
- 📚 Книги (Books)
- 💼 Работа (Work)
- 🍳 Рецепты (Recipes)

Emoji only on chats. There are no emoji anywhere else in the interface.

**Chat bar** above the feed: one thin bar ~44 px high, "📥 Входящие" (Inbox) in a 14 px sans-serif, semibold, on the left; a thin divider under the bar. **No large "Входящие" (Inbox) heading above the feed**: the chat name appears on screen only in the sidebar and in this bar.

**Input field** at the bottom of the feed, as in Telegram: ~52 px high. On the left a line paperclip icon ("attach a file"), then the field with the placeholder "Написать заметку…" (Write a note…), on the right two line icons: a smiley ("emoji") and a microphone ("voice note"). Icons 20 px, color `#a7aebd`, thin line, no fill. Do not write the Enter hint.

**Time**: 24-hour, `09:12`. Adjacent entries less than 5 minutes apart merge into a "page": only the first one shows its time.

**Day labels: one small label per day, with no second line with a date.** Today is Tuesday, 20 October 2026. There are exactly three labels, from top to bottom, from oldest to newest:
1. "14 марта 2025" (14 March 2025)
2. "Вчера" (Yesterday)
3. "Сегодня" (Today)

There are no other dates on the screen (no "13 марта" (13 March), no weekday, no "пятница" (Friday)).

**Forbidden:** message bubbles, avatars, shadowed cards around entries, emoji outside the sidebar and the chat bar, illustrations, the forest, the pet, purple and neon.

## Entries (the same on both images, from top to bottom)

The feed is scrolled to the very bottom; everything below must fit.

Label **"14 марта 2025"** (14 March 2025)
- `09:12` "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)

Label **"Вчера"** (Yesterday)
- `08:40` heading "Планы на неделю" (Plans for the week) (slightly larger than the text, semibold, not huge) and the paragraph "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick the fonts, don't open email before lunch.)
- `21:05` the quote "Лучше сделано, чем идеально." (Done is better than perfect.) (a thin vertical bar on the left) and the paragraph "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote it down to reread when I start polishing for the fifth day in a row.)

Label **"Сегодня"** (Today)
- `09:12` "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold)
- no time (merged with the previous one): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
- `11:30` "Планы на выходные" (Plans for the weekend) and a task list: ☑ Поход в горы (Hiking in the mountains), ☐ Разобрать фотографии (Sort the photos), ☑ Почитать книгу (Read a book) (checkboxes are squares, checked ones in the accent color)
- `13:45` "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is the result:) and a code block:
  ```
  fn main() {
      let today = Day::new(2026, 10, 20);
      println!("Записей: {}", today.count());
  }
  ```
- `16:10` "Купить билеты на субботу" (Buy tickets for Saturday) and below it, small, in the error color, "Не отправилось. Повторить" (Not sent. Retry)

The input field is empty, with the placeholder.

## Variants

### 1. Compact Editorial: `editorial-compact.png`

A magazine column, but dense.
- A text column of ~65–70 characters, centered in the feed area; margins on both sides.
- Time is small in the left margin of the column, opposite the first line of the entry, not on the line with the text.
- The day label is small, on the left at the column edge, in a 12 px sans-serif, in the secondary color, slightly letter-spaced; after it a thin rule to the right edge of the column.
- Entries go in a continuous flow: only paragraph spacing between entries, no rules.
- The input field spans the column width, no fill, a thin rule on top.

### 2. Compact Conversational: `conversational-compact.png`

The Telegram rhythm without bubbles.
- A wider column (~80 characters), aligned to the left edge of the feed with a small indent.
- The day label is a small "pill" centered in the feed: 12 px text on a `#3b4252` fill, rounded corners, no rules.
- Time is small on the right, at the end of the first line of the entry (as in Telegram), in `#a7aebd`.
- Entries are separated by a small gap (~8 px), no rules.
- The input field spans the full feed width: a rounded field on a `#3b4252` fill, icons inside and on the sides.

## Format and paths

- PNG, 16:10, the largest available size (in round 1 it was 1586 × 992).
- `design/concepts/feed-v2/editorial-compact.png`, `design/concepts/feed-v2/conversational-compact.png`.
- `design/concepts/feed-v2/README.md`: the date, the tool, the full prompt for each file, what did not work, whether there were regenerations.

## How we check

The user picks a variant: which rhythm is more pleasant, whether the density is enough, whether the interface weighs on the text. The choice goes into ADR-0004.

## Conversational regeneration: `conversational-compact-2.png`

Based on the user's feedback on `conversational-compact.png` (ADR-0004). This is the only regeneration of Conversational. Everything common above stays in force (palette, sidebar, input field, entries, three day labels), except what is listed below.

- **There is no chat bar above the feed** (the user: it duplicates the selected sidebar item and takes up space). The feed starts right at the top; "Входящие" (Inbox) appears only in the sidebar. Emoji only in the sidebar.

- The day label stays as it was: a small "pill" centered in the feed, 12 px text on `#3b4252`, rounded corners. The user likes it.
- **Each message is a separate plate**: a `#3b4252` fill on the `#2e3440` feed background, ~10 px corner radius, ~8 × 12 px inner padding, no tail, no shadow, no avatar. All plates are aligned to the left edge of the feed (indent ~24 px).
- **Plate width follows the text**: a short message gets a short plate, a long one is no wider than ~80 characters (about 620 px).
- **Time inside the plate, right after the text**: small (12 px, `#a7aebd`), in the bottom right corner of the plate, on the same line as the end of the text. This way the time sits next to the text, as in Telegram. **No time at the right edge of the feed.**
- **Time on every message**, no merging: "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.) gets its own time `09:14`.
- Gaps: ~4 px between the plates of one series (09:12 and 09:14 under "Сегодня" (Today)), ~10 px between the others.
- The code block is inside the plate of its message, on a `#2e3440` fill, ~6 px corner radius. The inline code `git rebase --onto main` is monospaced on a `#2e3440` fill, **with no visible backticks**.
- Quote: a thin vertical bar on the left only on the line "Лучше сделано, чем идеально." (Done is better than perfect.); the next paragraph has no bar.
- Error: "Не отправилось. Повторить" (Not sent. Retry) inside the plate below the text, in `#d89fa4`; the time of this message next to it.
- The input field stays as it was in `conversational-compact.png` (paperclip, rounded field, smiley, microphone).
