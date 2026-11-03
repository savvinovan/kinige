# Kinige

[![CI](https://github.com/savvinovan/kinige/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/savvinovan/kinige/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/savvinovan/kinige?include_prereleases)](https://github.com/savvinovan/kinige/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Kinige is a journal that works like a messenger. You write notes to yourself the way you would text someone, in chats like Inbox, Books or Ideas. Later you reread them as a journal, look at the year as a small forest, and get back what you wrote on this day years ago.

Your notes stay on your computer. They live in a SQLite database, and Kinige also keeps a plain Markdown copy of every chat in `~/Documents/Kinige`, so you can read them without the app. Sync between machines through your own server, with end-to-end encryption, is planned but not built yet.

This is a personal project and it is early. The interface is in English and Russian.

## What works

- Chats with emoji, pinned chats and an archive. You can edit notes, move them between chats, change their date, and delete them to a trash you can restore from.
- Quick capture: a global hotkey opens a small window over any app. You can start a new note, add to the last one, or paste the clipboard.
- Import of Telegram "Saved Messages", and search that understands Russian word forms.
- "On this day" shows what you wrote on the same date in earlier years, and you can reply to an old note "N years later".
- The forest of the year: each day is a tree that grows with how much you wrote. Click a day and the feed jumps to it. There is a calendar view too.
- An MCP server, so AI agents can search and read your journal and add to Inbox. You choose which chats they see (macOS only for now).
- A settings window, plus a `settings.json` you can edit by hand.

Not there yet: photos and files, voice notes, sync, Linux.

## Install

Builds for macOS (Apple Silicon) and Windows 10/11 (x64) are on the [Releases](https://github.com/savvinovan/kinige/releases) page, with one `SHA256SUMS` for all files.

The builds are not signed with a developer certificate yet, so both systems will warn you the first time.

On macOS 15 and later, open `Kinige.app` once, then go to System Settings > Privacy & Security and click "Open Anyway". On macOS 13 and 14, right-click the app and choose Open. `xattr -dr com.apple.quarantine /Applications/Kinige.app` works on any version.

On Windows, SmartScreen says "Windows protected your PC". Click "More info", then "Run anyway". The installer needs no admin rights. AI agent access (MCP) is not available on Windows yet.

The version is shown under Profile > About. The minor version goes up with each development slice, and 1.0 will come with sync ([ADR-0012](docs/adr/0012-versions-and-releases.md)).

## Build from source

You need the Xcode Command Line Tools (`xcode-select --install`), `rustup` and Node. `rustup` picks up the Rust version from [`rust-toolchain.toml`](rust-toolchain.toml); the Node version is in [`.node-version`](.node-version). Python 3 is only needed for the cold start measurement.

```sh
npm --prefix web ci
scripts/run-app.sh          # dev build with hot reload
scripts/build-app.sh        # release Kinige.app, ad-hoc signed
cargo test --workspace      # Rust tests
npm --prefix web test       # frontend tests
```

On Windows, `scripts/build-windows.sh` builds the installer under Git Bash. The full list of checks, the CI jobs and the release steps are in the "Build and tests" section of [`CLAUDE.md`](CLAUDE.md).

### Where things are

| Path                     | What it is                                                                                       |
|--------------------------|--------------------------------------------------------------------------------------------------|
| `crates/kinige-domain`   | Rules without IO: journal days, the Markdown copy format, the forest grid. Builds for `wasm32`.  |
| `crates/kinige-store`    | SQLite: migrations, one writer thread, the Markdown copy, import, search.                        |
| `crates/kinige-mcp`      | The MCP bridge for AI agents.                                                                    |
| `src-tauri/`             | A thin Tauri 2 layer: windows, commands, hotkeys, tray. Each window has its own permissions.     |
| `web/`                   | React + TypeScript + Vite, Storybook. `KinigeApi` has a Tauri and an in-memory implementation.   |
| `design/tokens/`         | Design tokens. Colors, sizes and fonts come only from here; a test checks it.                    |
| `docs/`                  | The PRD, ADRs, slice specs, and screenshots of every accepted slice.                             |

[`docs/plan/02-prd.md`](docs/plan/02-prd.md) is the source of truth for what the app should do. Decisions are in [`docs/adr/`](docs/adr/), and each slice starts from a one-page spec in [`docs/slices/`](docs/slices/).

### Data and logs

On macOS the database is in `~/Library/Application Support/me.savvinov.kinige/`, and the log is `~/Library/Logs/me.savvinov.kinige/kinige.log` (5 files of up to 1 MB). On Windows both are under `%LOCALAPPDATA%\me.savvinov.kinige`. The log never contains note text, chat names or your file paths, only event kinds and error categories.

Environment variables for development:

| Variable                                | Effect                                                                         |
|-----------------------------------------|--------------------------------------------------------------------------------|
| `KINIGE_DATA_DIR`, `KINIGE_MIRROR_DIR`  | Use another database folder and Markdown copy folder (to test on clean data).  |
| `KINIGE_LOG=debug`                      | Log level for Kinige code (`trace`, `debug`, `info`, `warn`, `error`, `off`).  |
| `KINIGE_TIMING=1`                       | Print the page-loaded mark to stderr, for the cold start script.               |
| `KINIGE_TRACE=1`                        | Print the quick capture window's events to stderr.                             |

### Cold start

The target is under 1.5 s (PRD §15). `scripts/measure-cold-start.sh 10` after `scripts/build-app.sh` starts the release build 10 times on an empty database. It measures from process start to the web view's page load event, which is a little later than the first frame. OS caches are not flushed, so the script reports the first run separately, and the median is the number that counts. Results for each slice are in `docs/screens/<slice>/cold-start.txt`.

## License

[MIT](LICENSE). Third-party fonts, icons, data and palettes, with their authors and licenses, are listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).
