# Stack research: local-first notes on Tauri 2 + React + Rust (as of 2026-10-19)

Legend: CONFIRMED = confirmed by a primary source (official docs / repository / API) that I opened during this work; UNCERTAIN = the source did not open, is contradictory, or this is my own knowledge without verification; WRONG = the premise is wrong.
Important: WebFetch returns a condensed digest of pages, so where a page did not show a version/date, I say so instead of guessing. All numbers below are either from a source or marked as my own arithmetic.

---

## 1. Tauri 2

### Version
- CONFIRMED: the 2.x branch is alive. GitHub Releases: `tauri v2.12.4` (14 Oct 2026), `tauri-runtime-wry 2.12.2`, `tauri-utils 2.10.2`. https://github.com/tauri-apps/tauri/releases
- Discrepancy: the page https://v2.tauri.app/release/tauri/ shows 2.12.3 from 10 Oct 2026 (most likely it simply lags behind GitHub). Take "2.12.x".
- CONFIRMED: in parallel there is **Tauri v3 alpha** (`tauri v3.0.0-alpha.5`, 14 Oct 2026, marked "Latest" on GitHub, the changelog has no description of breaking changes). Not suitable for production; v2 remains the stable branch. I did not find a stable date for v3 (UNCERTAIN).
- All plugins require Rust >= 1.90 (from the global-shortcut/updater/sql/single-instance plugin pages).

