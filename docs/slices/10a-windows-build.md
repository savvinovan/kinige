# Slice 10a: Windows build with MCP disabled

**Budget:** ~6 h of coding, ~10 h until ready for acceptance (most of it is CI iterations on a Windows runner that cannot be run locally). If it is not ready after 20 h, stop and report (§12.1a). **Depends on:** 5 (accepted), KINIG-54 (releases, ADR-0012). Huly: **KINIG-61** (also covers **KINIG-50**, `cfg(unix)` around MCP and the bridge); branch `slice/10a-windows-build`, PR and first commit are `KINIG-61: …`. **Sources:** PRD §1 (platforms), §5.1 (tray, capture window), §9.3 (MCP), §12.1–12.3, §15 (row 10); ADR-0003 (signing), ADR-0009 (MCP), ADR-0010 (settings file), ADR-0012 (versions and releases).

## Scenario
1. On the GitHub release page next to `Kinige-X.Y.Z-macos-arm64.dmg` there is `Kinige-X.Y.Z-windows-x64-setup.exe` and a single `SHA256SUMS` that covers every file. I download the installer, check the hash, and run it. SmartScreen says "Windows protected your PC" (the build is unsigned until slice 10); "More info" -> "Run anyway" starts the install. It is per-user: no administrator prompt.
2. Kinige opens a normal window: Inbox with the "On this day" card, the sidebar with chats, the composer. The journal works as on macOS: I write, the entry appears, I restart the process and it is still there. The database lives in `%APPDATA%\me.savvinov.kinige`, the Markdown copy in `Documents\Kinige`.
3. The tray icon has the same menu (New note, Inbox, recent chats, Settings, Quit). The global shortcuts are registered by the plugin; the capture window is an ordinary always-on-top window without a title bar.
4. "Settings" opens the settings window. The "AI agent access" page has no switch: a quiet line "Not available on Windows yet." There is no socket, no listener and no bridge in the Windows build; MCP commands answer `unsupported`.
5. On macOS nothing changes: same bundle, same bridge, same tests.

## In scope
- **The workspace compiles and passes tests on Windows (`x86_64-pc-windows-msvc`).**
  - `src-tauri/src/mcp` exists only under `cfg(unix)` (Unix socket, `peer_cred`, `own_uid`). On other platforms a small stub module with the same public surface: `start` and `shutdown` do nothing, `socket_info` answers `error = "unsupported"`.
  - `crates/kinige-mcp` (the bridge): the Unix code is under `cfg(unix)`; on Windows the binary compiles and exits with code 2 and one line on stderr ("unsupported on this platform"), without touching the network or the disk. It is not part of the Windows bundle (`tauri.windows.conf.json` empties `externalBin`; `build.rs` makes no stub there).
  - `kinige-store`: `settings_file/io.rs` already has `cfg(not(unix))` branches (no owner and mode checks, no `O_NOFOLLOW`, no `libc`). The weaker check is documented in ADR-0010 as a Windows note. The one `unsafe` (`geteuid`) stays unix-only.
  - Durability code that opens a directory or a read-only handle to `sync_all` is fixed for Windows (the call fails there): a regression test where the behaviour is observable.
  - Non-macOS paths of the capture window, tray, activation and global shortcuts compile and behave: the capture window is a normal window, `Focused(false)` hides it.
