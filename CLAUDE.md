# Kinige

A personal journal shaped like a "chat with yourself": you write as in a messenger, reread it as a typeset journal, and see the year as an isometric forest. Desktop first (macOS → Windows/Linux) on Tauri 2 + Rust + React/TypeScript, SQLite on the client, its own self-hosted server (axum + Postgres) with end-to-end encryption.

## Source of truth

- **`docs/plan/02-prd.md` is the only normative document.** If the code, the review (`01-review.md`) or the research (`research/`) disagree with the PRD, the PRD wins. Decisions change only through an edit to the PRD or an ADR.
- `docs/adr/NNNN-*.md` — decisions (half a page, written after the decision is made).
- `docs/slices/<id>-<name>.md` — one-page slice specification (§12.1a).
- `docs/screens/<slice>/` — screenshots of the real window for acceptance.

**Status:**
- **Accepted by the user:** slices 0–7a. The last one was 7a (voice notes) on 05.11.2026.
- **Merged:** the sync spikes S4a–S4c (server, ADR-0017–0020) and tech debt E.
- **Next:** slice 11, sync in the app.
- **History:** the PRD changelog (`docs/plan/02-prd.md`, top).
- **Build and test commands:** the "Build and tests" section below.

## How we work (mandatory, PRD §12)

1. **One slice = one branch = one PR.** Before starting: a one-page slice specification — the scenario in words, what is NOT included, what "done" looks like. We don't start without it.
2. **Agent roles** (`.claude/agents/`):
   - `slice-spec` — writes the slice specification from the PRD;
   - `slice-implementer` — implements the slice from the specification;
   - `slice-reviewer` — reviews in a clean context: runs the checks and builds the app itself, but does not launch its window on the user's Mac; takes the screenshots of the real window from the macOS runner (`screens.yml`);
   - `design-system` — tokens, components, Storybook;
   - `sync-crypto-guardian` — review of everything that touches the data schema, sync and encryption;
   - `codex-second-opinion` — a second opinion from Codex CLI; every piece of advice is verified.
   The agent that writes the code never reviews itself. Each definition holds its role's rules and its answer format: reviewers and the guardian answer `ACCEPT / REWORK / REJECT`.
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

**The harness enforces part of this** (`.claude/settings.json`, KINIG-110), for the coordinator and every subagent:
- **`.claude/hooks/guard.py` blocks:**
  - `sed -i`, `perl -i`, shell redirects and `tee` into sources, and inline scripts that write repository files (rule 8);
  - the user's real data paths;
  - `open`, `osascript`, screenshots, `killall` and system settings on this Mac;
  - writes to Claude configs;
  - pushes to main and force pushes, and a push in which gitleaks finds a secret;
  - docker removal of objects not named `kinige-…`, and `prune`;
  - edits by a subagent in the main checkout, and git there that changes anything (rule 13); `slice-spec` may write `docs/slices/`;
  - cargo builds with less than 6 GiB free.
- **The guard also reports** a source file that a Bash command created or rewrote without a tool that legitimately writes sources, a script edit committed in the same command included.
- **The guard is a backstop, not a sandbox.** Variables, `eval` and copies of prepared files (`cp`, `mv`) get past the checks before a command; the snapshot after it catches most of them.
- **A blocked call says why and what to do instead.** A reviewer's fault-injection probe goes through `git apply` of a patch in its own worktree.
- **`scripts/pre-pr-check.py`** checks a branch before its PR: files grown past 400 lines, backup files, invisible characters, unaligned tables, whitespace, rustfmt.
- **Tests:** `python3 -I .claude/hooks/tests/test_hooks.py` and `python3 -I scripts/pre-pr-check.test.py`, both run in CI's `web` job.
- **Progress file:** the coordinator's `claude-progress.md` at the repository root (ignored by git) is re-injected at session start and after compaction.

