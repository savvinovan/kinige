# ADR-0016: The screens driver inside the test build

Date: 2026-11-05. Status: **accepted** (KINIG-92; spec `docs/slices/screens-driver.md`). Touches Tauri commands and capabilities (PRD §12.3); no schema, sync or keys.

## Context
Slice 7a spent about 8 of its 30 hours on real-window scenarios: 42 screens runs, 27 failed or cancelled. On macOS an accessibility call hangs while a recording runs and a window tree dump takes up to 3 minutes; on Windows the Play button was reached by counting `SendKeys` Tab stops, which took about 4 hours for one shot. Both are the cost of driving a web view from outside.

## Decision
**A driver in the test build.** The built app carries a small driver that finds an element by its accessible name inside its own web view and does what a person's hands would: `press`, `menu` (a context menu), `type`, `key`, `wait`, `has`, `scroll`, `text` (the window text, for the personal-path check) and `front` (done by Rust: show and focus the window). There is no script evaluation and no IPC passthrough, so it can do nothing a user cannot do in the window. Elements match by `aria-label`, `aria-labelledby`, a form label, `title`, value text or content; an exact name beats a substring, an operable element beats plain text, hidden elements are skipped, and `role` and `nth` narrow the match like `ax.js` did.

**The channel is files, no ports.** With the cargo feature on and `KINIGE_SCREENS_DRIVER_DIR` set, a thread polls `requests/<id>.json` every 100 ms, validates it (ids are `A-Za-z0-9_-`, at most 64 characters; a file larger than 64 KiB, a link or a bad name is refused; every field is bounded), sends the step to the window as the app event `screens-driver-step` and writes `replies/<id>.json` atomically (a temporary file in the same directory, then a rename). The page answers through the command `screens_driver_reply`; only the window the step was sent to may answer it, once. A request for a page that has not said it listens stays in place until it does. Garbage gets an error reply naming a category (`bad_json`, `unknown_step`, ...), never a panic and never a write outside the directory (tests on every case in `request.rs` and `tests/ipc_screens_driver.rs`). Without the variable the driver is idle.

**The gate has three layers, all off by default.**
1. *Web:* `KINIGE_SCREENS_DRIVER=1` points `virtual:kinige-driver` (`web/vite.config.ts`) at `scripts/screens/driver.ts`; otherwise at a module that exports `undefined`. The driver gets `listen` and `invoke` from `main.tsx`, so its event and command names are not in a normal bundle.
2. *Rust:* the cargo feature `screens-driver` of `src-tauri` compiles the module and the command. `build.rs` refuses the feature unless `KINIGE_ALLOW_SCREENS_DRIVER=1`, and reads the driver's capability (`capabilities-driver/screens-driver.json`: the reply command and the event subscription for the three app windows) only with the feature, through the glob `./capabilit*/**/*`. Without the feature the command does not exist, has no permission and no capability.
3. *Bundle:* `scripts/check-test-hooks.sh` (the old fake microphone guard, widened) searches `web/dist` and the executable for the marker `kinige-screens-driver-92` and fails `build-app.sh` and `build-windows.sh` (so `release.yml` too) unless `KINIGE_ALLOW_SCREENS_DRIVER=1`; the scripts also refuse the variable `KINIGE_SCREENS_DRIVER=1` up front. `--expect` is the positive control the scenarios run, `scripts/check-test-hooks.test.sh` and the CI step that builds the feature without the allow variable and expects `build.rs`'s message are the negative controls. Only `screens.yml` sets the allow variables, and only for the scenarios that need the driver.

**Scenarios.** A scenario sets `DRIVER=1` (shell) or calls `Enable-Driver` (PowerShell) and uses `drive`, `press`, `wait_name`, `has` (`Drive`, `Press`, `Wait-Name`, `Has`); `shoot` reads the window text through the driver before each shot (about 100 ms where a tree dump took up to 3 minutes). Old scenarios keep their input and move when they are next touched. AX and `SendKeys` stay for native UI: file pickers, the tray and native menus, resizing a window, global hotkeys (never simulated).

## What the probe found (`docs/screens/screens-driver-probe*/facts.txt`)
PROBE_FACTS

## Consequences
- The capture window may hear the driver's steps in a driver build only; `tests/ipc.rs` and `tests/ipc_settings.rs` skip the "capture has no event subscription" check there, and keep it in a default build.
- A driver build is not a build for people: its `Info.plist` and installer are the screens runners' and are never published.
- Not verified: a driver step on a window that is hidden or minimised (the scenarios bring the window to the front first).
