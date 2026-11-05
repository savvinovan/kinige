# KINIG-92: screens driver inside the test build

Budget: ~5 h to a green CI with both 7a scenarios on the driver; stop and report at 10 h. Huly: KINIG-92; branch `tools/screens-driver`, PR and first commit `KINIG-92: …`. Release: none by itself (tooling; ships with the next patch or slice). Source: the slice 7a retro of 05.11.2026. Of ~30 h wall-clock, ~8–9 h went to the real-window screens scenarios: 42 runs, 27 of them failed or were cancelled. On macOS, accessibility actions hang while a recording runs and a window tree dump takes up to 3 min. On Windows, Play was reached by counting SendKeys Tab stops, which took ~4 h for one shot.

## Scenario
1. An agent writes a screens step such as `press "Play"`, the same on macOS and Windows. The scenario script (bash or PowerShell) hands it to the app. Code inside the app's own webview finds the element by its accessible name and clicks it. The script gets `ok` or an error naming what was not found within a second or so. There are no AX calls, no Tab counting and no window tree dumps.
2. The script still takes the screenshot itself (`screenshot-window.sh` / `CopyFromScreen`) and still checks the data with sqlite and the disk, as today.
3. Before every shot, the personal-path check reads the window text through the driver (`text` step): about 100 ms, where an AX dump takes up to 180 s.
4. A release build (`release.yml`, `build-app.sh`, `build-windows.sh` without the screens variables) contains no driver, neither in `web/dist` nor in the Rust binary. A build that does contain it is refused unless the screens workflow allowed it.

## Included
- **Build gate, two halves, both off by default.**
  - Web: `KINIGE_SCREENS_DRIVER=1` points `virtual:kinige-driver` (in `web/vite.config.ts`, read from `process.env` like the fake microphone) at `scripts/screens/driver.ts`. Otherwise it points at a no-op module.
  - Rust: a cargo feature `screens-driver` of `src-tauri`, enabled by `build-app.sh` and `build-windows.sh` only when `KINIGE_SCREENS_DRIVER=1`.
  - Guard: generalise `check-fake-mic.sh` into one test-hooks guard covering both markers: the fake microphone's and a new driver marker string in `driver.ts`. Each marker gets its own allow variable (`KINIGE_ALLOW_SCREENS_DRIVER=1`, set only in `screens.yml`). Keep `--expect` as the positive control, and add a shell test like `check-fake-mic.test.sh`. The cargo feature must also refuse to build without `KINIGE_ALLOW_SCREENS_DRIVER=1` (a `build.rs` check or the build scripts, implementer's choice, with a test or a CI step that proves it).
- **Channel: files, no ports.** With the feature on, the app reads `KINIGE_SCREENS_DRIVER_DIR` at start; without the variable the driver stays idle. The flow:
  1. The script writes `requests/<id>.json` atomically (temporary file, then rename): `{ "id", "window": "main" | "settings" | "capture", "step": {…} }`.
  2. A Rust thread polls about every 100 ms and sends the step to that window's webview as an app event.
  3. The webview runs the step and answers through a feature-gated command `screens_driver_reply`. The command exists only with the feature, and only the app windows' capabilities allow it, also only with the feature.
  4. Rust writes `replies/<id>.json` atomically: `{ "id", "ok": bool, "value"?, "error"? }`.
  - Garbage requests (bad JSON, unknown window, unknown step, oversized file, path tricks in `id`) get an error reply, or are skipped with a log line by event kind only. They never panic (ADR-0011) and never write outside the directory. The rules on new parsers and IPC commands (12) apply.
- **Steps (v1), in `driver.ts`.** Elements are matched by accessible name: `aria-label`, `aria-labelledby`, the label of a form control, `title`, or trimmed text content. The optional `role` and `nth` work like `ax.js` today.
  - `press {name, role?, nth?}`: scroll into view, then the pointer and mouse down/up sequence and `click`.
  - `menu {name}`: `contextmenu` at the element's centre.
  - `type {name?, text}`: into the named or the focused input or textarea, using the native value setter plus `input`, so React sees it; contenteditable if the composer needs it.
  - `key {key, code?, mods?}`: keydown/keyup on the active element.
  - `wait {name, role?, gone?, timeoutMs}`.
  - `has {name}`.
  - `scroll {name}`.
  - `text`: `innerText` of the document plus every `aria-label` and `title`, for the personal-path check.
  - `front`: Rust shows and focuses the window.
  - **No arbitrary JS eval** and no IPC passthrough: the driver can do only what a user's hands can do in the window.