**Every subagent works like this.** Prompts don't repeat these rules.
- **Worktree.** Work in your own worktree.
  - The definitions set `isolation: worktree`. It starts from `origin/main`, so `git fetch` and switch to the branch you were given.
  - Set `CARGO_TARGET_DIR` inside the worktree and delete it before you finish (rule 13).
  - Don't touch files outside your worktree, except your scratch directory.
- **The user's Mac.** Don't launch the app, take screenshots, simulate input or global hotkeys, open the user's data, or change Claude configs and register kinige-mcp.
  - GUI checks run on GitHub runners: see the `screens` skill.
- **Docker.** It runs only on ubuntu-server. Name your objects `kinige-…` and remove what you created.
- **Commits.** Commit with the repository's git identity, run gitleaks before a push, and never push to `main`.
- **CI.** Wait for a run with one call: `gh run watch <id> --exit-status` as a background command, or the Monitor tool. Never use `sleep` loops.
  - If a run is stuck or needs an approval, say so in your answer instead of waiting.
- **Reading.** Read the sections you need. ADR-0001 has an index at its top; the PRD's sections are numbered.
  - Don't re-check facts the prompt already gives (free disk, branch, base commit).
- **Answers.** Give facts with the commands and their output. Say plainly what is not done or not verified.

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

