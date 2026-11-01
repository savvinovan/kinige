# Kinige v2 — what I would do differently

Read: `02-prd.md` (v0.5), `01-review.md`, `research/*.md` (including both reviews). No files were changed. This is not a review for errors — there are already two reviews, and v0.5 took them into account. This is "if this were my project". Where it is taste, it is marked as such; where it is a strong argument, that is marked too. Facts from the web come with URLs; what I did not check is marked "not sure".

The main thought from which almost everything below grows: **in PRD v0.5 the most expensive part — server + accounts + Postgres + a key scheme with two wrappings — is built for a service that, by the PRD's own definition, "does not see and does not merge the content".** Such a service does not have to be a server. And on the product side, the most important fact that is missing from the PRD: **for a new user the forest and "On this day" are empty for the first months** — which means importing the past is not a P1 convenience but the condition under which the Rediscover pillar exists at all.

---

## Top 10 in descending order of value

### 1. Replace the server with "dumb storage": per-device append-only logs in S3/WebDAV/a folder

**What to change.** Remove `server/` from MVP+ (axum + Postgres + accounts + quotas + `seq` under a lock + server backups + generations + docker compose). Instead — an "object storage" sync backend: each device writes **its own** append-only log of encrypted envelopes (`log/<device_id>/<n>.bin`), reads the other devices' logs by a per-device cursor (a vector of cursors instead of a single `seq`), blobs go to `blobs/<hmac>`. The storage can be anything with PUT/GET/LIST: S3-compatible (your own on the homelab, Backblaze B2, Cloudflare R2), WebDAV (Nextcloud), or just a folder (iCloud Drive / Syncthing / Dropbox — then "sync" = copying files, and the merge is still done by the client).

