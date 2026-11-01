# Review of PRD v0.1 "Project Forest"

Date: 19 October 2026. What was reviewed: PRD v0.1, two mockups (`references/`), the history of the old Kinige (Rust/egui), competitor research (`research/competitors.md`), a check of the desktop stack (`research/tauri-stack.md`) and of Apple technologies (`research/apple-tech.md`, useful for the mobile version). Sources and "confirmed / not confirmed" notes are in the research files.

> **The normative document is `02-prd.md` (v0.6).** Fable's improvement advice: `research/advice-fable.md`.
>
> **Earlier:** If the review disagrees with the PRD, the PRD is right. Independent reviews of PRD v0.4: `research/review-fable.md` and `research/review-codex.md`; their outcome is reflected in PRD v0.5.
>
> **Update v0.4.** Per the user's notes: we build the design system ourselves (without Claude Design), end-to-end encryption with key deletion instead of data deletion, a self-hosted product with a free core and a paid tier, a single dark theme in the MVP, import from Obsidian/Notion/Telegram, embeddings on the client.
>
> **Same-day update.** After the first version of the review the user chose: desktop first on Rust + Tauri 2 + React, sync through our own server on the homelab, embeddings via OpenRouter instead of Apple's on-device AI, a "paper" style, concepts in Codex (the design system in Claude Design, later cancelled in v0.4). The sections below are updated for these decisions.

The revised PRD that can be handed to an agent: [`02-prd.md`](02-prd.md).

## 0. In short

**The idea is strong, the plan is too broad.** The PRD has the right instincts: spikes before code, vertical slices, local-first, AI optional, gamification without punishment. But the scope is larger than that of the old Kinige, which failed precisely because of breadth, an ill-suited stack for the hardest part and the absence of visual acceptance.

Six changes without which I would not start:

1. **The message body is Markdown text.** No block tree and no WYSIWYG. This is the main architectural decision, and it removes the main pain of the old Kinige.
2. **macOS first, one computer, no sync.** The data model is ready for sync from day one, though: UUID, HLC, tombstones, outbox. Windows/Linux and the server come right after the MVP.
3. **The forest grid is chosen by a prototype, not by a picture.** The mockup shows 12 × 31 (month × day), the PRD 7 × 53. Both options are tested in a spike.
4. **The forest art pipeline is a separate risk #1.** The expensive part here is not writing the code but drawing consistent trees: 7 stages × 4 seasons × variants.
5. **Visual acceptance is part of the Definition of Done.** The agent itself takes screenshots of the real window and runs the component catalog's visual tests in a browser, and the user accepts the slice. The old Kinige had none of this, and the user found all the visual bugs.
6. **The design system is the source of truth.** Codex draws concepts, while the tokens and components we develop ourselves in the repository (Storybook is the living documentation). This is the direct answer to lesson L4.

## 1. Lessons from Kinige

What went wrong in Kinige (based on the GLM and Codex session history and the state of the code) and how PRD v0.1 risks repeating it:

| #   | Kinige mistake                                                    | How it showed up                                                                                                                                                       | Risk of repeating it in v0.1                                                                   | Rule for the new project                                                                                                    |
|-----|-------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------|
| L1  | Started by copying someone else's product                         | `docs/01–10` are an analysis of Notion; the block model, `recordMap`, `saveTransactions` were taken from there. The result was a complex block editor nobody asked for | Medium: "like Notion", "like Telegram", "like Photos": 9 references in section 8.1             | The data model is derived from our own scenarios; references are only for visuals and gestures                              |
| L2  | The stack was chosen without testing it on the hardest part       | egui is good for panels but bad for editing formatted text; this was written in the project's own `docs/11` but ignored                                                | High: a Markdown composer, a feed of 100k messages and the forest are three hard parts at once | A spike of the hard part **before** the main code, with failure criteria written down in advance                            |
| L3  | "Done, 67 tests green", but nobody looked at the live window      | The disappearing handle, the crooked icon, "the elements are huge" were found by the user                                                                              | High: the PRD's DoD has no visual acceptance                                                   | Every slice is accepted by screenshots of the real window next to the baseline + catalog visual tests + the user's sign-off |
| L4  | A generated mockup was treated as the specification               | Mockup with a bold serif and neat checkboxes → implementation with a regular font and a standard checkbox                                                              | High: the images sent contradict the PRD (see 4.1)                                             | Image = mood. Specification = tokens + component catalog + geometry                                                         |
| L5  | The design direction changed without being recorded               | Notion blue + Arial → green + serif; `DESIGN_REVIEW.md` refers to an outdated spec                                                                                     | Medium                                                                                         | Decisions are recorded in ADRs; an outdated document is deleted or marked as such, not left lying next to the current one   |
| L6  | No git, edits made with `str.replace` scripts by different agents | Three passes (GLM → Codex → edits) cannot be compared or rolled back                                                                                                   | High when working through agents                                                               | git from the first commit, small PRs, CI (build + tests + lint) mandatory before the first feature                          |
| L7  | Everything at once                                                | Kinige tried to be Notion: blocks, DnD, trash, undo, import, search, themes, all in one night                                                                          | **Very high**: v0.1 is iOS + macOS + forest + pet + AI + sync + media + export                 | The MVP is cut down to one vertical scenario on one platform                                                                |
| L8  | Volume on paper instead of working software                       | 20 documents in `docs/`, half of them about a different product                                                                                                        | High: phase 7 requires 14 documents before the first line of code                              | 3 living documents + ADRs. A document is written when a decision is made, not "for the future"                              |
| L9  | Simple scenarios were broken under a complex architecture         | Switching pages did not change the document, the checkbox wrote the opposite value                                                                                     | Medium                                                                                         | First the end-to-end scenario "created → restarted → saw it", then everything else (the PRD already has this; keep it)      |
| L10 | God-file                                                          | `kinige-app/src/main.rs`: 1331 lines                                                                                                                                   | Low                                                                                            | Modules by feature, but not 13 modules up front (see 4.6)                                                                   |