Layout: `crates/kinige-domain` (rules, no IO, builds for wasm32), `crates/kinige-store` (SQLite, migrations, single writer, Markdown copy), `crates/kinige-crypto` (envelope and keys on libsodium, ADR-0018), `server/` (`kinige-server`: the sync server on axum and Postgres, a lib and a binary, Linux first; spike S4c-1, ADR-0019, and S4c-2, ADR-0020: blobs on a disk or S3-compatible store, `backup` and `restore` commands, account deletion; its `README.md` has the configuration and the test map), `deploy/` (the self-hosting stack: a pinned `Dockerfile`, `docker-compose.yml`, `Caddyfile`, `.env.example` with placeholders only, `backup.cron.example`, and `smoke.sh`, which CI's `compose` job runs; images pinned by digest; nothing is bind-mounted from the machine that runs compose, so a remote Docker daemon works), `src-tauri/` (thin Tauri 2 layer), `web/` (React + TS + Vite + Storybook), `design/tokens/` (DTCG), `scripts/`, `.github/workflows/ci.yml`. Rust and Node versions are pinned in `rust-toolchain.toml` and `.node-version`. Details and the startup measurement method are in `README.md`.

Rust (from the root):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — also regenerates the TS types in `web/src/api/generated/` (ts-rs, ADR-0002); after changing types in Rust, commit the result; it also rewrites the year tables of the forest stories in `web/src/ui/forest/fixtures/` (built by the real `year_table`, slice 6; CI fails when they differ from the commit)
- `cargo build -p kinige-domain --target wasm32-unknown-unknown`
- Test hooks (KINIG-103): injected panics and faults and probes of the internals (`crates/kinige-store/src/test_hooks.rs`, the `#[doc(hidden)]` items of `kinige-store` and `kinige`) live behind the cargo feature `test-hooks`, off by default; only the crates' own dev-dependencies turn it on, so `cargo test` and `cargo clippy --all-targets` see the hooks and a build for people does not. A new hook goes behind the feature. `cargo clippy --workspace --lib --bins -- -D warnings` compiles the code without them; `bash scripts/check-release-features.sh` fails if the release dependency graph enables the feature (`bash scripts/check-release-features.test.sh` is its control; both run in CI).
- Server against a real Postgres and S3, the S4a simulation against the server, and the compose smoke test: the `server-tests` skill (`.claude/skills/server-tests/SKILL.md`). It covers `--features pg-tests` with `KINIGE_TEST_DATABASE_URL`, throwaway `kinige-…` containers on ubuntu-server, and CI's `server` and `compose` jobs.

Frontend (from `web/`, after `npm ci`):
- `npm run tokens` — rebuild `src/ui/tokens.css` and `src/ui/themes.css` from `design/tokens/**/*.json` (runs by itself before `dev`, `build`, `storybook`); the result is committed
- `npm run typecheck`, `npm run lint`, `npm test` (vitest; includes the check "no raw colors and sizes outside tokens.css / themes.css", the contrast of all "color / background" pairs of the palette, and the font check against the woff2 files themselves)
- `npm run build-storybook`

`fixtures/api-contract/`: rules `MockApi` repeats from Rust, read by vitest and `cargo test -p kinige-store --test api_contract`; Rust is the source of truth, so a rule change updates the fixture and the mock.

**Screens and visual baselines: the `screens` skill** (`.claude/skills/screens/SKILL.md`). It covers:
- real-window screenshots through `screens.yml` on macOS and Windows;
- the screens driver (KINIG-92, ADR-0016) with `DRIVER=1`;
- the fake microphone of voice-note scenarios (ADR-0015), and the guard `scripts/check-test-hooks.sh` that keeps it and the driver out of a build for people;
- Storybook baselines through `visual-update.yml`. They are never captured on macOS or ubuntu-server.

App: `scripts/run-app.sh` (dev), `scripts/storybook.sh`, `scripts/build-app.sh` (release `.app` + ad-hoc signature + `codesign --verify` + check of the bundle version against `Cargo.toml`; `KINIGE_BUNDLES=app,dmg` adds a `.dmg`), `scripts/build-windows.sh` (release unsigned NSIS installer under Git Bash on Windows, per-user; checks the installer name and `ProductVersion` of `kinige.exe` against `Cargo.toml`; the MCP bridge is not built there: `src-tauri/tauri.windows.conf.json` empties `bundle.externalBin`), `scripts/screenshot-window.sh <out.png>` (the Kinige window, `screencapture -x -o -l`; for manual debugging and for `screens.yml` itself; agents don't run it on the user's Mac; nothing but the window may get into the screenshot), `scripts/measure-cold-start.sh`, `scripts/mcp-session.py <kinige-mcp> <socket> <protocol file>` (an MCP session through the bridge with our own client, for the acceptance of slice 4a; Claude configs are not touched). Build and run put the bridge into `src-tauri/binaries/kinige-mcp-<triple>` (sidecar, `bundle.externalBin`); on `cargo clippy`/`cargo test` on a clean clone, an empty stub from `build.rs` sits there.

**Versions and releases (ADR-0012): the `release` skill** (`.claude/skills/release/SKILL.md`).
- Semver: the minor version is the slice number, and 1.0.0 comes after sync.
- The version lives only in `[workspace.package]` of `Cargo.toml`.
- Only the coordinator cuts a release, after the user accepts a slice or a batch of fixes. It uses `scripts/release.sh`.
- A tag is never moved or deleted.

CI (GitHub Actions, `.github/workflows/ci.yml`; status and rerun via `gh run list`, `gh run rerun`) repeats everything listed above except building the `.app`, and adds: the `macos` job (`cargo clippy -p kinige --all-targets -- -D warnings` and `cargo test -p kinige` on `macos-15`: code under `cfg(target_os = "macos")`), the `windows` job (`cargo clippy` and `cargo test` for `kinige`, `kinige-store`, `kinige-domain`, `kinige-mcp` on `windows-2025`; MCP and the bridge are `cfg(unix)`, tests that need Unix sockets, modes, symlinks or FIFOs are `#[cfg(unix)]`), `gitleaks` (false positives go to `.gitleaksignore`) and `deny` (`cargo-deny`: licenses, RustSec, sources — `deny.toml`). Dependencies are updated by Dependabot (weekly, minor updates as a group); third-party actions are pinned by SHA with a version comment. A "green CI" can't be checked locally; the closest equivalent is the commands above.

## Environment

- Docker on this Mac is remote (ubuntu-server, `DOCKER_HOST` is already set): bind mounts of Mac paths don't work, ports are reachable at `192.168.1.61:<port>`. Name your containers with the `kinige-` prefix and clean up after yourself.
- Codex CLI: `codex exec --skip-git-repo-check --ephemeral -s read-only -C <dir> -o <file> -` (prompt via stdin).