### WebView per OS
- CONFIRMED (https://v2.tauri.app/reference/webview-versions/): Windows = WebView2 (Chromium/Edge, preinstalled on Win11, updates itself); macOS/iOS = WKWebView (the WebKit version is tied to the OS); Android = the system Android WebView; Linux = WebKitGTK (`webkit2gtk`, the version depends on the distribution).
- The version table on the page is incomplete (its maximum is 2.36 from Ubuntu 22.04, etc.); current WebKitGTK versions need to be checked per target distribution.

### Linux / WebKitGTK: known problems
- CONFIRMED (official https://v2.tauri.app/develop/debug/linux-graphics/):
  - The WebKitGTK DMABUF renderer on NVIDIA: white window, flicker on resize, crash on resize, errors `AcceleratedSurfaceDMABuf ...` and `Wayland Error 71`. Workarounds in order: `nvidia_drm.modeset=1`, `__NV_DISABLE_EXPLICIT_SYNC=1`, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (slower), `WEBKIT_DISABLE_COMPOSITING_MODE=1` (last resort). They advise against enabling them unconditionally.
  - WebGL/canvas can "silently" fall back to a slow path: high input latency/low FPS while the rest of the app looks normal; a WebGL2 context is created even on a software rasterizer; `WEBGL_debug_renderer_info` returns "Apple GPU" on any Linux (masking), so the real GPU cannot be determined.
  - Official recommendation: for WebGL-heavy views, have a non-WebGL fallback on Linux and a user-facing toggle.
- Fonts/rendering: there are issues tauri#14286 "Font-weight offset of 100 on webkitgtk (linux)" and #6224 "Incorrect DPI scaling when using non-integer scaling on Linux/X11" (titles obtained from a GitHub search; I did not read the contents, status not verified). WebKit Bugzilla 271714 is about blurriness with fractional scaling in GTK. Conclusion: differences in font rendering and fractional scaling on Linux are real, but there is no clear "official" page about fonts. UNCERTAIN in the details.
- WebKitGTK performance in general: the claim that WebKitGTK executes WebGL calls on the main UI thread while Chromium moves them to a GPU thread was found only in a third-party blog (instagit.com). UNCERTAIN.
- Tray on Linux: CONFIRMED, mouse events on the icon are not emitted (the icon and the context menu work). https://v2.tauri.app/learn/system-tray/
- WebRTC/camera on WebKitGTK: required building WebKitGTK with flags (discussion #8426, 2023-24). Not critical for us.

### Way out: CEF (important for the Linux risk)
- CONFIRMED (https://v3.tauri.app/develop/cef/): Tauri **3.0 alpha** introduced a runtime on the Chromium Embedded Framework (`tauri-runtime-cef`, alpha.6 as of 14 Oct). Not available in Tauri 2.0. Pro: the same Chromium on all desktops, CDP, Chromium sandbox. Con: the bundle is hundreds of MB larger, more memory, the first build downloads ~1 GB of CEF per platform. Mobile stays on wry.
- There is a third-party port `SableClient/tauri-runtime-cef` for the published crates (not official).
- Conclusion: if Linux rendering turns out to be painful, the fallback path is v3+CEF, not v2.

### Tauri 2 Mobile
- CONFIRMED (https://v2.tauri.app/blog/tauri-20/, the 2.0 stable post): iOS/Android are supported, but the team itself writes that it is "not completely happy about the developer experience", not all official plugins work on mobile, `tauri-action` could not do automated mobile builds (at the time of the post; I did not check whether this is still current), "Not all plugins are as stable as Tauri itself". I found no recent official statements about improved maturity (UNCERTAIN).
- Native code: plugins contain Kotlin (Android, `@TauriPlugin`, `@Command`) and Swift (iOS, Swift package, `@objc func x(_ invoke: Invoke)`); Rust calls them via `run_mobile_plugin`. CONFIRMED: https://v2.tauri.app/develop/plugins/develop-mobile/
- Share Extension / widgets (iOS): I did not find an official guide. These are separate Xcode targets (Share Extension, WidgetKit) in the generated `gen/apple` project with an App Group for data exchange; third-party examples: the `tauri-plugin-share` plugin (described on docs.rs as requiring a Share Extension target to be created in Xcode and files to be passed through an app group) and the `setup-ios-share-extension.sh` script in the DecentPaste app. A problem from the source: `gen/apple` gets regenerated, versions/settings drift. UNCERTAIN; this is manual native work, not "out of the box". For Android, a share target is done via an intent-filter in the manifest (my own knowledge, not verified).

### Plugins
- global-shortcut: CONFIRMED desktop only (`#[cfg(desktop)]`, dependency under macOS/Windows/Linux), permissions are granted explicitly (`global-shortcut:allow-register`, etc.). https://v2.tauri.app/plugin/global-shortcut/ The Wayland nuance (global hotkeys there are restricted by the protocol) was not checked in the source. UNCERTAIN.
- tray: CONFIRMED, the `tray-icon` feature of `tauri`; see the Linux limitations above.
- clipboard-manager: CONFIRMED, there are `allow-read-image` / `allow-write-image` permissions; on Android/iOS plain text only. https://v2.tauri.app/plugin/clipboard/ (the image API signatures are not shown on the page; see the API reference).
- File drag&drop: CONFIRMED, `dragDropEnabled` is on by default and generates a DragDropEvent; on Windows it has to be disabled for HTML5 drag&drop to work in the frontend (Tauri replaces the WebView2 handler). I.e. internal DnD (reordering) and file DnD from the OS conflict on Windows. https://v2.tauri.app/reference/config/
- single-instance: CONFIRMED, desktop; on Linux via DBus, for snap/flatpak it does not work by default without extra settings (slot/`--talk-name`). https://v2.tauri.app/plugin/single-instance/
- updater: CONFIRMED, signing is mandatory and cannot be disabled (private key via the env var `TAURI_SIGNING_PRIVATE_KEY`, `.env` does not work); a static JSON (S3/GitHub Releases) or your own server (204 if there is no update) is possible - fits a home server. Examples are desktop only. https://v2.tauri.app/plugin/updater/
- sql plugin vs our own rusqlite: tauri-plugin-sql is CONFIRMED to be built on sqlx (SQLite/MySQL/Postgres), migrations in Rust, by default the permissions are only close/load/select, writing requires `sql:allow-execute`; the page says nothing about FTS5/extensions. It exposes SQL to the frontend, which for our case (FTS, vectors, sync logic in Rust) is an unnecessary layer. Recommendation: rusqlite in the Rust core, the frontend goes through our own typed commands. This is my architectural judgment, not a fact from the docs.

### Security model
- CONFIRMED (https://v2.tauri.app/security/capabilities/): capabilities live in `src-tauri/capabilities`, are bound to window/webview labels (not to the title), permissions are merged when there are several capabilities, by default all registered commands are callable from all windows (restricted via `AppManifest::commands` in build.rs), remote URLs get access only via `remote.urls`; on Linux and Android an iframe is indistinguishable from a window. Capabilities do not protect against holes in Rust commands, overly broad scopes, or vulnerabilities in the WebView itself.
- CONFIRMED (https://v2.tauri.app/security/csp/): CSP is applied only if it is set in the config; Tauri itself adds a nonce/hash for local scripts and styles; recommended as strict as possible, without CDNs. 
- From a search (not a primary source, UNCERTAIN on the version): in May 2026 there was a fix after which the core started applying the ACL to IPC requests from remote origins even without AppManifest (source: the all-versions changelog page).
- Untrusted HTML/Markdown: there is no special official page (I did not find one). Practice: render MD to HTML and run it through a sanitizer (see item 2), a strict CSP (`default-src 'self'`, no `unsafe-inline` for script, `img-src` with `asset:`/`data:` as needed), do not give the window with user content any extra capabilities, open external links via the opener plugin rather than by navigating in the same webview.

---

## 2. Editor and Markdown

- Obsidian on CM6: CONFIRMED, quote from https://docs.obsidian.md/Plugins/Editor/Editor+extensions: "Obsidian uses CodeMirror 6 (CM6) to power the Markdown editor"; Live Preview is changed via editor extensions.
- `@codemirror/lang-markdown`: CONFIRMED, MIT; `markdownLanguage` includes GFM by default (+ subscript/superscript/emoji), `commonmarkLanguage` is strict; the parser is on Lezer. The GitHub repository is **archived** and moved to code.haverbeke.berlin (CodeMirror itself moved off GitHub); keep this in mind when tracking releases. https://github.com/codemirror/lang-markdown
- `@codemirror/view` is current: the changelog has a 6.43.0 entry from 2026-05-14; the forum mentions 6.43.1 (https://codemirror.net/docs/changelog).
- Live preview = decorations (replace/mark/widget) over the syntax tree; this is exactly Obsidian's path. This is also where the main IME risk is.
- IME / Cyrillic: I found no bugs specifically with Cyrillic (UNCERTAIN, there are no separate reports for the Russian keyboard layout; Russian input is ordinary character-by-character input without composition, IME problems mostly concern CJK and diacritics/dead keys). Confirmed IME problems in CM6: (a) in Safari 26.5 on 6.43.1, composition next to a widget can recreate `cm-widgetBuffer` (forum discuss.codemirror.net, the thread is still open); (b) a mark decoration wrapping several replace decorations breaks the selection during IME; (c) 6.39.16 fixed DOM corruption when decorations change right before composition ends. The author's position: you must not touch the DOM/selection during composition (otherwise composition is interrupted). Practice: do not change decorations during `view.composing`. Links: https://discuss.codemirror.net/t/ime-input-may-break-when-cursor-is-adjacent-to-widget-due-to-cm-widgetbuffer-reconstruction/9799
- Alternatives (one line each):
  - Milkdown: a WYSIWYG framework on ProseMirror + remark, MIT, ~12k stars (https://github.com/Milkdown/milkdown); I could not read the version.
  - ProseMirror: a low-level rich-text toolkit (the basis of Milkdown/TipTap), the document model is not the same as Markdown text, round-trip serialization is needed (my own knowledge).
  - Lexical (Meta): an editor framework, JSON/Markdown/HTML import/export, MIT, ~24k stars (https://github.com/facebook/lexical); also not text-first.
  - Conclusion: if "raw Markdown with live preview like in Obsidian" is the goal, then CM6; WYSIWYG models create a risk of the source Markdown diverging on round-trip (my judgment).
- Feed rendering:
  - markdown-it: active, but there are no Releases on GitHub (the page is empty; the repository mentions v15 and a migration guide); I could not confirm the exact version via npm (403). Extensions via plugins; "Safe by default" is claimed in the README.
  - unified/remark/micromark: the ecosystem is active (discussions in the micromark org up to July 2026); the AST (mdast/hast) is more convenient for custom nodes (tags, mentions, spoilers) and sanitization at the tree level. Exact remark versions not confirmed (the releases page shows remark-cli 12.0.0 / 12.0.1, that is the CLI).
  - Sanitization: rehype-sanitize CONFIRMED: works on an allow-list schema (GitHub-like `defaultSchema`), place it after the last unsafe step, prefixes `id`/`name` with `user-content-` against DOM clobbering, extending the schema extends the attack surface. https://github.com/rehypejs/rehype-sanitize
  - DOMPurify CONFIRMED: v3.4.16, actively maintained (recent GHSAs in the README, 3.4.12-3.4.14 are hardenings; security releases are announced only on the mailing list), tests on Chromium/Firefox/WebKit. https://github.com/cure53/DOMPurify Key point: DOMPurify by itself is "parse into DOM and clean", so for the attached Markdown inserts the pipeline must be "render -> DOMPurify -> innerHTML", not the other way round.
  - Recommendation: store the source Markdown, render it to HTML (markdown-it without raw HTML, `html:false`, OR remark without rehype-raw), then DOMPurify as a second layer; plus a strict CSP. This is advice, not a fact from the docs.

---

## 3. Feed virtualization in React

- TanStack Virtual: CONFIRMED (https://tanstack.com/virtual/latest/docs/api/virtualizer and /docs/chat): dynamic heights via `measureElement`, `scrollToIndex({align})`, `shouldAdjustScrollPositionOnItemSizeChange`; for chat/prepend there is `anchorTo: 'end'` (on prepend it remembers the visible element by key and corrects the offset), a stable `getItemKey` by id is needed, `followOnAppend`, `scrollToEnd`, `isAtEnd`. Versions as of 16 Oct 2026: `@tanstack/virtual-core 3.19.6`, `@tanstack/react-virtual 3.14.21`. The source does not say in which version `anchorTo` appeared (UNCERTAIN), so pin the version and test the "load more upwards" scenario on your own data. On iOS WebKit, scroll writes are deferred during touch/momentum (from the docs).
- virtua: CONFIRMED (https://github.com/inokawa/virtua): MIT, dynamic sizes are measured from the DOM, reverse scroll (the `shift` prop for prepend; the README talks about "scroll position adjustment while reverse scrolling"), smooth scroll, `scrollToIndex`; release 0.53.5 from 12 Oct 2026, active development by a single maintainer. The details of the prepend mechanics are not covered on the page (UNCERTAIN: need to read the docs/chat examples).
- react-virtuoso: licensing CONFIRMED (https://virtuoso.dev/pricing): free and MIT `Virtuoso`, `GroupedVirtuoso`, `VirtuosoGrid`, `VirtuosoTable`, `Masonry`; **only `VirtuosoMessageList` is commercial** ($168/seat/year or Pro $312/seat/year, dev mode 30 days without a license). So "the message list is commercial" is CONFIRMED. The plain `Virtuoso` with `firstItemIndex` (prepend) and `followOutput` (from my memory; the API page returned a redirect with no content) = UNCERTAIN, but this is the standard free path for chat.
- Recommendation: for a pet project without licenses, choose between TanStack Virtual (new `anchorTo`, headless, actively maintained) and virtua (simpler "out of the box", less code). For 100k messages the key thing is not the virtualizer but that data is loaded in pages (keyset pagination by `(ts,id)` from SQLite) and only a window is kept in the DOM.

---

## 4. Forest: rendering

- PixiJS v8: CONFIRMED active: v8.22.0 (1 Oct 2026), frequent releases (8.20-8.22 over August-October). WebGL renderers and optionally WebGPU. https://github.com/pixijs/pixijs/releases , https://pixijs.com/8.x/guides/getting-started/intro
- pixi-viewport: CONFIRMED v6.0.0 "moves pixi-viewport to pixi.js v8+"; has drag, pinch, wheel zoom, clamp. Activity not confirmed (the page showed no dates; 136 open issues): UNCERTAIN. https://github.com/pixijs-userland/pixi-viewport Alternative: write pan/zoom ourselves (camera = matrix, 3 gestures), that is ~100 lines.
- Suitability: ~372 tiles + sprites is a very light scene (my estimate; I did not get performance numbers from sources). WebGL is needed not for performance but possibly for effects (shaders, glow). For such a load, Canvas 2D (with an offscreen tile cache, `drawImage`, hit-testing on the isometric grid done mathematically) is usually enough (my judgment, UNCERTAIN without measurements).
- WebKitGTK caveat: CONFIRMED above (the silent slow WebGL path on Linux, the recommendation to have a non-WebGL fallback). Consequence: Canvas 2D as the main engine, or Pixi with `preference: 'webgl'` plus a fallback; Pixi v8 has a canvas-renderer fallback (my own knowledge, not checked in this session, UNCERTAIN).
- macOS WKWebView WebGL/WebGPU: not checked in this session (UNCERTAIN); WebGL on WebKit has worked for a long time.

---

## 5. Rust: SQLite, FTS5, Russian, vectors

- rusqlite: CONFIRMED v0.40.1 with bundled SQLite 3.53.2 (README). The `bundled` build always sets `-DSQLITE_ENABLE_FTS5` (I read libsqlite3-sys's build.rs): FTS5 is enabled. https://github.com/rusqlite/rusqlite
- A custom FTS5 tokenizer from rusqlite: the rusqlite feature list (docs.rs/crate/rusqlite/latest/features) has no `fts5`/`tokenizer`; there are `vtab` (read-only virtual tables), `loadable_extension`, `load_extension`. So there is no high-level tokenizer API. The path: either raw FFI (`fts5_api` via `sqlite3_prepare("SELECT fts5(?1)")` pointer-passing; my own knowledge, not verified), or a C extension. UNCERTAIN on the effort, but it is non-trivial.
- Built-in tokenizers (CONFIRMED, https://www.sqlite.org/fts5.html): `unicode61` (default, case-insensitive, correctly splits Cyrillic as L* letters, diacritics are removed only for Latin), `ascii`, `porter` (English only), `trigram` (substrings, `case_sensitive`/`remove_diacritics`, speeds up LIKE/GLOB; queries shorter than 3 characters find nothing in FTS, for LIKE with fewer than 3 characters it is a linear scan). The docs do not state whether `unicode61` folds case for Cyrillic; "case-insensitive" is claimed without a restriction to Latin (it works in practice, but check it with a test: UNCERTAIN).
- Russian morphology:
  - Option A (I recommend it): store **pre-stemmed text** in the FTS5 table (Rust `rust-stemmers`, Snowball; Russian is in the list of algorithms, CONFIRMED, https://github.com/CurrySoftware/rust-stemmers, commit freshness/version not confirmed), and stem the query with the same code; `unicode61` index. Pros: pure Rust, no FFI, cross-platform (including mobile). Cons: the column duplicates the text (better `content=''`/contentless-delete >=3.43.0 or external content and a separate stem column), highlighting/snippets on the original require extra work. My recommendation, not a fact.
  - Option B: the C extension `abiliojr/fts5-snowball` (CONFIRMED, BSD-3, `tokenize='snowball russian'`, the project is small, 22 commits, loading from code is "untested" per the README). For Tauri cross-platform and mobile - an extra C build.
  - Option C: trigram as an additional index for substrings/typos (semantics CONFIRMED). More expensive in index size (the source gives no numbers).
  - Combination: stem-FTS for the main search + trigram as needed.
- sqlite-vec: CONFIRMED (https://github.com/asg017/sqlite-vec): README "pre-v1, expect breaking changes"; the last stable release is v0.1.9 (31 Mar 2026), after that only alpha v0.1.10-alpha.1..4 (up to 18 May 2026); Rust crate `sqlite-vec`, registered via `sqlite3_auto_extension` (example with rusqlite bundled + zerocopy, https://alexgarcia.xyz/sqlite-vec/rust.html); license Apache-2.0/MIT; sponsor Mozilla Builders. KNN: `vec0` (faster/more compact) or a manual query with `vec_distance_cosine()` = brute-force (CONFIRMED). There are no benchmark numbers in the docs; the repo has ivf/diskann files (a sign of ANN work, without explanations - UNCERTAIN).
- Brute-force cosine over f32 blobs: memory arithmetic (mine): 100 000 x 1024 x 4 B = ~410 MB; x1536 = ~614 MB; x3072 = ~1.23 GB; at 256 dimensions = ~102 MB. OpenAI `dimensions` allows shortening the vector (CONFIRMED, https://developers.openai.com/api/docs/guides/embeddings; with manual truncation you need to re-normalize). I did not measure scan time and there are no numbers from sources - it needs to be measured with a prototype. For 100k it is reasonable to: store normalized vectors (cosine = dot product), keep the matrix in memory in Rust (mmap/Vec<f32>/or f16/int8 quantization), scan with SIMD; sqlite-vec is needed only if SQL joins with metadata are wanted. Start with brute-force in Rust: no dependency on a pre-v1 extension.

---

## 6. OpenRouter embeddings

- CONFIRMED endpoint: `POST https://openrouter.ai/api/v1/embeddings`; model list: `GET https://openrouter.ai/api/v1/embeddings/models`. https://openrouter.ai/docs/api-reference/embeddings
- OpenAI compatibility: the docs call the API as a whole "OpenAI-compatible with additional OpenRouter features"; the response shape (`object:"list"`, `data[].embedding`, `usage`) matches OpenAI; there is no direct compatibility statement for this particular endpoint (UNCERTAIN, but in practice the OpenAI SDK with `base_url` usually works - not verified). Fields: `model`, `input` (string/array/multimodal `content` objects), `dimensions`, `encoding_format` (`float`|`base64`), `input_type` (`search_query`/`search_document`), `user`, `provider` (`order`, `only`, `ignore`, `sort`, `zdr`, ...). https://openrouter.ai/docs/api/api-reference/embeddings/create-embeddings
- Models (CONFIRMED from the live API catalog as of 2026-10-19, 33 models; price = per token x 1e6 = per 1M tokens, conversion mine):
  - openai/text-embedding-3-small: $0.02/1M, ctx 8192 (1536 dim by default, from the OpenAI docs)
  - openai/text-embedding-3-large: $0.13/1M, ctx 8192 (3072 dim)
  - baai/bge-m3: $0.01/1M, ctx 8194, 1024 dim, 100+ languages, MIT (from HuggingFace)
  - intfloat/multilingual-e5-large: $0.01/1M, ctx 512
  - qwen/qwen3-embedding-8b: $0.01/1M, ctx 40960; qwen3-embedding-4b: $0.02/1M, ctx 32768. The model `qwen3-embedding-0.6b`, mentioned in the docs as an example, is not in the live list (UNCERTAIN, it may have been renamed/withdrawn).
  - voyageai/voyage-4 / -4-lite / -4-large: $0.06 / $0.02 / $0.12 per 1M; google/gemini-embedding-001: $0.15, ctx 20000; mistralai/mistral-embed-2312: $0.10.
  - Free: nvidia/nemotron-3-embed-1b:free, nvidia/llama-nemotron-embed-vl-1b-v2:free (text+image), liquid/lfm-2.5-embedding-350m:free.
  - Multilingual with Russian: bge-m3, multilingual-e5-large, qwen3-embedding (claimed by the developers; I did not check Russian metrics in this session: UNCERTAIN); text-embedding-3 are multilingual (common knowledge, not verified with a benchmark).
- Multimodal (images): CONFIRMED in the catalog: `google/gemini-embedding-2` (+`-preview`; text, image, file, audio, video; ctx 8192; $0.20/1M), `voyageai/voyage-multimodal-3.5` (text, image; $0.12/1M), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (text, image). Inserting an image: `image_url` in the `content` array, can be combined with `text`. Nuance: the vector spaces of different models are incompatible, so choosing a model fixes the schema (store `model` and `dim` next to the vector).
- Cost ballpark (my arithmetic, assuming 100 tokens per message, not from a source): 100k x 100 = 10M tokens, 10M x $0.02/1M = $0.20 for 3-small, $1.30 for 3-large, $0.10 for bge-m3. The real message length is unknown.
- Privacy: CONFIRMED (https://openrouter.ai/docs/guides/privacy/data-collection and .../features/zdr): by default OpenRouter does not store prompts/responses without an explicit opt-in, but stores metadata (tokens, latency); prompt logging and use of data for improvement are opt-in; ZDR is enabled (1) in the privacy settings (per model group), (2) via the `enforce_zdr_*` guardrail, (3) per request `provider: {"zdr": true}` (OR logic, something already enabled cannot be turned off). ZDR covers inference routing and does not cover plugins/tools. The example in the embeddings docs also has `provider.data_collection: "deny"`; it is not described on the privacy page (UNCERTAIN in the details, but the field is in the example). Bottom line: for personal notes, enable ZDR in the account and/or `zdr:true`; note that with ZDR the set of available providers/models narrows (my assumption, to be verified).

---

## 7. Sync (our own server in Rust)

Statuses (CONFIRMED from repositories/releases where indicated):
- Custom op-log + HLC: a pattern, not a library. The Rust crate `uhlc` 0.9.0 (12 Jan 2026; used in Eclipse Zenoh; the license is not shown on the docs.rs page) https://docs.rs/crate/uhlc/latest . HLC gives monotonic timestamps that account for clock drift (it rejects remote timestamps that are too far "in the future").
- Automerge: CONFIRMED active, v3.5.0 (16 Sep 2026), many releases in summer 2026. A document-oriented CRDT with full history; for "100k independent messages" its model is rather heavy (one document per message or one shared document - both are awkward; my judgment). https://github.com/automerge/automerge/releases
- Loro: CONFIRMED `loro-crdt 1.16.4` (30 Sep 2026), Rust+WASM; strengths - rich text/tree/movable list; overkill for LWW messages.
- Yjs/yrs: yrs is the Rust port of Yjs, the README has a parity table (yrs 0.21 per the table; whether the version is current is not confirmed). Good for collaborative editing of text inside a message, not for a relational set of messages.
- cr-sqlite (vlcn): the repository is not archived; commits on 4-10 Aug 2026 (builds: iOS sim, Windows arm64, Android ABIs, macOS headerpad) - it is maintained, but the focus is on building the loadable extension; the README says inserts into a CRR are 2.5 times slower than ordinary ones, the "causal event log" is only in the v2 plans, some CRDTs (counter, rich text) are "still being implemented". I did not confirm the latest release/version. UNCERTAIN on maturity; it drags a C extension into Tauri (including mobile) and ties us to the cr-sqlite schema.
- ElectricSQL: CONFIRMED, "read-path sync engine for Postgres" (shapes, needs Postgres with logical replication), Apache-2.0; writes go separately through your backend. Not suitable: we have SQLite + a Rust server, not Postgres. https://github.com/electric-sql/electric
- PowerSync: README: sources Postgres/MongoDB/MySQL/SQL Server; the service is self-hosted in Docker (`journeyapps/powersync-service`), there is Enterprise Self-Hosted; a Rust/Tauri SDK is not listed in the JS SDK repository. Not suitable as a lightweight option (needs an external source DB + a service). I did not confirm the Open Edition license terms (UNCERTAIN).
- Turso/libSQL: CONFIRMED, Turso Database (a Rust rewrite of SQLite) has not reached 1.0, some features are experimental, it is announced as the replacement for libSQL. Turso Sync: conflict strategy "last push wins", the docs assume Turso Cloud; self-hosting the sync server is not mentioned in the docs. Not suitable. https://docs.turso.tech/sync/usage
- What fits "messages with a Markdown body, LWW per message + revision history": **our own op-log with HLC**. Every edit = an immutable record (message_id, rev_id/hlc, device_id, op type, body/metadata payload); the message state = the record with the maximum HLC (LWW), all revisions are kept as history (they naturally go into the `message_revisions` table), deletion = a tombstone with an HLC. The server is an append-only log + a `since_hlc`/seq cursor. CRDTs would be needed only if automatic merging of concurrent edits within one text is required (not needed with LWW). This is my conclusion from the requirements, supported only by the status descriptions above. Attachments: content-addressable blobs (sha256), synced separately from the log.

---

## 8. Claude Design and design tokens

- What it is: CONFIRMED (https://www.anthropic.com/news/claude-design-anthropic-labs): an Anthropic Labs product, released **17 April 2026**, on Claude Opus 4.7, in research preview for Pro/Max/Team/Enterprise (at the time of the announcement). It makes designs, interactive prototypes, slides, one-pagers, landing pages, wireframes. Export: Canva, PDF, PPTX, standalone HTML, folder/zip, internal URL; a handoff bundle for Claude Code.
- Current state (help center, https://support.claude.com/en/articles/14604416-get-started-with-claude-design): Export: .zip, PDF, PPTX, Google Slides, standalone HTML, third-party tools (Lovable, Replit, v0, Vercel, Netlify and others), handoff to a local coding agent or Claude Code on the web; in Claude Code the `/design` command imports a design into the codebase or exports code as a live prototype. Enabled by default for Free/Pro/Max/Team/Enterprise (Enterprise - since 15 Oct 2026); the separate claude.ai/design is closing on **14 December 2026**. Limitations: no version history, basic multi-user editing.
- Design systems: CONFIRMED (https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design): created from code (link/upload), files, Figma (as input), decks, logos, fonts; the output is a palette, typography, components, layout patterns (for migrated ones: guide, tokens, components); the system is stored as an artifact available in any chat and in Claude Code; beta. `/design-sync` in Claude Code reads tokens and components from React code (works best if the design system is already in React). **Exporting a design system as a file/to another tool is not described in the article.**
- Figma: not available as an output (in the help center Figma is only an input; third-party blogs contradict each other, do not use them as a source).
- How to bring it into a React codebase: (1) a handoff bundle (design files, the chat, a README with instructions for the model) -> paste the prompt into a local Claude Code, it implements the components; (2) `/design` in Claude Code; (3) `/design-sync` in the opposite direction (code -> Design). So tokens/components will get into our codebase as agent-generated code, not as a separate standardized export. Recommendation: keep the tokens ourselves in DTCG JSON in the repository (the source of truth), and use Claude Design as an exploration/prototyping layer.
- Not covered by a BAA (the BAA help article lists Claude Design as not covered; from a search, I did not open the primary page: UNCERTAIN).
- W3C DTCG: CONFIRMED https://www.designtokens.org/tr/drafts/format/ - this is a report of the Design Tokens Community Group (not a W3C Standard); the latest published version is **2025.10** (designtokens.org/TR/2025.10/format/), the draft page dated 08.09.2026 warns "Do not attempt to implement this version". Types: color, dimension, fontFamily, fontWeight, duration, cubicBezier, number + composite ones (shadow, border, typography, gradient...). The page itself does not confirm 2025.10's stability as "the first stable" one (UNCERTAIN).
- Style Dictionary: CONFIRMED v5.7.0 (16 Oct 2026), active releases (5.4.4 - 5.7.0 from June to October); "first-class DTCG support since v4", but according to the page itself there is no full 2025.10 support yet ("work in progress in v5"); 5.7.0 added the DTCG type `duration`. There were prototype pollution security patches (5.4.4, 5.5.1, 5.5.5) - keep updating. Built-in Swift/Kotlin formats are not confirmed on the pages I opened (UNCERTAIN; the `ios-swift`/`android` platforms exist in SD to my knowledge). For CSS variables now (`css/variables`) - fine; Swift/Kotlin later - via SD platforms or custom formats. https://styledictionary.com/info/dtcg/ , https://github.com/style-dictionary/style-dictionary/releases

---

## 9. Codex image generation

- The real feature: CONFIRMED (https://learn.chatgpt.com/docs/image-generation; developers.openai.com/codex/image-generation redirects there): the built-in image generation uses **`gpt-image-2`**; in Codex CLI it is invoked in natural language or explicitly with `$imagegen`; a reference image is attached with `-i/--image`; also available in ChatGPT desktop/web and the IDE extension.
- Limits: generation counts toward the general Codex limits, consuming them "3-5 times faster on average" than similar turns without an image (depends on quality and size); for large batches they advise `OPENAI_API_KEY` and API billing.
- The `imagegen` skill (CONFIRMED, https://raw.githubusercontent.com/openai/skills/main/skills/.system/imagegen/SKILL.md): by default the built-in `image_gen` without an API key, saves to `$CODEX_HOME/generated_images/...`; the CLI fallback `scripts/image_gen.py` (generate/edit/generate-batch) only on explicit request and with `OPENAI_API_KEY`; built-in edits only on images already in the conversation (local files via `view_image`), masks and `input_fidelity`/`quality` only in the CLI; batch = one `image_gen` call per asset; consistency recommendations: list the invariants ("change only X, keep everything else"), one edit per iteration, refer to input images by index with a role.
- Transparent background: the Codex page is silent about it. In the API guide (https://developers.openai.com/api/docs/guides/image-generation) transparency is `background:"transparent"` + `output_format` png/webp, but the models mentioned are `gpt-image-2.5-sunburst`/`gpt-image-2.5-flare`, not `gpt-image-2`; transparency support specifically in `gpt-image-2` and in the built-in tool is UNCERTAIN. For sprites this is critical: plan a background removal step (the `background-extraction` option from the skill or an external tool).
- Set consistency: CONFIRMED official caveat of the API guide - the model "may occasionally struggle to maintain visual consistency for recurring characters or brand elements" across generations. For a set of trees: one master style reference, make one change at a time, generate a sheet of several variants at once, fix the palette/angle (2:1 isometry) in the prompt. API sizes: multiples of 16, aspect ratio 1:3..3:1, edge up to 3840 px, 655,360-8,294,400 pixels in total (>2560x1440 experimental). The model does not guarantee exact isometry/a pixel grid (my judgment, check by hand).
- Note: blog sources mention the launch of gpt-image-2 on 21 April 2026 - a secondary source only.

---

## Stack recommendations

1. Shell: Tauri **2.12.x** (stable), not v3 alpha. macOS first (WKWebView) - low risk. Windows (WebView2) - low. Linux (WebKitGTK) - medium/high risk: plan a test run on NVIDIA+Wayland and a fallback without WebGL.
2. Data: rusqlite (`bundled`) in the Rust core, the frontend communicates only through typed commands/events; no tauri-plugin-sql. Migrations - our own (refinery/`user_version`).
3. Search: FTS5 `unicode61` + a pre-stemmed column (Snowball Russian via `rust-stemmers`), optionally trigram for substrings. Do not pull in a C tokenizer until we hit a wall.
4. Semantics: OpenRouter `/api/v1/embeddings`, default model `baai/bge-m3` (cheap, 1024 dim, 100+ languages) or `openai/text-embedding-3-small` (1536, or `dimensions`=512/768 to save); store `model`+`dim` alongside; vectors as normalized f32 BLOBs; brute-force search in Rust. Enable ZDR. Multimodal (images) - later via `google/gemini-embedding-2`/`voyage-multimodal-3.5`, but that is a separate vector space.
5. Editor: CodeMirror 6 + `@codemirror/lang-markdown` + our own decorations for live preview; do not change decorations during `view.composing`; IME tests on macOS Safari/WKWebView and Windows.
6. Feed: TanStack Virtual (`anchorTo:'end'`, `getItemKey` by id) or virtua; keyset pagination from SQLite in windows. Do not take `VirtuosoMessageList` (paid).
7. Markdown: remark/micromark (or markdown-it) without raw HTML -> DOMPurify (+rehype-sanitize if unified) -> strict CSP.
8. Forest: Canvas 2D (hand-written camera and hit-test) as the baseline, PixiJS v8 as an upgrade if effects are needed; on Linux, the non-WebGL path by default or an automatic FPS benchmark at startup. Sprites: Codex `$imagegen` (gpt-image-2) + background post-processing.
9. Sync: our own op-log with HLC (`uhlc`), LWW per message, a revisions table, tombstones, blobs by sha256; a Rust server as an append-only log with a cursor. Do not take CRDT/cr-sqlite/Electric/PowerSync/Turso at the start.
10. Design: tokens in DTCG JSON in the repository -> Style Dictionary 5.x -> CSS variables now, Swift/Kotlin later; Claude Design as a source of ideas/prototypes with handoff via Claude Code, not as the source of truth.
11. Updater: tauri-plugin-updater with a static JSON/our own endpoint on the home server; sign, keep the private key outside `.env`.

## Top risks

1. **Linux WebKitGTK**: DMABUF/NVIDIA crashes, "silent" slow WebGL, font differences and fractional scaling. Mitigation: Canvas 2D/fallback, a test matrix, if necessary a deferred move to the CEF runtime of Tauri v3 (alpha for now).
2. **IME/live preview in CM6** (Safari/WKWebView): composition bugs next to widget decorations are confirmed; Cyrillic is usually simpler than CJK, but it has to be checked by hand.
3. **Tauri mobile**: the team itself admitted immature DX and an incomplete set of plugins; Share Extension/widgets - manual native code in the regenerated `gen/apple`. Do not promise mobile early.
4. **Embeddings pipeline**: changing the model = recomputing all vectors; dependency on OpenRouter (privacy - ZDR; prices/catalog change, e.g. `qwen3-embedding-0.6b` from the docs is missing from the live list); in-memory size for brute-force (~0.4-1.2 GB at 1024-3072 dim for 100k).
5. **Sync design**: a hand-written op-log means taking responsibility for correctness (HLC drift, tombstones, log compaction, attachments). Property tests and a simulation of several devices are needed.
6. **Pre-v1/poorly maintained dependencies**: sqlite-vec (pre-v1, stable release 0.1.9 from March, alpha after that), pixi-viewport (activity not confirmed), fts5-snowball (a small project), lang-markdown (repo archived/moved), Tauri v3 as a future migration.
7. **XSS/IPC surface**: user Markdown + Tauri IPC; all commands are available to all windows by default - narrow `AppManifest::commands`, strict CSP, a sanitizer, do not open external links in the same webview.
8. **Claude Design**: no export of the design system as a file/tokens and no Figma output (per the help center), the product is in beta with a standalone shutdown date of 14.12.2026; do not build the critical path on it.
9. **Sprite generation**: style inconsistency across generations (official caveat), unclear transparent background support in the built-in `$imagegen`, Codex limits consumed 3-5 times faster.

## Main sources
- https://github.com/tauri-apps/tauri/releases , https://v2.tauri.app/reference/webview-versions/ , https://v2.tauri.app/develop/debug/linux-graphics/ , https://v3.tauri.app/develop/cef/ , https://v2.tauri.app/blog/tauri-20/ , https://v2.tauri.app/security/capabilities/ , https://v2.tauri.app/security/csp/
- https://v2.tauri.app/plugin/{global-shortcut,clipboard,single-instance,updater,sql}/ , https://v2.tauri.app/learn/system-tray/
- https://docs.obsidian.md/Plugins/Editor/Editor+extensions , https://github.com/codemirror/lang-markdown , https://codemirror.net/docs/changelog
- https://github.com/cure53/DOMPurify , https://github.com/rehypejs/rehype-sanitize
- https://tanstack.com/virtual/latest/docs/chat , https://github.com/inokawa/virtua , https://virtuoso.dev/pricing
- https://github.com/pixijs/pixijs/releases , https://github.com/pixijs-userland/pixi-viewport
- https://github.com/rusqlite/rusqlite , https://www.sqlite.org/fts5.html , https://github.com/asg017/sqlite-vec , https://alexgarcia.xyz/sqlite-vec/rust.html
- https://openrouter.ai/docs/api-reference/embeddings , https://openrouter.ai/api/v1/embeddings/models , https://openrouter.ai/docs/guides/features/zdr
- https://github.com/automerge/automerge/releases , https://github.com/loro-dev/loro/releases , https://github.com/vlcn-io/cr-sqlite , https://github.com/electric-sql/electric , https://docs.turso.tech/sync/usage , https://docs.rs/crate/uhlc/latest
- https://www.anthropic.com/news/claude-design-anthropic-labs , https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design , https://support.claude.com/en/articles/14604416-get-started-with-claude-design
- https://www.designtokens.org/tr/drafts/format/ , https://styledictionary.com/info/dtcg/ , https://github.com/style-dictionary/style-dictionary/releases
- https://learn.chatgpt.com/docs/image-generation , https://raw.githubusercontent.com/openai/skills/main/skills/.system/imagegen/SKILL.md , https://developers.openai.com/api/docs/guides/image-generation