**Main conclusion.** Kinige failed not because of the code but because of the process: the wrong technology for the hardest task, no visual acceptance and too broad a scope. PRD v0.1 covers part of the risks well (spikes, vertical slices, "don't implement everything"), but its scope is even broader than Kinige's.

## 2. What the market says

Details and sources are in `research/competitors.md`. What the agent found:

- **"Chat with yourself" is already a commodity, not a differentiator.** There are many niche clones (tetrify, Luckynote, Hey Me, SelfChatNote, Chat Notes…), most with 1–60 ratings, almost all of them draw **bubbles**. The closest in spirit is **mynd**: iOS + Mac, Markdown files, local, iCloud for money, "no AI and no streaks". It confirms the niche, but also shows that a "chat form" alone is not enough.
- **The real gap:** chat-style input + **a typographic feed instead of bubbles** + local-first with sync + open Markdown export + gentle gamification tied to notes. The agent did not find this combination.
- **Rediscovery matters more than capture.** Day One and Apple Journal retain users with "On this day" and memories. In the PRD "Memories" sit in v1.4; I move a simple "On this day" card to P1.
- **"A tree per day" is not unique:** One Year, Garden of You, DailyLeft, MoodForest and Daylio (Year in Pixels). The agent did not find an isometric calendar grid tied to a feed of notes; the difference is in the execution, not the idea.
- **Gamification:** in Forest (Seekrtech) the tree **dies** if you leave the app; Habitica takes away health and levels; Duolingo itself admits it plays on loss aversion and added Streak Freeze. Finch does not punish, but collects complaints about **lost progress**. Conclusion: the pet's progress must not be lost either when notes are deleted or when sync fails.
- **Name:** "Forest" is a well-known app (~50K ratings) with the same metaphor. The product is called Kinige, which is fine, but it is better not to call the feature itself "Forest" in marketing. The agent did not run a trademark check.
- **Encryption:** in Bear, titles and tags are not encrypted even with Advanced Data Protection; in Day One, entry dates are not encrypted. What to encrypt has to be decided up front.

## 3. What the technology says

Details and sources are in `research/tauri-stack.md` (the desktop stack) and `research/apple-tech.md` (the Apple stack, needed if the mobile version is native). Conclusions that affect the plan:

| Question       | Conclusion                                                                                                                                                                                                                                     | Confidence                                                |
|----------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------|
| Tauri          | Stable branch 2.12.x. Tauri v3 is in alpha (with a CEF runtime): don't touch it. The mobile part of Tauri 2 is raw, by the team's own account; Share Extension and widgets are manual work in Xcode                                            | Confirmed                                                 |
| Linux          | WebKitGTK: crashes on NVIDIA (DMABUF), WebGL may "silently" run in software. Tauri itself recommends a path without WebGL. The tray on Linux does not deliver mouse events                                                                     | Confirmed                                                 |
| DB             | `rusqlite` (bundled) with FTS5, in the Rust core. `tauri-plugin-sql` is not needed                                                                                                                                                             | Confirmed                                                 |
| Russian search | The built-in `porter` is English only, and `rusqlite` cannot register custom FTS5 tokenizers. Solution: a column with pre-stemmed text (Snowball) + `unicode61`. Trigram finds substrings, but none shorter than 3 characters                  | Confirmed; quality of `rust-stemmers` still to be checked |
| Composer       | CodeMirror 6 (Obsidian is built on it). Risk: IME bugs in Safari/WKWebView next to widget decorations                                                                                                                                          | Confirmed                                                 |
| Feed           | TanStack Virtual (`anchorTo: 'end'` for chat) or virtua (MIT). In react-virtuoso only `VirtuosoMessageList` is paid                                                                                                                            | Confirmed                                                 |
| Forest         | Canvas 2D as the base, PixiJS v8 as an enhancement; a path without WebGL is mandatory because of Linux                                                                                                                                         | Confirmed                                                 |
| Sync           | Own change log with HLC (`uhlc`) + LWW per message + revisions. ElectricSQL/PowerSync require Postgres as the source, Turso Sync requires Turso Cloud, Automerge/Loro are overkill                                                             | Confirmed                                                 |
| Embeddings     | OpenRouter `POST /api/v1/embeddings`, 33 models; `bge-m3`, `multilingual-e5-large`, `qwen3-embedding-8b` cost $0.01 / 1M tokens, `text-embedding-3-small` $0.02. Multimodal ones exist. ZDR: `provider.zdr: true`                              | Confirmed                                                 |
| Vector memory  | 100k × 1024 dimensions ≈ 0.4 GB, noticeable with the homelab limit of 1 GiB. Use 512 dimensions or int8 (since v0.4 the vectors live on the client)                                                                                            | Calculation; speed needs to be measured                   |
| Claude Design  | Released 17.04.2026, beta. The design system is stored as an artifact; into code via `/design` in Claude Code, from code back via `/design-sync`. **No file export, no version history**; the standalone claude.ai/design closes on 14.12.2026 | Confirmed                                                 |
| Tokens         | DTCG 2025.10 is a working group report, not a W3C standard. Style Dictionary 5.7, support for 2025.10 is still being finished                                                                                                                  | Confirmed                                                 |
| Codex imagegen | A real feature (`$imagegen`, `gpt-image-2`); it burns through limits 3–5 times faster, consistency across a series is not guaranteed                                                                                                           | Confirmed                                                 |

**Consequences for the plan:**

- **Linux is a separate risk.** The forest and animations are tested on Linux in a spike, not at the end.
- **Claude Design rejected:** no export and no version history, the standalone site closes on 14.12.2026. The design system lives in the repository.
- **Codex is for concepts only.** The final sprites come from Blender, where a series is consistent by construction.
- **Semantic search lives on the client.** With end-to-end encryption the server does not see the text, so the client computes the embeddings and searches; the vectors sync encrypted.

**Why Tauri and not SwiftUI, even though I proposed SwiftUI in the first version of the review.** SwiftUI was the answer to PRD v0.1 (iOS + macOS + CloudKit). The decision "desktop on three OSes + our own server + web later" changes the priorities: one codebase on three OSes and a shared frontend for the web matter more than native iPhone gestures. The price: the phone will later be either on Tauri 2 mobile (raw) or a native client to the same server. The second is more realistic, and our own sync protocol makes it possible.

## 4. Contradictions and gaps in PRD v0.1

### 4.1. The mockups contradict the PRD

- **The forest grid.** PRD: 7 × 53 (weekday × week, like GitHub). Mockup 2: rows "Янв…Дек" (Jan…Dec), columns "1…31", that is 12 × 31 (month × day). Mockup 1 (screen 4) is a landscape with no grid at all.
  - 7 × 53 in isometric view is a strip with an aspect ratio of ≈ 1 : 7.5. In a laptop window the tiles come out small, and on a future phone in portrait the year is not readable at all: either tiny tiles or constant panning.
  - 12 × 31 is an almost square rhombus, the date is readable without a hint ("row October, column 16"), the months group seasonally into bands. The price: 372 positions, 7 of them nonexistent dates (30–31 February, etc.), which are shown as empty ground/stones; the weekday is not visible.
  - **Decision:** build both options in spike S3 and choose by readability in a 1280 × 800 window and in a narrow window. Preliminary recommendation: 12 × 31 for the forest, keep 7 × 53 for a flat "heatmap" view if one is needed.
