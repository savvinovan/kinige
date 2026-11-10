---
name: screens
description: Kinige window screenshots and GUI checks on GitHub runners (screens.yml on macOS and Windows), writing a screens scenario with the in-app screens driver, the fake microphone of voice-note scenarios, and Storybook visual baselines. Use when a slice needs real-window screenshots or acceptance evidence, a new or broken scenario in scripts/screens/, or new visual baselines.
---

# Screens: the real window and visual baselines

The user's Mac never runs the Kinige window for an agent (the guard in `.claude/hooks/guard.py` blocks `open …app`, `scripts/run-app.sh`, `screencapture`, `osascript`). Real-window evidence comes from GitHub runners only.

## Real-window screenshots (`screens.yml`)

Screenshots of the real window for acceptance are captured not by this Mac but by a GitHub macOS runner.
- **Run:** `gh workflow run screens.yml --ref <branch> -f slice=<id>`. It builds the `.app` and runs `scripts/screens/<slice>.sh` on synthetic data, in a temporary `KINIGE_DATA_DIR` and `KINIGE_MIRROR_DIR`.
- **Result:** it commits `docs/screens/<slice>/` to the branch. Afterwards run `git pull`; the workflow starts CI on the branch by itself.
- **Input:**
  - In a scenario that sets `DRIVER=1`, input is the screens driver (see below).
  - Otherwise it is accessibility actions only (`scripts/screens/ax.js`; shared functions are in `scripts/screens/lib.sh`), no keys or mouse.
- The workflow doesn't run in `main`.

**On Windows (slice 10a):** `gh workflow run screens.yml --ref <branch> -f slice=10a -f os=windows`.
- It builds the NSIS installer on `windows-2025`, installs it silently and runs `scripts/screens/<slice>.ps1` (Windows PowerShell 5.1).
- **Input** is the driver where the scenario calls `Enable-Driver`. Otherwise it is keys injected with `SendKeys` after `SetForegroundWindow`:
  - the runner is a disposable VM;
  - Ctrl+K then Tab stops reach the profile menu;
  - there is no Settings accelerator on Windows.
- Windows are found with plain Win32, and the screenshot is the window rectangle (`CopyFromScreen`).
- **Do not use UI Automation or a DevTools port there:** both stalled the WebView2 page on the runner (slice spec, follow-ups).
- **Data checks don't use the UI.** Python reads the database, and the Markdown copy is read from disk.
- **No personal-path check on Windows.** Its place is taken by working only in the runner temp directory, on synthetic data. A driver scenario also reads the window text before each shot.
- The commit step is shared.

**Wait for the run with one call:**
- `gh run watch <run-id> --exit-status` as a background command, or the Monitor tool;
- never a `sleep` loop.

## The screens driver (KINIG-92, ADR-0016)

A scenario that sets `DRIVER=1` (`7a`, `7a-windows`, `screens-driver-probe*`) clicks and reads by accessible name through the test build itself.

**How it is switched on:**
- `KINIGE_SCREENS_DRIVER=1` points `virtual:kinige-driver` at `scripts/screens/driver.ts`.
- `build-app.sh` and `build-windows.sh` add the cargo feature `screens-driver` of `src-tauri`. It is off by default.
- Without `KINIGE_ALLOW_SCREENS_DRIVER=1`, `src-tauri/build.rs` refuses the feature and the guard refuses the bundle. Only `screens.yml` sets that variable.

**How a step runs:**
- The script writes `requests/<id>.json` into `KINIGE_SCREENS_DRIVER_DIR`, and the app answers in `replies/<id>.json`.
- The shell side is `drive`, `press`, `wait_name` and `has` in `lib.sh`; the PowerShell side is `Drive`, `Press`, `Wait-Name` and `Has` in `lib-windows.ps1`.
- **Steps:** press, menu, type, key, wait, has, scroll, text, front. There is no script eval and no IPC passthrough.

New scenarios use the driver. AX and SendKeys are for native UI only: file pickers, the tray and native menus, resizing the window. Global hotkeys are never simulated.

## Voice-note scenarios (slice 7a, ADR-0015)

- The screens runners record from a fake microphone chosen at build time (`KINIGE_FAKE_MIC=1`, aliased in `web/vite.config.ts`).
- `scripts/check-test-hooks.sh` makes `build-app.sh` and `build-windows.sh` fail if the fake microphone reached `web/dist` without `KINIGE_ALLOW_FAKE_MIC=1`.
  - Only `screens.yml` sets both variables.
  - `bash scripts/check-test-hooks.test.sh` is its test.
  - The same guard covers the screens driver above.
- The `7a-probe` and `7a-probe-windows` scenarios record what each web view can do.

## Storybook visual baselines

- Storybook visual tests run only on GitHub, in a Playwright container. The comparison is the `visual` job in CI.
- **New baselines:** `gh workflow run visual-update.yml --ref <branch>`.
  - It captures them and commits them to the branch.
  - It is not allowed in `main`.
  - After the commit it starts CI on the branch by itself.
- Baselines are not captured on macOS or on ubuntu-server.

## UI work order (retro of slice 7a)

- Refine a design in Storybook first. Shoot the real window only before review.
- A chat-like element is judged against Telegram desktop. Put a side-by-side with the reference in the spec, and show a story screenshot to the user before building it in full.