**Why (strong argument).**
- Everything the server does in §10.1–10.2 is (a) hand out a monotonic `seq`, (b) check `op_id` for idempotency, (c) store. If each device owns its own log, (a) disappears (the order within a log is the write order of a single writer, between logs it is HLC, as now), (b) disappears (a repeated write of the same file with `If-None-Match: *` is simply rejected — S3 has supported this since August 2024: [AWS docs](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html), [announcement](https://aws.amazon.com/about-aws/whats-new/2024/08/amazon-s3-conditional-writes)), (c) is done by the storage.
- A whole layer of the PRD disappears: accounts and `auth_key`, separating the login password from the KEK, "the server announces the minimum protocol version", "server generation after restore", "the server keeps purged ≥ 180 days" (the device keeps its own log itself and compacts it itself), quotas, multi-tenancy, "hosting for others = personal data operator". The key scheme shrinks to one: passphrase → Argon2id → KEK → the DEK wrapping sits next to the data in the bucket; the recovery key is a second wrapping. Access to the storage (S3 keys) is the "login", and it is in no way tied to encryption.
- From the user's point of view this is exactly the Obsidian Sync model (E2EE password separate from the account, loss = data loss; [obsidian.md](https://obsidian.md/blog/verify-obsidian-sync-encryption/)) and the "Markdown folder + iCloud/Git" model of Reflect Open and mynd from your own `competitors.md`. Self-hosted users already know how to set up S3/Nextcloud; "yet another docker compose with Postgres" is an extra barrier.
- The web client (if there ever is one) talks to S3 directly with CORS; no server code, so "a malicious admin swapped the JS" disappears as a class too — the page can be served from any static hosting.
- It can be upgraded later without breakage: if a "smart" server is needed later (push notifications, webhooks, paid hosting), it implements the same `SyncBackend { put, get, list }` interface and can add things on top, while the log format does not change.

**How.** `crates/kinige-sync` gets a `SyncBackend` trait with three operations and two implementations: `FsBackend` (a folder — for tests and Syncthing/iCloud Drive) and `S3Backend`. The cursor is `HashMap<device_id, (file_n, offset)>`. Log compaction: a device rewrites its old files into a single snapshot and deletes the superseded ones — other devices see this via LIST. "An old device came back after 200 days" — the same quarantine rule as in §10.2, it is client-side anyway. S4a (in-process simulation) stays as is; S4c is replaced with "S3 on the homelab + FsBackend".

**Cost/risk.** No realtime (polling every N seconds or on window focus — fine for a personal journal). LIST on S3 costs money with frequent polling — solved by a `manifest.json` that each device updates after a push, plus an ETag check. The blob garbage collector runs on the client, under a lock file with `If-None-Match` (or simply "only the device that wrote last deletes"). MinIO as the recommendation in §10.1 — **no longer an option**: the community repository has been in maintenance mode since December 2025 and was archived in February 2026 ([ayedo.de](https://ayedo.de/en/posts/minio-im-maintenance-mode/), [glukhov.org](https://glukhov.org/data-infrastructure/object-storage/minio-dead/), [InfoQ on alternatives](https://infoq.com/news/2025/12/minio-s3-api-alternatives/)); images on Docker Hub are no longer published. Take Garage, SeaweedFS or RustFS — I did not check their state as of October 2026, a half-day spike is needed. For the first weeks Backblaze B2 or just a folder in iCloud Drive is enough.

I would reconsider "our own server" precisely because it gives no guarantee that a bucket does not give, yet it costs 3 of the 7 spikes and all of slice 11.

### 2. Import the past — before daily use starts, not in P1

**What to change.** Move the Telegram import ("Избранное" (Saved Messages) + personal chats) and importing photos from a folder by EXIF date into the "before the 4 weeks start" block (between slices 3 and 4). Leave Notion/Obsidian in P1.

**Why (strong argument).** Pillar No. 2 ("Rediscover") and the forest are functions of history. With an empty database "On this day" stays silent for a year, and the forest is bare ground. A four-week "on yourself" trial without the past will only test Capture, and you will make the "should this be a product" decision based on a truncated picture. The Telegram export is documented JSON (`result.json` with `messages[]`, `date`, `text_entities`, media by relative paths: [core.telegram.org/import-export](https://core.telegram.org/import-export)); the import is 1–2 days of agent work. Photos from Photos/a folder by date — "the day's media attachment" fills the forest with a second layer (their weight of 0.5 is already in the formula). For onboarding other users this becomes the "first wow": paste the export — and within a minute you see your 5 years as a forest.

**How.** The importer is a pure function `export → Vec<Message>` in `kinige-domain` with a test on a real export; dedup by `(source, source_message_id)` in `unknown_json`/a separate `origin` column. `journal_date` comes from the message date in the device's time zone.

**Cost.** +2–3 days before "day 1". Risk — the Telegram export format changes; a fixture file in the repository is needed.

### 3. Cut the pet; make the forest navigation and a "mail carrier", not decoration

**What to change.** The pet (XP ledger, levels, accessories, deterministic ids for merging) — cut it entirely, together with `xp_ledger`. The forest is not a separate "take a look" tab but (a) the main way to move through time and (b) the source of the daily "delivery" of old material.

**Why.** In the PRD the pet has no connection to notes at all except "+10 per day". It is a separate game that pulls toward itself (accessories, levels) rather than toward entries — and that is exactly what mynd/One Year sell as an absence ("no streaks", your `competitors.md` § 3.8). Strong argument: the whole "reward" should be **the past itself**. The forest can already provide that:

- **The morning starts with a letter from the forest.** On the first open of the day, a system card "On this day: 2023, 2021" appears in the Inbox (1–3 notes, photos, how many years ago) with a "reply" button. The reply is an ordinary message with `reply_to_id` pointing at the old entry, marked "2 years later". That is how threads across years appear, and the old note gets a "new layer". This is Day One's "On This Day" (they do not even have a "random entry" — only forum requests: [forums.dayoneapp.com](https://forums.dayoneapp.com/forums/topic/feature-request-random-entry/)), but built into the chat stream, which Day One does not have.
- **The tree also grows from rereading** (taste, but I like it): the crown comes from the day's entries, the "undergrowth/flowers" at the foot from how many times you came back to that day (replied, added to favorites, found via search and opened). This is the only mechanic that rewards Rediscover rather than Capture. No penalties, derived, needs no separate ledger.
- **The forest as a scrubber.** Click a tree — the feed scrolls to that day (without switching screens, the forest collapses into a strip at the top or into a side panel). Hovering over the feed highlights the tree. Then the forest is a "map" of the feed, not a gallery.

If you still want the pet (taste): make it the one who **brings** the "On this day" card — one creature, one animation, zero progression. Then it is functional.

**Cost.** "Cosmetics" is lost as a paid candidate — see item 9, the replacement there is better. The gain — one less synced entity, no merge rules for it, no art for accessories.

### 4. Start using it in week 3, not after 4 slices + D + spikes

**What to change.** Before slice 0 — one week of a "walking skeleton": a Tauri window, a textarea without CodeMirror, SQLite, a feed on a plain `div` (no virtualization), one Inbox chat, a global hotkey (the plugin is ready-made), export to Markdown. No Storybook, no tokens, no CI signing. From week two — you are writing in it. In parallel — everything the PRD calls slice 0 and D, but now against live data.

**Why (strong argument).** PRD v0.5 has already admitted that "you need to start using it earlier" (both reviews), but according to the roadmap, before the first entry there is still: slice 0 (CI, signing, wasm32, Storybook, visual tests, screenshot script) + D (tokens, fonts, components) + S1 (3 days) + S3 (4 days) + slices 1–4. That is 4–6 weeks for one person with agents, and it is lesson L7/L8 once again. Rule 11.1 "until design system v1 is assembled in Storybook, slices with UI do not start" is exactly the bureaucracy that produced 20 documents in the old Kinige. Visual acceptance is needed, but Storybook is not required for it: a screenshot of the real window is enough, and Storybook gets wired in when there are >10 components.

**How.** See the "first 4 weeks" section at the end. Key point: the skeleton's data **is not thrown away** — the SQLite schema from day one is the one in §4.2 (UUID, `journal_date`, `hlc_json`, `deleted_at`), so everything written during the "skeleton" weeks lives on.

**Cost.** For the first two weeks the interface is ugly. That is deliberate: beauty is a slice, not a precondition.

### 5. A Markdown mirror on disk instead of a "daily database snapshot"

**What to change.** SQLite stays the primary storage (search, forest, sync — all of it runs on it). But instead of "a daily database snapshot into a folder" — a **continuous one-way mirror** into the folder `Kinige/<chat>/YYYY/YYYY-MM-DD.md` (format as in the §16 export; written in the same transaction or in a queue after commit, with debounce). Not two-way.

**Why.** Strong argument: (a) a backup a person can read without the app is trust in local-first in the most literal sense; Time Machine and any cloud drive pick it up for free; (b) Obsidian/iA Writer/grep work with this folder right away — the answer to "what if the project dies"; (c) "export" stops being a separate feature — it always already exists; (d) the whole market trend in your `competitors.md` is Markdown files (mynd, Reflect Open, Obsidian). Why not two-way: two-way file sync is a second merge protocol with conflicts out of nowhere (mynd had to build a "find conflicting notes" feature for this). The folder is an output, not an input; the input is import (item 2).

**How.** A `mirror_dirty(journal_date, chat_id)` table; a background task rewrites the day's file in full (atomically via temp + rename). Attachments — symlinks/copies in `attachments/`. A "where" setting defaulting to `~/Documents/Kinige`.

**Cost.** Plaintext on disk (but SQLite is currently plaintext too — §4.1). Double writes during a bulk import — postpone the mirror until the import finishes.

### 6. Simplify the crypto scheme to one passphrase and one envelope; audit not "before release" but "in the format"

**What to change.** Together with item 1: one secret — a passphrase (or only the 24 words, if a passphrase of ≥ 12 characters is unwanted) → Argon2id → KEK; the DEK is random; two wrappings (passphrase, recovery) are stored in `keys.json` in the bucket; `auth_key` disappears. The envelope is as in §10.3 (XChaCha20-Poly1305, AAD with metadata); that is right and stays. DEK rotation, device revocation, QR transfer — not even in P2: "revocation" = change the S3 keys and re-encrypt the bucket with a "move to new storage" command.

**Why.** Every entity in the key scheme is an onboarding screen, a recovery scenario and a test. Right now there are five (password, auth_key, KEK, DEK, recovery). For one user on 2–3 machines, two are enough. Day One lives with "the key is in Keychain, lose it and it's lost" ([dayoneapp.com](https://dayoneapp.com/guides/security/end-to-end-encryption/) from `competitors.md`), Obsidian with a single password. Strong argument: fewer primitives — fewer places where home-grown logic around libsodium will get something wrong.

**How.** The format of `keys.json` and of the envelope — a separate short ADR with test vectors (fixed nonce/key → expected bytes), so that a web/mobile client can later be checked against one table.

**Cost.** Without an account there is no "delete account" — there is "delete the bucket", and that is more honest than §10.3 about server backups.

### 7. Quick capture — with context and appending, not "a single field"

**What to change.** The hotkey window shows the last 2–3 entries of today in faint text above the input field and supports two actions: Enter — a new entry, ⌘Enter (or `+` at the start) — **append to the last entry** (if it is < 10 minutes old). Plus: pasting from the clipboard via a second hotkey (⌥⇧Space — "save clipboard to Inbox", no window), and "Esc saves a draft" already exists.

**Why.** A strong argument from personal experience with "Избранное" (Saved Messages) in Telegram: thoughts come in a cascade, 40 seconds after the first one; without context a person either makes 4 fragment messages (the forest encourages this — spam) or opens the main window. "Append" removes both problems and makes the feed more readable (paragraphs instead of shards). That is exactly what sets it apart from a Raycast field and from Telegram.

**How.** The capture window gets permission to read "the last N Inbox messages for today" — this breaks the IPC matrix from §12.3 ("only create a message"), and that is fine: the matrix change is "create + append to the last one + read the last 3 for today". The negative test stays: it cannot read arbitrary messages or search.

**Cost.** Half a day. A second hotkey on Linux/Wayland — the same caveat that already exists.

### 8. Cut the "editorial" and "performance" ambitions of P0 down to what one person actually needs

**What to change (concrete list).**
- **A feed for 100k and spike S2** — from P0 to "when the database passes 20k". Your own volume after the Telegram import is more likely thousands, not hundreds of thousands. Keyset pagination by `(journal_date, created_at, id)` is needed from day one (it is the data model), virtualization is not: up to ~5k messages a plain DOM with "load more" works. Keep the load script, measure the threshold.
- **Three levels of forest detail** — one sprite + a dot (already said in the review, but §7.1 still has three).
- **Version history with UI** — store revisions (needed for conflict and "undo"), the "view versions" UI — P1.
- **Moving between chats, changing the date, pinning a message** — P1. Your chats are Inbox + 2–3 topical ones; "move" is needed once a week.
- **"Media/tasks/favorites" filters in the feed** — only "favorites" and "tasks"; "media" goes with the day gallery in P1.
- **Reduce Motion, screen readers "at a basic level"** — keep keyboard and contrast, screen readers at the product stage.
- **wasm32 in CI for `kinige-domain`** — remove until the web decision (item 10). It is free while the domain has no dependencies, and stops being free the very first day you want `rusqlite` or `tokio` in the domain.

**Why.** Each item is a separate slice with acceptance. PRD v0.5 is already shorter than v0.4, but P0 is still ≈ 12 features × 3 states. There are three pillars; let P0 have one flagship feature per pillar: Capture = hotkey with appending; Rediscover = the "On this day" card + forest scrubber; Grow = the tree. Everything else — after the 4 weeks.

**Cost.** Taste in the details (someone needs "move to chat" daily). The argument is strong only for ordering, not for cutting forever.

### 9. Business: sell a hosted bucket, a mobile client and forest art packs, not "AI and the pet"

**What to change.** Replace the paid features (candidates in §2.6) with:
1. **Kinige Cloud** — that same bucket (item 1) "turnkey": encrypted storage + versioned backups + an "all devices online" status. You store only ciphertext, so the personal data operator obligations are minimal (this still needs a legal check — not sure about 152-FZ/GDPR for ciphertext). Price benchmarks from your `competitors.md`: Obsidian Sync $4/month, mynd $9.99/year, Day One $49.99/year — a $2–4/month niche is realistic.
2. **Mobile app** — paid in the store (or free with paid sync). The store is a natural paywall: forking the open client and publishing it under your own name in the App Store is separate work with signing and review that "forkers" usually do not do.
3. **Forest art packs** ("coniferous", "Japanese garden", "desert", winter decorations) — this is the only place where the Blender pipeline from §7.5 pays off as an asset: a render script set up once → new packs are cheap, and they can be sold for $3–5. Cosmetics that do not break "no penalties".
4. **AI** — do not sell it; BYOK (your own OpenRouter key) in the free version; in Cloud — "included credits". Selling AI as a feature means paying for other people's tokens out of the margin (the Mem lesson from `competitors.md`).

License: with this model the client core can even be MIT/Apache (less friction for self-hosted people), because the paid part is not in the client. AGPL makes sense only if the paid part is server code, which per item 1 does not exist.

**Why.** The PRD itself calls open-core "paid features inside an open client" easy to fork (§2.6). Everything I listed is either physically not in the repository (bucket, store) or is assets.

**Cost.** Hosting needs at least one "smart" component — issuing S3 keys and billing. It is a small service, but it does appear. Build it after the first 100 self-hosted users, not earlier.

### 10. Process with agents: "one slice = one one-page document + one screenshot", the owner of taste is the human

**What to change (on top of §12, which is good).**
- **The slice spec is one page before starting**, with three sections: "scenario in words", "what is NOT included", "what done looks like (a reference screenshot or a text description of the window)". The agent does not start without it; you do not accept without it. This is protection against L7 inside a slice.
- **Two agents, two roles, not three passes by one.** One writes, the second (in a clean context, without the first one's history) reviews the PR against the slice spec and the DoD checklist, and must *launch* the app and take a screenshot itself. "Check yourself" is useless for the same agent — it will confirm its own hypothesis.
- **You look only at screenshots and the live window; code — selectively.** Your scarce resource is taste and acceptance, not reading diffs. Rule: a PR without a screenshot of the real window (and "before/after" for visual changes) is not opened for acceptance.
- **Keep the project journal in Kinige** from week 2 — it is both dogfooding and a way to notice where capture is awkward. An entry "what broke / what I want" every evening is your backlog.
- **The slice budget is in calendar days, not tokens**, and a slice that goes over 2× is stopped and cut, not stretched to completion. The old Kinige died of "just a little more".
- **No spike is opened until its result is needed by the next slice.** S3 (forest) and S6 (embeddings) — in a month, not now; S1 (CodeMirror) — only when the textarea starts getting in the way (item 4).
- **ADRs — yes, but an ADR is written after the decision, in 10 minutes, and no more than 1 per week on average.** If there are more, you are designing, not building.

**Why.** The failure of v1 was the process. PRD v0.5 fixes it with rules for the agent; I would add rules for *you*: what you look at, what you don't, when you stop. Agent work breaks in two places: the agent confidently expands the scope, and the human gets tired of accepting. The one-page spec cures the first, "screenshots only" the second.

**Cost.** Two agents — twice the tokens per slice. It pays for itself with the first caught "done, 67 tests green".

---

## Other ideas in brief

**Product and UX**
- **Auto-merging paragraphs in the feed.** Render messages of one day with a pause of < 5 minutes as paragraphs of a single "page" without times between them, the time only on the first. §6 already calls this "merge into a page"; I would make it a time-based rule rather than a visual trick, and show the time on hover.
- **"Random entry"** (⌘R or a button in an empty Inbox) — the cheapest Rediscover feature; Day One never built it (see the link in item 3). Weight the choice toward days with photos and "favorites".
- **A daily "cover".** The day's first photo as a background/thin strip at the day separator in the feed and as the tree texture in the hover card. This is what makes the feed a "journal" rather than a chat, with almost no code.
- **Search — one field for everything:** `#тег` (#tag), `@чат` (@chat), `2023-10`, `октябрь 2023` (October 2023), `фото` (photo), "на этой неделе" (this week). Filter chips appear after typing, not before. Enter navigates into the feed with highlighting (already exists). Semantic search later simply adds a second source to the same results (RRF is already described).
- **`#тег` (#tag) tags — in P0, not P1.** They cost nothing (a regex on write + a `message_tag` table), and they are the main structuring tool for users of Telegram "Избранное" (Saved Messages) (your `competitors.md`, Telegram). Without tags the imported Telegram archive will lose the only structure it had.
- **Voice notes** — do not record them ourselves in P1, rely instead on macOS system dictation in the input field (free, offline on Apple Silicon — I did not check the exact limitations, not sure). Our own recording + file — P2.
- **Forest: 12×31 — yes**, but add a third "grid": a **linear year strip** (365 trees in a single row with scrolling) as a scrubber mode above the feed. Isometric for "looking at the year", the strip for navigation. It is the same set of sprites.
- **Season as the forest's mood, not just the date.** Taste: a day with "favorites" blossoms; a day you came back to a year later gets a lantern. Decor as a signal, not as a random set.
- **"Year in review"** (P2 in the PRD) — cheap even before AI: "the longest day", "the most photos", "first mention of a #tag", "days you came back to". All from SQL.

**Architecture and stack**
- **CodeMirror 6 — only if the textarea has failed.** Markdown highlighting in a textarea is not needed for a chat: 90% of entries are 1–3 lines. CM6 is justified for an "expanded mode". I would start with a textarea + preview on ⌘P and put CM6 into a full-screen mode in a later slice. That takes S1 and the whole IME risk off the critical path (taste with an argument).
- **A single SQLite writer** — right; I would add: all Tauri commands are `enqueue(Command) → Result`, the frontend never waits for IO on the UI thread, and instead subscribes to `MessageAdded` events. This is at the same time the model for sync (incoming envelopes are the same commands).
- **`ts-rs`/`tauri-specta`** — specta, because it generates commands too, not only types; I did not check its state as of 2026 — not sure.
- **Light theme first, dark second** — taste, but there is an argument: the "paper journal" and the references (Bear, Day One, Apple Journal) are light; dark "paper" is a rare beast that easily drifts into the "dark UI with purple" you rejected. If dark, then specifically warm, checked on a real screen in daylight. I am not challenging the decision, I am warning about the drift.
- **The web client** I would strike from P1 entirely (item 1 makes it technically possible, but its value for you is zero until there is a phone). The phone — a native client (SwiftUI) for the same bucket, the protocol allows this; `apple-tech.md` is already at hand.
- **Attachments: do not deduplicate by sha256 in P0.** The space savings are negligible for a personal journal, and the rules "delete only if nothing references it, including the trash and revisions" are a GC that is easy to get wrong. A file belongs to a message; the message is physically deleted — the file is deleted. Dedup — when importing a photo library appears.
- **`unknown_json` round-trip** — good; I would add `feature flags` in the bucket manifest: a device that does not understand a flag switches to read-only. Simpler than a "minimum protocol version".
- **Local auto-backups** given the mirror (item 5) — a copy of the `.sqlite` once a day into the same folder, the last 7. That is enough.

**Process and product**
- **Feature name** — not "Forest"/"Лес" in marketing but something of its own ("роща" (grove), "год" (year), "поляна" (glade)); the PRD already knows this, I will add: check the name Kinige in the App Store/USPTO now, not "before release" — renaming after 4 weeks is cheaper than after a year.
- **One public artifact from the first week** — a screenshot of the day in a private channel/thread. Not for marketing, for the "done = visible" discipline.
- **One metric: how many days in a row you wrote ≥ 1 entry** — and it is the same thing the forest draws. If after 4 weeks there are fewer than 20 trees in the forest, the product is not needed even by you, and that is also a result.

---

## What I would cut entirely

1. **The server (`server/`, axum, Postgres, accounts, quotas, generations, server backups, docker compose)** — replaced by a bucket (item 1).
2. **The pet and `xp_ledger`** — (item 3).
3. **The web client**, and together with it the wasm32 requirement for the domain and "SQLite in OPFS — a separate decision".
4. **Three feed variants in Storybook** — choosing from static images was already adopted in v0.5; I would postpone Storybook too until >10 components.
5. **`auth_key`/separating the login password and the KEK, device revocation, DEK rotation, QR key transfer** — no account, so none of this either.
6. **Link previews** in any form until P2 (already P1 — I would go further: it is network, privacy and a parser of other people's HTML for the sake of cosmetics).
7. **Multi-level forest detail and the PixiJS branch** — Canvas 2D and one sprite; 372 tiles do not require WebGL (your `tauri-stack.md` § 4 says the same).
8. **Notion import** — a Notion export maps poorly to a chat format (pages, not entries); Obsidian is closer, but also not "messages". Telegram and Day One/Apple Journal (they have dates and short entries) — yes, Notion — no.
9. **Attachment dedup by sha256 and a global GC** in P0.
10. **Global analytics/metrics** — the PRD already struck it, I confirm.

---

## If I were doing it myself: the first 4 weeks

Premise: one person, evenings + weekends ≈ 20 h/week, two agents (a writer and a checker). A new repository (`kinige-v2`), git from the first commit, CI = `cargo test` + `tsc` + building the `.app` without signing — half a day for that, no more.

**Week 1 — walking skeleton (nothing pretty).**
- Day 1–2: Tauri 2 + React + Vite; `crates/kinige-store` with rusqlite, migration v1 per the §4.2 schema (chat, message, attachment, revision, `hlc_json`, `deleted_at`; without `change_outbox` — an empty table for now); a `send_message` command, a `message_added` event; `KinigeApi` + `MockApi` (thin, no Storybook).
- Day 3: the feed on a plain DOM, grouping by days, a textarea with Enter/Shift+Enter, the draft in localStorage. One Inbox chat.
- Day 4: a global hotkey (`tauri-plugin-global-shortcut`) + a small window with a field; tray. The IPC matrix for the second window — right away, it is 20 lines of capabilities.
- Day 5: the Markdown mirror in `~/Documents/Kinige/Inbox/2026/2026-10-25.md` (item 5); kill the process → restart → everything is in place; `integrity_check`.
- Acceptance: a screenshot of the main window and the capture window, posted to the thread. **From Friday evening I write in it.**

**Week 2 — the past and structure.**
- Import of the Telegram export (`result.json` → Inbox or a separate "Telegram" chat), dedup by `source_id`; import of a photo folder by EXIF date (attachments without dedup).
- `#тег` (#tag) tags (parsing on write, a table, filter on click).
- Editing (revisions in storage, no UI), deletion with "Undo" and a trash (only `deleted_at`, no physical deletion yet).
- Search: FTS5 over a stemmed column (S5 is exactly one day, I do it right here on my own data).
- Acceptance: found my note from 2022 by a word form; the feed with 5 years of history does not lag (if it lags — that is when virtualization comes in).

**Week 3 — Rediscover.**
- The "On this day" card in the Inbox in the morning + "reply N years later" (`reply_to_id` + a visual thread).
- "Random entry".
- Forest v0: 12×31 on Canvas 2D, circles/simple shapes instead of trees, stages per the §7.2 formula, hover card, click → the feed scrolls to the day (the forest in the top panel, not a separate screen). Hit-test on the grid. That is half of S3; the second half (Pixi, Linux without a GPU) is not there yet.
- The first batch of tokens: `tokens.json` → `tokens.css` only for the colors/typography/spacing already in use; a font with Cyrillic (woff2) embedded. No Storybook.
- Acceptance: opened it in the morning — saw myself from three years ago, replied, and that day's tree got "undergrowth".

**Week 4 — reliability and the second machine.**
- `change_outbox` with envelopes (XChaCha20-Poly1305, AAD per §10.3), passphrase → Argon2id → KEK, `keys.json`; a simulation of two clients in one process with property tests (S4a — exactly now, when the format has already lived three weeks on real data).
- `SyncBackend::Fs` (a folder) → a check on two Macs via iCloud Drive or Syncthing; `S3Backend` — if there is time left (Backblaze B2 or a bucket on the homelab).
- Light/dark — whichever was chosen; the first polish of the feed (typography, spacing, "the day's page") — the first slice where the checking agent compares the screenshot with the description from the one-page spec.
- Retro: 28 trees in the forest? If yes — PRD v0.6 gets written, in which what remains from P1 is what was missing during the month, not what was invented before it. If not — an honest conversation about why, and that is also a planning success.

What was deliberately left out of the 4 weeks: CodeMirror, virtualization, Blender art, Windows/Linux, signing/notarization/updater, embeddings, Storybook. All of that is slices 5+, and each of them now starts with a month of your own data and habits, which will show whether it is needed at all.
