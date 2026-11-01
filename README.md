# Kinige v2

[![CI](https://github.com/savvinovan/kinige/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/savvinovan/kinige/actions/workflows/ci.yml)
[![Coverage Rust](https://codecov.io/gh/savvinovan/kinige/branch/main/graph/badge.svg?flag=rust)](https://app.codecov.io/gh/savvinovan/kinige?flags%5B0%5D=rust)
[![Coverage Web](https://codecov.io/gh/savvinovan/kinige/branch/main/graph/badge.svg?flag=web)](https://app.codecov.io/gh/savvinovan/kinige?flags%5B0%5D=web)
[![Release](https://img.shields.io/github/v/release/savvinovan/kinige?include_prereleases)](https://github.com/savvinovan/kinige/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.94](https://img.shields.io/badge/rust-1.94-orange.svg)](rust-toolchain.toml)

A personal journal shaped like a "chat with yourself". Desktop app on Tauri 2 + Rust + React/TypeScript. The normative document is [`docs/plan/02-prd.md`](docs/plan/02-prd.md).

Status: slices 0–5 and 4a/4b are accepted — writing to chats (SQLite and a Markdown copy in `~/Documents/Kinige`), quick capture with a global hotkey, entry actions and trash, import from Telegram and search, an MCP server for AI agents, a settings window, the "On this day" card and replies; the interface is in English and Russian. The list and status of slices are in `CLAUDE.md` and `docs/slices/`, acceptance screenshots are in `docs/screens/<slice>/`.

## Download

Ready builds are on the [Releases](https://github.com/savvinovan/kinige/releases) page: for macOS (Apple Silicon) a `.dmg` and a `.zip`, for Windows 10/11 (x64) `Kinige-X.Y.Z-windows-x64-setup.exe`, and one `SHA256SUMS` for all of them. The build is signed ad-hoc (Developer ID and notarization come in slice 10), so macOS stops it on first launch. On macOS 15 and later: launch `Kinige.app` once and click "Open Anyway" in "System Settings" -> "Privacy & Security"; on macOS 13-14 a right click -> "Open" is enough. On any version, `xattr -dr com.apple.quarantine /Applications/Kinige.app` works too. The Windows installer is unsigned until slice 10, so SmartScreen shows "Windows protected your PC": click "More info" -> "Run anyway". It installs per-user, without administrator rights; the data are in `%LOCALAPPDATA%\me.savvinov.kinige`, the Markdown copy in `Documents\Kinige`. AI-agent access (MCP) is not available on Windows yet. Versions are semver, minor = slice number ([ADR-0012](docs/adr/0012-versions-and-releases.md)); the current version is shown in the app: "Profile" -> "About".

## Requirements

- macOS with Xcode Command Line Tools (`xcode-select --install`).
- Rust: the version is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it by itself.
- Node: the version in [`.node-version`](.node-version) (the same one in CI and in the visual test container; locally it was also checked on Node 26.8.1).
- Python 3 (for the cold start measurement). Storybook visual tests don't run on this machine: they run on GitHub (the `visual` job), Docker is not needed.

## Commands

| What                                                                         | Command                                                                                                                                         |
|------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| Run the app (dev)                                                            | `scripts/run-app.sh`                                                                                                                            |
| Open Storybook (http://localhost:6006)                                       | `scripts/storybook.sh`                                                                                                                          |
| Build `Kinige.app` (release, ad-hoc signature, `codesign --verify`)          | `scripts/build-app.sh`                                                                                                                          |
| Release a version (owner: a PR with the new version, then a tag)             | `scripts/release.sh X.Y.Z`, after the merge `scripts/release.sh --tag X.Y.Z` (the build and the Release are done by `release.yml`)              |
| Screenshots of the real window for slice acceptance                          | `gh workflow run screens.yml --ref <branch> -f slice=<id>` (GitHub macOS runner, synthetic data; CI on the branch starts by itself)             |
| Take a screenshot of the Kinige window by hand                               | `scripts/screenshot-window.sh docs/screens/0/window.png` (capture window: 3rd argument `capture`, tray icon menu: `menu`, window list: `list`)  |
| Measure cold start                                                           | `scripts/measure-cold-start.sh [runs]`                                                                                                          |
| Storybook visual tests                                                       | the `visual` job in CI on GitHub                                                                                                                |
| Update snapshot baselines (in a branch, not in `main`)                       | `gh workflow run visual-update.yml --ref <branch>` (CI on the branch starts by itself)                                                          |
| Rebuild `tokens.css` and `themes.css` from `design/tokens/**/*.json`         | `npm --prefix web run tokens`                                                                                                                   |

The other checks (the same as in CI) are in the "Build and tests" section of [`CLAUDE.md`](CLAUDE.md).

## Structure

- `crates/kinige-domain` — domain rules, no IO, builds for `wasm32`: HLC, canonical JSON of groups (JCS), groups of a new entry, `journal_date`, link validation, Markdown copy format, API types.
- `crates/kinige-store` — SQLite (`rusqlite`): migrations, a single writer thread, WAL, background writing of the Markdown copy.
- `src-tauri/` — thin Tauri layer: two windows (the main one and the quick capture window — on macOS an NSPanel that doesn't activate the app, [ADR-0006](docs/adr/0006-quick-capture.md)), commands, global shortcuts, menu bar icon, clipboard (`arboard`), opener and dialog plugins (from Rust only), `on_navigation`, each window with its own capability (`capabilities/main.json`, `capabilities/capture.json`), CSP.
- `web/` — React + TS + Vite; `src/api` (`KinigeApi`, `TauriApi`, `MockApi`, generated types), `src/ui/tokens.css` (generated).
- `design/tokens/**/*.json` — tokens in DTCG format → Style Dictionary → `web/src/ui/tokens.css` (roles, `:root`) and `web/src/ui/themes.css` (`@font-face`, Reduce Motion). Colors, sizes and fonts outside these files are forbidden in `web/src` (checked by the `no-raw-values` test).
- Design system D1: the dark graphite palette — neutral graphite with an accent and error color from Nord (`design/tokens/palettes/graphite.json`; color origins are in the `README.md` next to it) and the Inter + JetBrains Mono font set (`design/tokens/fontsets/inter.json`; files in `web/src/ui/fonts/`, sources and licenses in its `README.md`); both were chosen by the user (ADR-0004), and `design/tokens/active.json` points to them. Chat emoji are drawn by the system emoji font (the only exception to "fonts are bundled"). Icons are Lucide via `lucide-react` (`web/src/ui/icons/README.md`).
- Density and text size: all font sizes, spacing, radii, line heights and icons are in rem, where 1rem = the entry text size (`font.root.default`, 13 px — chosen by the user among 13 / 14 / 15 px); changing the root size changes the whole density at once. `data-text-scale="large"` on `<html>` multiplies the root size (`font.scale.large`, ×1.25). Reduce Motion: `prefers-reduced-motion` or the `data-reduce-motion` attribute.
- The screen in the app is the real main window on `TauriApi` (slice 1); `MockApi` implements the same `KinigeApi` in memory for tests and Storybook. Interface strings are in `web/src/locales/{en,ru}.json` (i18next, [ADR-0005](docs/adr/0005-i18n.md)); Rust picks the language from the system languages.
- Data: the database is `~/Library/Application Support/me.savvinov.kinige/kinige.db`, the Markdown copy is `~/Documents/Kinige/<chat>/YYYY/YYYY-MM-DD.md` (the folder is changed in the settings). To check on clean data: the environment variables `KINIGE_DATA_DIR` (database directory) and `KINIGE_MIRROR_DIR` (default copy folder). `KINIGE_TRACE=1` writes the life of the capture window to stderr (`show`, `key`, `resign`, `hide`, `flash`; no entry texts) — for checking in the live window.
- App log: macOS — `~/Library/Logs/me.savvinov.kinige/kinige.log` (Windows — `%LOCALAPPDATA%\me.savvinov.kinige\logs`, Linux — `~/.local/share/me.savvinov.kinige/logs`; per the `tauri-plugin-log` documentation, not checked on these OSes). Rotation by size: a file up to 1 MB, up to 5 files are kept (the current one and archived ones with a date in the name). The default level is `info`; `KINIGE_LOG=debug` (or `trace`, `warn`, `error`, `off`) changes the level for Kinige code, third-party crates write only `warn` and above. In a dev build and when `KINIGE_LOG`, `KINIGE_TIMING`, `KINIGE_TRACE` are set, the log is duplicated to stderr. Entry texts, chat names, user paths and import contents never get into the log — only event kinds and error categories. The `KINIGE_TIMING` and `KINIGE_TRACE` marks go only to stderr and are not written to the file.
- Rust → TS types: `ts-rs`, see [ADR-0002](docs/adr/0002-ts-types-via-ts-rs.md). Signing keys: [ADR-0003](docs/adr/0003-signing-keys.md).

## CI (GitHub Actions)

Workflow: [`.github/workflows/ci.yml`](.github/workflows/ci.yml): `rust` (fmt, clippy, test, wasm32, TS types up to date), `web` (`tokens.css` and `themes.css` up to date, tsc, ESLint, vitest with coverage → Codecov, flag `web`), `macos` (clippy and tests of the app crate on `macos-15`), `visual` (Storybook snapshots in a Playwright container), `gitleaks` (secret scanning, `.gitleaksignore`), `deny` (`cargo-deny`: licenses, RustSec vulnerabilities, sources, `deny.toml`), `coverage` (`cargo llvm-cov` → [Codecov](https://app.codecov.io/gh/savvinovan/kinige), flag `rust`; informational for now, doesn't block PRs).
The `.app` is built and signed ad-hoc locally (`scripts/build-app.sh`); the release build for a tag is `release.yml` (`.dmg`, `.zip`, `SHA256SUMS`, GitHub Release, ADR-0012); Developer ID and notarization come in slice 10. A macOS runner is already in use: the `macos` job (clippy and tests of the app crate), window screenshots via `screens.yml`.

## License

[MIT](LICENSE). Third-party fonts, icons, data and palettes, their authors and licenses are listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).

## Cold start measurement method

The target is < 1.5 s (PRD §15). Measurement: `scripts/measure-cold-start.sh 10` after `scripts/build-app.sh`.
- The machine and OS are printed by the script; results are recorded together with them.
- Release build only (`target/release/kinige`), 10 runs, each a new process, with a 1 s pause between runs.
- What is measured is the time from process start to the page load event in the WebView (Rust writes a mark to stderr via `log` when `KINIGE_TIMING=1`). This is an upper bound on the time to the first frame with content. OS caches are not flushed (flushing needs `sudo purge`); so the first run is reported separately, and the deciding number is the median.
- Result of slice 0: [`docs/screens/0/cold-start.txt`](docs/screens/0/cold-start.txt).

## Rules

See [`CLAUDE.md`](CLAUDE.md): one slice = one branch = one PR; visuals only from tokens; no edits to source files with replacement scripts.
