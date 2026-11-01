# Kinige v2

A personal journal shaped like a "chat with yourself": you write as in a messenger, reread it as a typeset journal, and see the year as an isometric forest. Desktop first (macOS → Windows/Linux) on Tauri 2 + Rust + React/TypeScript, SQLite on the client, its own self-hosted server (axum + Postgres) with end-to-end encryption.

## Source of truth

- **`docs/plan/02-prd.md` is the only normative document.** If the code, the review (`01-review.md`) or the research (`research/`) disagree with the PRD, the PRD wins. Decisions change only through an edit to the PRD or an ADR.
- `docs/adr/NNNN-*.md` — decisions (half a page, written after the decision is made).
- `docs/slices/<id>-<name>.md` — one-page slice specification (§12.1a).
- `docs/screens/<slice>/` — screenshots of the real window for acceptance.

Status: slice 0 (skeleton), D1 (design system foundation) and ADR-0001 (protocol + keys) are done; slice 1 (Inbox) is merged; slice 2 (quick capture: window, hotkeys, tray, appending) was accepted by the user on 27.10.2026; slice 3 (chats, entry actions, trash, database backup) and slice 4 (import from Telegram, archive, search with word forms) were accepted on 28.10.2026; slice 4a (MCP server for AI agents) was accepted on 29.10.2026; slice 4b (settings window, settings.json) was accepted on 31.10.2026; slice 5 ("On this day", replies) was accepted on 01.11.2026. Build and test commands are in the "Build and tests" section below.

## How we work (mandatory, PRD §12)

1. **One slice = one branch = one PR.** Before starting: a one-page slice specification — the scenario in words, what is NOT included, what "done" looks like. We don't start without it.
2. **Agent roles** (`.claude/agents/`):
   - `slice-spec` — writes the slice specification from the PRD;
   - `slice-implementer` — implements the slice from the specification;
   - `slice-reviewer` — reviews in a clean context: runs the checks and builds the app itself, but does not launch its window on the user's Mac; takes the screenshots of the real window from the macOS runner (`screens.yml`);
   - `design-system` — tokens, components, Storybook;
   - `sync-crypto-guardian` — review of everything that touches the data schema, sync and encryption;
   - `codex-second-opinion` — a second opinion from Codex CLI; every piece of advice is verified.
   The agent that writes the code never reviews itself.
3. **"Done" only with evidence:** test output + screenshots of the real window in `docs/screens/<slice>/`. The next slice starts only after acceptance by the user.
4. **Visuals come only from design system tokens.** A new color, spacing or shadow first appears in `design/tokens/`.
5. **Don't invent facts** about libraries, APIs and numbers: verify them (documentation, source code) or write "not verified".
6. **No abstractions or documents "for the future".** A file over 400 lines is a signal to split it (except migrations and generated types).
7. **A slice's budget is in days.** If you exceed twice the estimate, stop and report instead of pushing on.
8. **No edits to source files with string-replacement scripts.** Only targeted edits.
9. **The repository is public.** Demo and test data are synthetic only; no personal entries, exports, paths or screenshots with the user's data.
10. **Any asset** (font, icon, image, emoji data, palette) only with a license that permits redistribution, and with a line in `ATTRIBUTION.md`. If the origin is unclear, don't add it.
11. **Programmatic calls** (MCP, automation, IPC tests) never open dialogs or windows — they only return a result or an error.
12. **Every fix for a crash or data loss comes with a regression test.** A new parser or IPC command comes with a test on garbage input.
13. **Parallel agents:** each has its own worktree and its own `CARGO_TARGET_DIR`, edits only its own files, and deletes its target after the work (the disk is small). Don't fix someone else's edits that break the build: wait or report. Rewrite `Cargo.toml` and other manifests atomically (a temporary file outside `crates/`, then `mv`), so that a broken manifest doesn't break the neighbors' build.
14. **Network requests** from code, scripts and agents (downloading assets, APIs) use the anonymized User-Agent `kinige-dev`. A name, e-mail or other personal data never goes into headers, URLs or request bodies.
15. **The PR title and the message of the branch's first commit carry the Huly task number** (`KINIG-N: …`), so that Huly links the PR to the task. Exception: the release commit and PR `Release vX.Y.Z` from `scripts/release.sh` (ADR-0012) go without a number.
16. **Repository language is English:** docs, specs, ADRs, code comments, log messages, commits and PRs. UI strings are bilingual (ru/en) via i18n.

