# Feed concepts

Date: 2026-10-20.

Tool: the built-in `image_gen.imagegen` (`image_gen`), six separate sequential calls, no CLI and no web search.

Model: the built-in tool does not disclose its identifier; the exact name cannot be confirmed.

Status: all six PNGs are done. Order: editorial-a → conversational-a → immersive-a → editorial-b → conversational-b → immersive-b. Limits did not interrupt the work.

Each call received `referenced_image_paths` with the absolute path to `../mockup-forest-desktop.png` for structure only, not style. `transparent_background` was not set (false by default). The brief was read in full; its draft status is overridden by the user's approval.

## General notes

All PNGs were copied without resizing or retouching. Each one is 1586 × 992, close to 16:10, below the desired 2560 × 1600. The size was set in the prompt; the tool offers no separate parameter for it.

There is slight background unevenness/vignetting despite the no-gradients requirement. The palette is close but not identical between frames. No forest, pet, avatars, illustrations, composer button bars or watermarks were added. The text was checked visually; small errors may have gone unnoticed. These are mood references, not exact specifications.

The two-line draft in B was chosen to show multi-line input: "Ещё одна мысль о сегодняшнем дне" (One more thought about today) / "Продолжу завтра" (I'll continue tomorrow). The brief does not specify its text.

## editorial-a.png

File: [editorial-a.png](editorial-a.png).

Notes: An extra time, 09:14, is shown on the merged entry. The extra dates under "Вчера" (Yesterday) and "Сегодня" (Today) are wrong (March 2025 instead of October 2026). The window title is set in a serif instead of a sans-serif. Size 1586 × 992, close to 16:10, smaller than the Retina target.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: EDITORIAL FRAME A.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

````

## conversational-a.png

File: [conversational-a.png](conversational-a.png).

Notes: The time is placed to the left of the text, not on its own line above it. The column is indented from the left edge more than required. The composer has a noticeable outline. No visually noticeable distortions of the Cyrillic were found.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: CONVERSATIONAL FRAME A.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

Critical: hide 09:14 entirely, merge final paragraph into 09:12 entry. Window title uses sans-serif. Flat solid background, no gradient.
````

## immersive-a.png

File: [immersive-a.png](immersive-a.png).

Notes: An extra horizontal line above the composer; the placeholder and the time have more contrast than required. The text size is only slightly larger than in Editorial; +15% is not guaranteed. No visually noticeable distortions of the Cyrillic were found.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: IMMERSIVE FRAME A.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

Critical: time only at first entry per day: 09:12, 08:40, 09:12. NO 21:05 and NO 09:14. No window title. Sidebar completely hidden. Flat solid background no gradient. Render all frame A prose.
````

## editorial-b.png

File: [editorial-b.png](editorial-b.png).

Notes: Extra horizontal dividers between entries; the column is wider and less centered than Editorial requires. The search highlight is too noticeable. The error is colored red, breaking the single-ochre-accent rule. No visually noticeable distortions of the Cyrillic were found.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: EDITORIAL FRAME B.
Frame B composer draft exactly two lines: "Ещё одна мысль о сегодняшнем дне" (One more thought about today) then "Продолжу завтра" (I'll continue tomorrow). For Immersive B show those two lines and cursor only, no placeholder.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

Critical: today subtitle exactly "вторник, 20 октября" (Tuesday, 20 October). Window title sans-serif. Flat solid background no gradient. Show all five entries, all four code lines and both composer draft lines. No syntax coloring except muted ochre.
````

## conversational-b.png

File: [conversational-b.png](conversational-b.png).

Notes: The window title is set in a serif instead of a sans-serif. The code block has a blue syntax accent that breaks the shared palette. The search highlight is too noticeable. The composer has a noticeable outline. The Cyrillic reads visually without obvious distortions.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: CONVERSATIONAL FRAME B.
Frame B composer draft exactly two lines: "Ещё одна мысль о сегодняшнем дне" (One more thought about today) then "Продолжу завтра" (I'll continue tomorrow). For Immersive B show those two lines and cursor only, no placeholder.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

Critical variant distinction: place each timestamp on its OWN SMALL LINE ABOVE its entry, never in left margin or on same line as entry text. Wide left aligned column. Today centered compact between thin rules. Only ochre accent, error text muted ivory with ochre retry, no red. Flat solid background no gradient. All entries and code must fit.
````

## immersive-b.png

File: [immersive-b.png](immersive-b.png).

Notes: The inline command git rebase --onto main is set in a serif and with backticks instead of monospaced styling. The search highlight is too noticeable. The day caption is set in a serif. No obvious distortions of the Cyrillic were found.

Full prompt:

````text
Use case: ui-mockup
Create ONE polished raster desktop app feed concept, not a collage. Output PNG 2560 × 1600, 16:10, flat app content with no outer window border, traffic lights, window shadow, labels or watermark. Reference image is ONLY for abstract screen structure: left sidebar, main feed, bottom composer. Ignore ALL reference styling and imagery. Follow approved brief below. Render only the requested variant and frame; other variants and frames in brief are context, NOT additional panels. Fit all specified entries legibly in the viewport. Identical palette across series: warm near-black brown paper, ivory typography, muted ochre accent. No illustrations or decorative graphics.
REQUESTED SINGLE IMAGE: IMMERSIVE FRAME B.
Frame B composer draft exactly two lines: "Ещё одна мысль о сегодняшнем дне" (One more thought about today) then "Продолжу завтра" (I'll continue tomorrow). For Immersive B show those two lines and cursor only, no placeholder.
APPROVED BRIEF (user approval overrides draft status):
# Brief for Codex: three feed variants (slice D1)

Status: **draft for the user's approval**. No concepts have been generated yet. Once approved, this file is handed to Codex in full (`$imagegen`).

Sources: PRD §11.2 (direction), §6 (feed), §1.2 items 2–3 (typography, content matters more than interface); `docs/slices/D1-design-foundation.md`. The PRD names the variants (Editorial / Conversational / Immersive) but does not describe them. The description below is my interpretation; the user approves it too.

## What these images are

- **A mood reference, not a specification.** Pixels and the exact font are not copied: tokens and components are written in the repository (§11.1).
- Goal: choose one of the three feed variants. Only the chosen one is implemented.
- The model draws letters with errors, especially Cyrillic. This is expected; we look at rhythm, margins, hierarchy and color, not spelling. Entry texts are given exactly below and must be inserted verbatim.

## Common to all three

- A 1280 × 800 macOS window in a "dark paper" dark theme: a warm dark background (almost black with a brown undertone), soft light ivory-colored text, a single muted ochre accent, thin dividers. The palette is the same on all six images, so that we compare variants, not colors.
- The color on the concepts (ochre, warm background) is not a palette choice: the user chooses the palette separately in Storybook, from three candidates.
- **Forbidden:** purple and blue neon, gradients, glow, glass, message bubbles, avatars, cards with shadows around entries, emoji, level badges, the pet, the forest, illustrations. The forest, the pet and "On this day" are not part of D1.
- Entry text — a book serif with Cyrillic (like Literata or Source Serif); time, captions and interface — a neutral sans-serif (like Inter); code — monospaced. No system handwritten or decorative fonts.
- Screen structure comes from the mockup `docs/plan/references/mockup-forest-desktop.png` (structure only, not style): a sidebar on the left (placeholder), the feed in the center, the input field at the bottom. Sidebar: "Поиск" (Search), "Входящие" (Inbox) (selected), a list of 4 chats — Идеи (Ideas), Книги (Books), Работа (Work), Рецепты (Recipes). Window title: "Входящие" (Inbox).
- The input field is a plain field, not a panel with buttons: the placeholder "Написать заметку…" (Write a note…) and, small on the right, "Enter — отправить, Shift+Enter — новая строка" (Enter to send, Shift+Enter for a new line).
- Time — 24-hour format, `09:12`. Day separators: "Сегодня" (Today), "Вчера" (Yesterday), a date with the year for past years: "14 марта 2025" (14 March 2025).
- Adjacent entries less than 5 minutes apart merge into a "page": the time is shown only on the first one.

## What we draw

Each variant is **two frames** (the same entries in all three variants):

### Frame A — "day structure and prose"

From top to bottom:

1. Separator **"14 марта 2025"** (14 March 2025); entry `09:12`: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям." (I need to start preparing for system design. Look at materials on Raft and distributed transactions.)
2. Separator **"Вчера"** (Yesterday); entry `08:40`, containing an H2 heading "Планы на неделю" (Plans for the week) and a paragraph: "Закончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда." (Finish the design system foundation, pick fonts, don't open email until lunch.); entry `21:05`, a quote: "Лучше сделано, чем идеально." (Done is better than perfect.) and under it a paragraph: "Записал, чтобы перечитать, когда начну полировать пятый день подряд." (Wrote this down to reread when I start polishing for the fifth day in a row.)
3. Separator **"Сегодня"** (Today); entry `09:12`: "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать." (Woke up early and before breakfast wrote down a thought about the journal: _you write as in a messenger, you reread as a book_. The main thing is **not to choose** where to write.) (italic and bold); followed by entry `09:14` (merged with the previous one, no time): "Ещё: позвонить маме и купить хлеб." (Also: call mom and buy bread.)
4. The input field at the bottom (empty).

### Frame B — "rich content"

Only "Сегодня" (Today), from top to bottom:

1. `11:30` task list "Планы на выходные" (Weekend plans): `[x]` Поход в горы (Hike in the mountains), `[ ]` Разобрать фотографии (Sort through the photos), `[x]` Почитать книгу (Read a book).
2. `13:45` text "Починил историю командой `git rebase --onto main`, вот что получилось:" (Fixed the history with `git rebase --onto main`, here is what came out:) and a four-line code block:
   ```
   fn main() {
       let today = Day::new(2026, 10, 20);
       println!("Записей: {}", today.count());
   }
   ```
3. `15:20` link: "The Psychology of Habits — fs.blog" (accent color, underlined), under it on one line "Latin and Кириллица в одной строке" (Latin and Cyrillic in one line).
4. `16:02` entry in the "highlighted" state (found by search): "Вернуться к идее про общий черновик" (Come back to the idea of a shared draft). The highlight is a barely visible background fill, not a border.
5. `16:10` entry in the "не отправилось" (not sent) error state: "Купить билеты на субботу" (Buy tickets for Saturday) and under it, small, "Не отправилось. Повторить" (Not sent. Retry).
6. The input field at the bottom with a two-line draft (to show multi-line input).

## How the variants differ

The difference is in **rhythm and in how much interface remains around the text**, not in color.

### 1. Editorial

The feed as a typeset magazine column.

- A single centered column with a narrow measure (about 65 characters), wide margins.
- Day separator — a heading: a large serif with a thin line under it. The heading text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)). A feature of this variant: under the heading, small, in the secondary color, the weekday and the date in words ("вторник, 20 октября" (Tuesday, 20 October)), as in magazine layout; this does not affect the heading rule.
- Time — small, in the left margin (in the column margins), in the secondary color, not on the same line as the text.
- Entries without borders and without noticeable gaps: the paragraphs of one "page" follow one another, with only paragraph spacing between entries.
- Composer — a thin line and a field, no fill.

### 2. Conversational

The familiar messenger rhythm, but without bubbles.

- A wider column (about 80 characters), aligned to the left edge of the feed.
- Day separator — a compact centered label ("Сегодня" (Today)), with a line on the left and on the right.
- Time — on its own small line directly above the text of each entry, in the secondary color (the chat name "Входящие" (Inbox) is not written next to the time); on merged entries the time is hidden.
- Each entry is separated from its neighbor by a noticeable gap and a very thin line; you can see where one entry ends and the next begins.
- Composer — a field with a light fill and rounded corners at the bottom, like in a messenger.

### 3. Immersive

Maximum text, minimum interface: like reading a book in silence.

- The sidebar is collapsed into a narrow strip of icons or hidden entirely; no window title.
- The text is noticeably larger (by eye +15% over Editorial), the margins are very wide, the leading is loose.
- Day separator — a quiet caption in small caps, no lines, with a large top margin; the text follows the general rule ("Сегодня" (Today), "Вчера" (Yesterday), "14 марта 2025" (14 March 2025)), no weekday.
- Time appears only on the first entry of the day, and very faintly; the rest on hover.
- Composer — a single line at the bottom with no border and no fill, the placeholder barely visible; while writing, only the cursor remains.

## Format and paths

- PNG, 2560 × 1600 or the nearest available size with a 16:10 ratio (a Retina frame of a 1280 × 800 window), without a window frame, window shadows, captions or watermarks.
- Path: `design/concepts/feed/`:
  - `editorial-a.png`, `editorial-b.png`
  - `conversational-a.png`, `conversational-b.png`
  - `immersive-a.png`, `immersive-b.png`
- Next to them, `design/concepts/feed/README.md`: date, tool and model, the prompt itself for each file (so it can be repeated), notes on what did not work out.
- `$imagegen` limits are used up 3–5 times faster than regular requests (`docs/plan/01-review.md`); if limits are low — frame A for all three variants first, then the B frames.

## How we check the result

The user answers the questions: which rhythm is more pleasant to reread? Does the interface weigh on the text? Is it comfortable for the eye on long entries? Can you see, without captions, where a day and an entry end? The choice is recorded in an ADR; the concepts stay in the repository as a mood reference.

Critical: sidebar completely hidden, no title. Only first timestamp 11:30 very faint; hide 13:45,15:20,16:02,16:10. No divider rules anywhere. Center narrow book-like column with very wide margins, larger serif text. Composer two draft lines and cursor ONLY, no line, no rectangle, no placeholder. Error muted ivory and retry ochre, no red. Code monochrome ivory only. Solid flat warm brown-black background no gradient. Show all entries and exact four code lines.
````