- **Error code `unsupported`** (Rust `ErrorCode`, generated TS type, `TauriApi`, strings `error.unsupported` ru/en) for the agent-access commands that change something (`set_agent_enabled`, `set_agent_mode`, `set_agent_chats`) on platforms without MCP.
- **UI:** the "AI agent access" page, when `socket.error == "unsupported"`, shows only the title and the line "Not available on Windows yet." (ru and en, `agent.unavailable`); no switch, mode, chats, log or connect blocks. A Storybook story and a component test.
- **CI (`ci.yml`):** job `windows` on `windows-2025`: `cargo clippy -p kinige -p kinige-store -p kinige-domain -p kinige-mcp --all-targets -- -D warnings`, then `cargo test` for the same crates. Unix-only tests are gated with `#[cfg(unix)]`. `rust-cache` saves on `main` only, like the other jobs.
- **Release (`release.yml`):** a `windows` build job on `windows-2025`: unsigned NSIS installer (`tauri build --bundles nsis`, per-user), file `Kinige-X.Y.Z-windows-x64-setup.exe`; the tag is checked against the version in `Cargo.toml` (as for macOS) and against the version stored in the built `kinige.exe` and in the installer. One `SHA256SUMS` covers the macOS and Windows files (a short `checksums` step in `publish`, not two files). Release notes get a Windows section: SmartScreen ("More info" -> "Run anyway"), data locations, AI-agent access is not available yet.
- **Screens on a Windows runner:** `screens.yml` gets the input `os` (`macos` default, `windows`). `scripts/screens/10a.ps1` builds nothing itself: the workflow builds the installer, installs it silently into a temporary directory, runs the installed app on synthetic data (`seed_demo`, temporary `KINIGE_DATA_DIR` and `KINIGE_MIRROR_DIR`), waits for the window, drives it with UI Automation (`Invoke` patterns, no mouse and no keyboard) and captures only the Kinige window by its rectangle. The personal-path check runs on the UI Automation text of each window before the capture, as on macOS.
- **Docs:** ADR-0012 (Windows artifacts), ADR-0010 (Windows note), `README.md` "Download", `CLAUDE.md` "Build and tests", PRD §15 row 10a and a history line.

## Out of scope (stay in slice 10)
- Linux build and acceptance.
- MCP over named pipes; any agent access on Windows.
- Code signing (Authenticode), the updater, winget or MSI. SmartScreen warns until then.
- Windows-specific UI polish: shortcut glyphs (⌘/⌥ vs Ctrl/Alt) in tooltips and the shortcut recorder, native title bar styling, tray icon colouring for light and dark taskbars.
- The global-shortcut and capture-window flow in a real session: the runner cannot press keys, so these compile and are covered by the existing mock-runtime tests only. This is the main residual risk and is called out in the PR.
- Installer languages, custom installer artwork, per-machine install.

## Done looks like
- `windows` job in CI is green; macOS and Linux jobs unchanged and green.
- `release.yml` run in dry-run mode (build and upload the artifact, no Release) on a branch: the installer exists, size is reported, `SHA256SUMS` lists macOS and Windows files, the versions match.
- Screenshots of the real window from the Windows runner in `docs/screens/10a/` (normal and large text), plus the text checks.
- Data survives killing the process: store tests on Windows (the existing `abort` tests are Unix-only because they use signals; a Windows variant kills the child with `TerminateProcess`).

## Screenshots (`docs/screens/10a/`)
1. `01-inbox.png`: main window, Inbox with the "On this day" card.
2. `02-chat.png`: a chat with entries (Markdown, tasks).
3. `03-settings-appearance.png`: settings window, Appearance.
4. `04-settings-access-unavailable.png`: "AI agent access" with "Not available on Windows yet."
5. `05-large-text-inbox.png`, `06-large-text-settings.png`: large text.
6. `07-after-kill.png`: after killing the process and starting again, entries are in place.
Texts: `install.txt` (installer size, SHA256, install directory, version of the installed exe), `data-location.txt` (names of files in `%APPDATA%` and the mirror folder, no content).

## Touches sensitive zones
Only the settings-file safety checks on Windows (ADR-0010): ownership and mode checks do not exist there, the data directory is protected by the per-user ACL of `%APPDATA%`. `sync-crypto-guardian` reviews the Windows note in ADR-0010 before merge. Schema, protocol, envelope and keys do not change.

## Decisions of the user (1.11.2026)
1. Build for Windows now, not the whole slice 10: it compiles, AI-agent access is disabled until named pipes.
2. An unsigned installer goes into the GitHub Release next to the macOS `.dmg`.
3. Acceptance by screenshots of the real window from a GitHub Windows runner (no Windows machine).

## Open questions
None.