## Code style

Applies to new and edited code; don't rewrite existing code en masse.

1. **Logging only through `log`** (`log::{error,warn,info,debug}!`). `println!`/`eprintln!` outside tests, examples and spikes are forbidden (workspace lints `clippy::print_stdout` and `clippy::print_stderr` = `deny`). The log file is in the OS log directory (macOS: `~/Library/Logs/me.savvinov.kinige/kinige.log`, rotation 5 × 1 MB); the default level is `info`, `KINIGE_LOG=debug` turns on details for Kinige code. **Entry texts, chat names, paths to the user's files and import contents are never written to the log** — only event kinds and error categories (`StoreError::category`).
2. **Order within a file: important things on top, details below.** Constants after imports; then types (outer structs above nested ones; API types in the order request → response → error); in an `impl` — constructors, public methods, private helpers below the methods that call them; in test modules — constants, tests, helpers at the end.
3. **Imports through `use`, not full paths in code.** A full path only for a one-off case or on a clash with `std`. If two crates export the same name, use an alias with the crate suffix: `use foo::Bar as BarFoo;`.
4. **Markdown tables are aligned with spaces to the column width:** they are often read raw (terminal, diff). Applies to new and edited tables; don't reformat old ones en masse.
5. **Don't delete the user's untracked and ignored files.** You may clean up only what you created yourself; caches only with consent.
6. **Time and dates through `chrono`,** without hand-written calendar arithmetic.
7. **Never panic** (ADR-0011): `unwrap`, `expect`, `panic!`, `unreachable!`, indexing and string slicing in app code are forbidden by workspace lints (`clippy.toml` allows them only in tests); the exception is a provable literal with `#[allow]` and a reason. Numbers from input go through `get()`/`checked_*`; mutexes through `unwrap_or_else(PoisonError::into_inner)`.

## Sensitive areas

- SQLite schema, sync protocol, envelope format, keys — PRD §4 and §10. Any change here goes through `sync-crypto-guardian` before merge.
- Home-made cryptography is forbidden: only libsodium / RustCrypto and the scheme described in the PRD.
- Tauri commands: each window has its own set of allowed commands (§12.3), with negative tests.

## Build and tests

Layout: `crates/kinige-domain` (rules, no IO, builds for wasm32), `crates/kinige-store` (SQLite, migrations, single writer, Markdown copy), `src-tauri/` (thin Tauri 2 layer), `web/` (React + TS + Vite + Storybook), `design/tokens/` (DTCG), `scripts/`, `.github/workflows/ci.yml`. Rust and Node versions are pinned in `rust-toolchain.toml` and `.node-version`. Details and the startup measurement method are in `README.md`.

Rust (from the root):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — also regenerates the TS types in `web/src/api/generated/` (ts-rs, ADR-0002); after changing types in Rust, commit the result
- `cargo build -p kinige-domain --target wasm32-unknown-unknown`

Frontend (from `web/`, after `npm ci`):
- `npm run tokens` — rebuild `src/ui/tokens.css` and `src/ui/themes.css` from `design/tokens/**/*.json` (runs by itself before `dev`, `build`, `storybook`); the result is committed
- `npm run typecheck`, `npm run lint`, `npm test` (vitest; includes the check "no raw colors and sizes outside tokens.css / themes.css", the contrast of all "color / background" pairs of the palette, and the font check against the woff2 files themselves)
- `npm run build-storybook`