- **Visual style.** The mockups mix three styles: a flat dark UI with a purple accent, photorealistic photos, a 3D "Pixar-like" forest and kitten. The PRD (8.5) explicitly forbids mixing design systems. One direction has to be chosen and turned into tokens; lesson L4.
- **The forest in the mockup is an illustration, not assets.** Every tree there is unique. In reality it is a set of sprites: 7 stages × 4 seasons × 3–5 variants = 84–140 images, plus ground, decor, highlighting, growth animations. **This is the main hidden cost of the product: not the code but the art pipeline.** The PRD does not say a word about it.
- The "3 voice notes" counter on the hover card and the pet/report screens show features that are not in the MVP. MVP mockups should show only the MVP.

### 4.2. Gamification

- **XP vs derived data.** The PRD (9.1) makes DailyActivity/ForestTile derived from messages, which is right. But if the pet's XP is also computed from messages, deleting notes will lower the level → that is a punishment (a violation of principle 6). **Decision:** the forest is derived (it can shrink on deletion), XP is a separate append-only log of awards, nothing is taken away.
- **The forest rewards spam.** Tree size = number of notes → 30 messages "." produce a sequoia. The PRD sets a daily limit for the pet, but not for the forest. Options: compute a "day weight" (`log` of the number of notes + a bonus for media/length), cap the contribution of empty/short messages. Decide in forest-spec.
- **Seasons in the year view.** One board shows winter (Jan–Feb, Dec), spring, summer and autumn at the same time. This is both beautiful and correct, but the PRD at the same time proposes a "Зима/Весна/Лето/Осень" (Winter/Spring/Summer/Autumn) switcher (in the mockups), and the switcher's meaning is not defined (a filter? zooming in to a quarter?). For 12 × 31 a season = a group of rows, the switcher = a quick zoom to the group.
- Southern hemisphere: a "shift seasons by 6 months" setting is cheap; do it right away as a parameter.

### 4.3. Input and content format

- **Markdown in P0, "text formatting" in P1, an extended composer with preview somewhere in between.** What is stored is not defined. **Decision (key for the data model):** the message body is **Markdown text (CommonMark + GFM task lists)**. The composer edits plain text with live syntax highlighting (like Bear), the feed renders it. No WYSIWYG and no block tree (the Kinige lesson). The "B / I / list" buttons simply insert syntax.
- A checklist inside Markdown: clicking a checkbox in the feed changes the text (`[ ]` → `[x]`); this is an edit of the message, and it must go through the same version history and sync.
- **"Convert to Task / Reminder / Quote / Bookmark"** mixes different things: Task/Pin/Favorite are **properties** of a message (flags), Quote is text **formatting**, Reminder is an **integration** (EventKit or a local notification), Bookmark is an **attachment type** (a link with a preview). Each is a separate decision.
- Enter sends, Shift+Enter inserts a new line (desktop). Write this down explicitly; for the future mobile client, separately.
- **No system-level capture points.** "Zero-friction" is not just the app window. On the desktop: a global hotkey with a small input window and a tray / menu bar; later a browser extension; on the phone, a Share Extension and widgets. This is what most distinguishes "chat with yourself" from Telegram Saved Messages.

### 4.4. Platforms and sync

- PRD v0.1: "iOS, macOS; potentially Android, Web" + "CloudKit" is a contradiction: CloudKit is closed to Android and awkward for the web. The user's decision removes it: **desktop on Tauri 2 + our own server on the homelab**. The transport is our own, which means both Android and the web can later connect to the same protocol.
- **Tauri does not give a web version automatically.** It builds the desktop app (and mobile apps in version 2), and there is no Rust backend in the browser. So the frontend talks to the backend through the `KinigeApi` interface from day one; the web client later is an `HttpApi` to the server.
- **Our own server is a responsibility:** Postgres backups, access from outside the home, device authentication, updates. The upside: the same server provides the web client and computes embeddings once for all devices.
- A limitation of the homelab deployer: the persistent volume is 1 GiB, memory up to 1 GiB. That is too little for a media library; attachments have to be stored on the cluster's NFS/MinIO.
- Conflicts: creation is conflict-free (UUID); editing the same message on two devices → the larger HLC wins, the losing version goes into the version history with a "there is another version" indicator; deletion is a tombstone + a 30-day trash. A CRDT for short messages is overkill.
- ~~The server stores data in plaintext~~ → v0.4: end-to-end encryption. The web client decrypts data in the browser, search and embeddings run on the clients.

### 4.5. AI