- **Shell side.**
  - `lib.sh` gets `drive <window> <json>` (time limit, reply check, `fail` on error).
  - `press`, `show_menu`, `scroll_to`, `has` and `shoot` switch to the driver when the scenario sets `DRIVER=1`. `shoot` then runs its personal-path grep on the `text` reply instead of the AX dump.
  - `lib-windows.ps1` gets `Drive`, `Press`, `Wait-Name`, `Has` and a driver-based text check in `Shoot`.
  - Old scenarios that do not set `DRIVER=1` keep working as they are.
- **`screens.yml`:** builds with the driver for the scenarios that need it (a `driver` input or the scenario name, implementer's choice), sets the allow variable only there, and passes `KINIGE_SCREENS_DRIVER_DIR` (LSEnvironment on macOS, the environment on Windows) under the runner temp.
- **Port as proof:** move `7a.sh` and `7a-windows.ps1` onto the driver. Same shots, same file names, same data checks. On Windows the Play shot comes from a `press` instead of Tab stops.
- **Docs:** ADR-0016 (half a page: why a driver in the build, the channel, the gate). Update the screens paragraph in `CLAUDE.md` with the new input rule ("driver steps where `DRIVER=1`; AX / SendKeys only for native UI"). Also put "7a accepted 05.11.2026" in the `CLAUDE.md` status line, in the first commit.

## Probe first (first hour, before porting)
A minimal `screens-driver-probe` scenario, macOS and Windows, records `docs/screens/screens-driver-probe/facts.txt`:
1. Does `press` on a voice note's Play start playback without user activation? Synthetic events are `isTrusted: false`. Check `audio.paused` and that `currentTime` moves.
2. Does it start a recording from the fake microphone?
3. Does the driver answer while a recording runs, where AX hangs on macOS?
4. The round-trip time of one step.

If Play does not start on a platform, that one step stays on the old input there (AX press on macOS, the Tab path on Windows). Write the fallback down in the ADR and keep going. Don't spend the budget fighting it.

## Not included
- Migrating the other scenarios (4a…7, viewer, chat-color). They move when they are next touched.
- Native UI: file pickers, the tray menu, native context menus, global hotkeys (still never simulated), the system permission prompt.
- Screenshots from inside the app.
- A recorder for steps, a test DSL, or running scenarios outside the runners.
- Any presence in release builds.

## What "done" looks like
- `docs/screens/7a/` and `docs/screens/7a-windows/` are regenerated by the driver-based scenarios with the same file list as on `main`. The reviewer compares each shot with the `main` version: same states, differences only from timing (the waveform progress).
- The macOS and Windows run durations, before (from the 7a runs on main) and after, are in the PR description.
- The guard is shown in action: the build fails on a driver build without the allow variable (log line in the PR), and `--expect` passes inside screens.
- The probe facts are committed.

## Checks
- **cargo test:** request parsing on garbage input, the id check against path traversal, reply writing, the feature's absence in the default build (the command is not registered, so a negative test through the guard / command names), `clippy -D warnings` with and without the feature.
- **vitest:** name matching in `driver.ts` (aria-label, label, title, text, role, nth, hidden elements skipped), `type` reaching a React-controlled input, unknown step, an element that is missing.
- **Shell:** the guard's shell test; `lib.test.sh` still passes.
- **CI:** everything in `ci.yml` plus the `macos` job built with `--features screens-driver` (clippy only) so the gated code does not rot.

## Touches sensitive areas
Tauri commands and capabilities (§12.3): one new command, feature-gated, with negative tests. No schema, sync or keys. The risk is a test backdoor reaching users, so the reviewer and `sync-crypto-guardian` both check the gate: no driver in a default build, and refused without the allow variable. No new packages.