Storybook visual tests run only on GitHub, in a Playwright container: the comparison is the `visual` job in CI; new baselines via `gh workflow run visual-update.yml --ref <branch>` (captures them and commits to the branch; not allowed in `main`; after the commit it starts CI on the branch by itself). Baselines are not captured on macOS or on ubuntu-server.

App: `scripts/run-app.sh` (dev), `scripts/storybook.sh`, `scripts/build-app.sh` (release `.app` + ad-hoc signature + `codesign --verify` + check of the bundle version against `Cargo.toml`; `KINIGE_BUNDLES=app,dmg` adds a `.dmg`), `scripts/screenshot-window.sh <out.png>` (the Kinige window, `screencapture -x -o -l`; for manual debugging and for `screens.yml` itself; agents don't run it on the user's Mac; nothing but the window may get into the screenshot), `scripts/measure-cold-start.sh`, `scripts/mcp-session.py <kinige-mcp> <socket> <protocol file>` (an MCP session through the bridge with our own client, for the acceptance of slice 4a; Claude configs are not touched). Build and run put the bridge into `src-tauri/binaries/kinige-mcp-<triple>` (sidecar, `bundle.externalBin`); on `cargo clippy`/`cargo test` on a clean clone, an empty stub from `build.rs` sits there.

Versions and releases (ADR-0012): semver, minor = slice number (0.5.0 — slices 0–5), patch — fixes between slices, 1.0.0 — after sync. The version lives in `[workspace.package]` of `Cargo.toml` (`tauri.conf.json` has none of its own); the other files and `web/src/version.test.ts` repeat and check it — don't change them by hand. Only the coordinator cuts a release, after acceptance of a slice or a batch of fixes: `scripts/release.sh X.Y.Z` (branch `release/vX.Y.Z`, commit, PR) -> CI and merge -> `scripts/release.sh --tag X.Y.Z` (tag `vX.Y.Z` on `main`, only with a green `ci.yml` on that commit) -> the `release.yml` workflow builds the `.dmg` and `.zip` on a macOS runner and creates a GitHub Release; progress: `gh run list --workflow release.yml`. Rerun for a tag: `gh workflow run release.yml -f tag=vX.Y.Z` (`-f publish=false` — build only). A tag is never moved or deleted: a faulty release is fixed by the next patch.

Screenshots of the real window for acceptance are captured not by this Mac but by a GitHub macOS runner: `gh workflow run screens.yml --ref <branch> -f slice=<id>` builds the `.app`, runs `scripts/screens/<slice>.sh` on synthetic data (temporary `KINIGE_DATA_DIR` and `KINIGE_MIRROR_DIR`) and commits `docs/screens/<slice>/` to the branch (after that, `git pull`; the workflow starts CI on the branch by itself). Input in the scenarios is accessibility actions only (`scripts/screens/ax.js`, shared functions in `scripts/screens/lib.sh`), no keys or mouse; the workflow doesn't run in `main`.

CI (GitHub Actions, `.github/workflows/ci.yml`; status and rerun via `gh run list`, `gh run rerun`) repeats everything listed above except building the `.app`, and adds: the `macos` job (`cargo clippy -p kinige --all-targets -- -D warnings` and `cargo test -p kinige` on `macos-15`: code under `cfg(target_os = "macos")`), `gitleaks` (false positives go to `.gitleaksignore`) and `deny` (`cargo-deny`: licenses, RustSec, sources — `deny.toml`). Dependencies are updated by Dependabot (weekly, minor updates as a group); third-party actions are pinned by SHA with a version comment. A "green CI" can't be checked locally; the closest equivalent is the commands above.

## Environment

- Docker on this Mac is remote (ubuntu-server, `DOCKER_HOST` is already set): bind mounts of Mac paths don't work, ports are reachable at `192.168.1.61:<port>`. Name your containers with the `kinige-` prefix and clean up after yourself.
- Codex CLI: `codex exec --skip-git-repo-check --ephemeral -s read-only -C <dir> -o <file> -` (prompt via stdin).