- **The user's decision: embeddings via OpenRouter**, not Apple's on-device models. This is right for cross-platform support and removes the Russian-language problem: according to data as of September 2026, Apple Intelligence does not support Russian.
- The price of the decision: **privacy.** The text of notes goes to OpenRouter and to the model provider. We need an explicit opt-in, an estimate of volume and cost before the first indexing, the exclusion of individual chats (for example, "Personal") and honest wording in the interface.
- The user's query also has to be turned into a vector through the API, so semantic search is an online feature by definition. ~~It makes sense to compute embeddings on the server~~ → v0.4: on the client, the server does not see the text.
- A search for "photo of a black dog" requires multimodal embeddings; cheaper is to caption the image with a vision model and index the caption. Postpone until there is real demand.
- First FTS5 with Russian morphology taken into account: `unicode61` does not stem, so "книги" will not find "книга" (two forms of "book"). The solution is a stemmed column (Snowball for Russian) + prefix search. This is a real P0 problem, not mentioned in PRD v0.1.

### 4.6. Architecture and process

- **13 modules in section 9** is premature structure (lesson L10 in reverse). At the start: `App`, `Feed+Composer`, `Storage`, `Forest`. Everything else appears when a feature appears.
- **14 documents before code** (phase 7): lesson L8. Enough: `vision-and-scope.md`, `architecture.md` + `adr/`, `roadmap.md`, `forest-spec.md`. The rest are sections of these files.
- **Metrics (part XV)** contradict privacy-first and are meaningless for a pet project with one user. While the product is for myself, there is one metric: "I use it myself every day for 4 weeks in a row". Analytics only after a decision on a public release.
- **No visual acceptance in the DoD**: lesson L3.
- **There was no decision on "who it is for".** Decided: for myself; the question of releasing it comes after 4 weeks of daily use.
- **Name.** The product is Kinige, "Project Forest" is a working name. Publicly it is better not to call the feature "Forest": that is the name of a well-known app (Seekrtech) with the same metaphor of growing trees.

## 5. Summary of decisions

| #   | Decision                                                                                                                   | Status                               |
|-----|----------------------------------------------------------------------------------------------------------------------------|--------------------------------------|
| D1  | The message body is Markdown (CommonMark + GFM task lists, no raw HTML); attachments are separate                          | Accepted                             |
| D2  | Desktop first: macOS → Windows/Linux on Tauri 2; web (thin client) and phone later                                         | Decided by the user                  |
| D3  | Rust core (`kinige-domain`, `kinige-store`) + React/TypeScript; the frontend talks to the backend only through `KinigeApi` | Decided by the user                  |
| D4  | SQLite (rusqlite) + FTS5 with a stemmed column for Russian                                                                 | Accepted, spike S5                   |
| D5  | Sync: own server on the homelab (axum + Postgres), outbox + HLC, "last one wins + revisions"                               | Decided by the user, spike S4        |
| D6  | Composer: CodeMirror 6 with live Markdown highlighting                                                                     | Spike S1                             |
| D7  | Feed: a virtualized list with variable-height cells                                                                        | Spike S2                             |
| D8  | Forest grid 12 × 31 or 7 × 53; rendering with PixiJS or Canvas 2D                                                          | Spike S3                             |
| D9  | The forest is derived data; the pet and XP are cut (v0.6)                                                                  | Decided by the user (v0.6)           |
| D10 | `journal_date` is fixed at creation in the local time zone; it can be changed manually                                     | Accepted                             |
| D11 | Quick capture: global hotkey + tray                                                                                        | Accepted                             |
| D12 | Style: a calm "paper" journal                                                                                              | Decided by the user                  |
| D13 | The design system is our own, in the repository: DTCG tokens → CSS → React components → Storybook; concepts from Codex     | Decided by the user (v0.4)           |
| D14 | Art: Codex for concepts, Blender for sprites                                                                               | Decided by the user                  |
| D15 | AI: OpenRouter embeddings, computed on the client, explicit opt-in, chat exclusion                                         | Decided by the user, spike S6        |
| D16 | For myself first; the goal is a self-hosted product: free core + paid extended tier                                        | Decided by the user (v0.4)           |
| D17 | End-to-end encryption: the server and the admin do not see the data; account deletion = key deletion                       | Decided by the user (v0.4), spike S4 |
| D18 | One theme in the MVP (dark); themes are token sets, visual tests on one theme                                              | Decided by the user (v0.4)           |
| D19 | Import: Telegram only, before daily use begins; Obsidian and Notion removed (v0.6)                                         | Decided by the user (v0.6)           |
| D20 | Own server kept; the alternative "device logs in S3/a folder" rejected                                                     | Decided by the user (v0.6)           |
| D21 | Order: skeleton → design system → slices; "On this day", tags, Markdown copy, capture with appending: in the MVP           | Decided by the user (v0.6)           |
