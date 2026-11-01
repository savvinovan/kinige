# Slice D1: Design system foundation

Budget: 3 days. Depends on: slice 0 (Storybook, tokens JSON → CSS, visual tests, the window screenshot script, `MockApi`), Codex concepts (below). Sources: PRD §11, §15, §2.1, §5, §6, §1.2.

The other components are added in their own slices when they are needed: the quick capture window in slice 2; the interactive chat list (emoji choice, collapsing the sidebar) and the tag in slice 3; the “On this day” card in slice 5 (§15: the design system grows with the slices).

## Scenario
1. Before D1 starts, Codex (`$imagegen`), following a brief approved by the user, draws static concepts per §11.2 on the same entries (Cyrillic, different days, a task, a quote, code): the main window in three feed variants: Editorial, Conversational, Immersive. Files go in `design/concepts/feed/`. This is a mood reference, not a spec.
2. Day 1: the user looks at the concepts and chooses a feed variant. The choice is recorded in an ADR. Without the choice the feed components do not start; tokens and fonts can be done in parallel.
3. The user opens Storybook and sees the «Главное окно» (“Main window”) screen on `MockApi` data: a sidebar with chats as “emoji + name” (no chat title above the feed), the feed in the chosen variant with day labels and times, a composer with the “file”, “emoji”, “microphone” buttons at the bottom. The palette is neutral graphite, the density is like Telegram, the accent is muted, messages are bubbles without tails (PRD v0.7, ADR-0004).
4. They turn on large text: nothing is cut off and nothing overlaps. They go through the screen with the keyboard: focus is visible on every interactive element.
5. They open the «Типографика» (“Typography”) story: all styles (regular, italic, semibold/bold, mono) in Russian and English, and bold is really bold.
6. They run the token test: they see a table of “text / background” pairs with contrast ratios, all passing WCAG AA.
7. They launch the app: on a temporary screen on `MockApi` in the real Tauri window (WKWebView) the same screen looks the same, and the fonts are embedded, not system ones.

## Included
- **Dark theme tokens** (`design/tokens/*.json`, role-based names): colors: background, paper, text ×3, accent, divider, selection, focus, error (the “not sent” state, §1.2 item 9); type scale: day heading, H1–H3, body, small, code, metadata (size + line height); spacing, radii, shadows, animation durations and curves with a variant for Reduce Motion. The text scale mechanism goes through tokens.
- **Fonts:** a grotesque (Inter) and a monospace (JetBrains Mono), woff2, OFL, Cyrillic, regular / italic / semibold or bold; licenses next to the files.
- **Components** (`web/src/ui/`, semantic tokens only):
  - message: text, H1–H3, lists, task `- [ ]` / `- [x]`, code (inline and block), quote, link; a bubble without a tail, the time on every message right after the text; a series (pause < 5 min) gets a smaller gap; the “highlighted” state;
  - day label: one, without repeating the date (today, yesterday, a date, a date with the year for past years), time label;
  - composer: a plain input field with the “file”, “emoji”, “microphone” buttons (look only; the functions come in slices 3, 7, 7a): empty, multi-line up to ~40% of the height and scrolling beyond that, with a draft, send error;
  - the “Undo” toast (5 s, `aria-live`, the button is reachable from the keyboard);
  - button and input field, to the extent the components above use them.
- **A screen in Storybook:** the main window (a sidebar with “emoji + name” chats from `MockApi`, without emoji choice and collapsing, which are in slice 3; the feed; the composer).
- **A temporary screen in the app** on `MockApi`, for a screenshot of the real window before slice 1; slice 1 replaces it with the real one.
- **Token test:** all roles are set, text / background contrast ≥ 4.5:1, large text and UI ≥ 3:1.
- ADR: the chosen feed variant, the pair of fonts, the accent color.

## Not included
- The quick capture window (slice 2), choosing a chat emoji, collapsing the sidebar, the emoji picker and the tag (slice 3), the “On this day” card and the “N years later” reply (slice 5).
- The forest, the forest day card, season colors: slice 6.
- Images and files in a message, a composer with attachments: slice 7.
- An expanded composer and live Markdown highlighting (CodeMirror, spike S1).
- The context menu (slice 3); search (slice 4); jump to date, the floating date, filters, a density switch (D1 has only “compact”).
- Feed variants that were not chosen in Storybook: only the chosen one.
- The light theme (P1), checking font rendering on Windows and Linux (slice 10).
- Connecting the components to Rust and real data (slice 1).

## What “done” looks like
Storybook: the sections «Основа» (“Foundation”: colors, typography, spacing, animations), «Компоненты» (“Components”), «Экраны» (“Screens”). Each component has state stories: empty, normal, long text and a long line without spaces, Cyrillic + Latin, error (where applicable), keyboard focus, large text. The main window at 1280 × 800 and a narrow window. Compared with the chosen concept from `design/concepts/feed/` by mood, not pixel by pixel.

## Checks
- Automated tests: the token test (roles, AA contrast); a font check: each file from the tokens exists and contains Cyrillic (U+0400–U+04FF) and the required style; visual snapshots of every story (normal and large text, one theme); an ESLint rule or a test that there are no raw colors and sizes in `web/src/ui/`.
- Accessibility in the stories: roles and screen reader labels, visible focus, animations turn off under `prefers-reduced-motion`.
- Screenshots in `docs/screens/D1/`: «Главное окно» (“Main window”, normal and large text), «Типографика» (“Typography”), the real Tauri window with the temporary screen (with the script from slice 0). The output of the token test.
- Data: the slice does not write data.

## Touches sensitive areas
No: there is no data schema, sync, encryption and no new IPC commands. New npm dependencies are mentioned in the PR or in an ADR (§12.1 item 5).

## Decisions (19.10.2026)
- The scope is D1 (this slice); the other components go into slices 2, 3, 5.
- Storybook has only the chosen feed variant (§15; the contradiction with §11.3 is resolved in the PRD).
- A temporary screen on `MockApi` in the app is acceptable.
- Claude prepares the brief for Codex on the differences between Editorial / Conversational / Immersive; the user approves it.

## Decisions (20.10.2026, ADR-0004)
- Fonts: Inter (text and UI) + JetBrains Mono (code); Literata was dropped after looking at the live window.
- Darker background and bubbles, near-white text; the input field has no outline (a thin line only in focus); an entry counter next to the chat in the sidebar (look on demo data).
- A profile row at the bottom of the sidebar (look only); the feed is a centered ~720 px column, a bubble up to ~65 characters; no tabs.
- The background is neutral graphite instead of the blue-graphite Nord, the accent is light blue; a “Send” button instead of the microphone when the field has text; hover actions to the right of the bubble (“Copy” works, the rest is look only). Sidebar sections are slice 3.
- Links in entries: in D1 a click does not take the webview away from the page (the navigation is canceled); opening in the system browser is slice 1 (it needs a Tauri command/plugin and a capability).
- Palette: Nord; “warm dark paper” is dropped.
- Density like Telegram, “compact” by default: 13 px text (chosen on the live screen).
- No duplicated chat name and date.
- Buttons in the input field, emoji on chats.

## Open questions (a matter of taste, the user decides during the slice)
1. ~~Feed variant~~: decided: compact Conversational (`design/concepts/feed-v2/conversational-compact-2.png`). Text size on the live screen, a 13 / 14 / 15 px switch.
2. Nord in the live window: a pale pink error, a bluish border on fields; if Nord does not work, dark blue as in Telegram or graphite.
