# Slice 0: Skeleton

Budget: 4 days. Depends on: Q8 (decided: `kinige-claude`), a repository on the homelab Gitea (created by the user).

## Scenario
There is no user scenario here; there is a developer scenario. It also serves as the acceptance criterion:
1. I clone the repository onto a clean Mac with Rust, Node and Xcode CLT (versions pinned in `rust-toolchain.toml` and in the Node version file) and run one command from the README.
2. Dependencies install, the app builds, and one Kinige window opens: background and text from tokens, the line “Kinige <version>” obtained from Rust through `KinigeApi`.
3. With a second command I open Storybook in a browser and see the story «Каркас / Пустое окно» (“Skeleton / Empty window”) on `MockApi`, which looks like this window.
4. I change a token value in `design/tokens/*.json` and rebuild: the color changes both in the window and in Storybook.
5. I run the screenshot script: a PNG of the real window, not of the whole screen, lands in the given folder.
6. I open a PR on Gitea: CI (Gitea Actions, Linux) is green.
7. Locally I run `scripts/build-app.sh`: a `.app` is built and signed ad-hoc, and `codesign --verify` passes.

## Included
- Git branch `slice/0-skeleton`, remote is the repository on the homelab Gitea. Cargo workspace: `crates/kinige-domain` (one function, needed by the command, no IO) and `src-tauri/` (Tauri 2.12.x, one command); frontend `web/` (React + TS + Vite).
- `KinigeApi` (interface), `TauriApi` (invoke) and `MockApi` with one method. Generating TS types from Rust: choose `ts-rs` or `tauri-specta` and write an ADR. CI checks that the generated types are up to date.
- A capability for the main window with an explicit list of commands (not “all commands”). CSP without inline scripts (§6).
- Token pipeline: `design/tokens/*.json` (DTCG) → Style Dictionary 5.x → `web/src/ui/tokens.css`. Two or three temporary role-based tokens (`color.surface.background`, `color.text.primary`, `font.size.body`). Slice D will replace their values.
- Storybook with one story on `MockApi`. A visual snapshot of this story in CI.
- **CI is Gitea Actions on a Linux runner** (on every PR): `cargo test`, `clippy -D warnings`, `rustfmt --check`, `cargo build -p kinige-domain --target wasm32-unknown-unknown`, `tsc`, ESLint, frontend tests, Storybook visual tests (headless browser on Linux; the snapshot baselines are taken there too), a check that `tokens.css` and the generated types are up to date. Check whether the homelab has an `act_runner`; if not, bring one up in Docker on ubuntu-server (container `kinige-act-runner`) and describe it in the README.
- **The `.app` build is local**, with the script `scripts/build-app.sh`: release build, ad-hoc signature, `codesign --verify`. A macOS runner comes no earlier than slice 10.
- Visual tests run locally in the same Linux container as in CI (Docker is remote: files go through the build context or `docker cp`), so that the baselines match.
- `scripts/screenshot-window.sh <out.png>` finds the Kinige window and captures it with `screencapture -l <windowid>`.
- Measurement method (machine, release build, number of runs) in the README, and a script that measures cold start (target < 1.5 s, §15).
- ADR “Signing, notarization and updater keys” (§12.3): where they live, who has access, how to revoke them.
- README (requirements, commands) and the build and test commands section in `CLAUDE.md`.

## Not included
- **ADR “protocol + keys”**: a separate task, ADR-0001, after slice 0 is accepted, in parallel with slice D1, before slice 1 (user decision).
- `kinige-store`, SQLite, migrations, `kinige-sync`, `server/` (slices 1 and 11). The load data generator: it needs the schema.
- The real design system: palette, fonts (Q11), type scale, components, the large-text mechanism, three feed variants (slice D).
- Sidebar, feed, composer, Inbox (slice 1). Capture window, hotkeys, tray, single-instance (slice 2).
- The IPC matrix “window → commands” with negative tests (§12.3) arrives with the second window in slice 2.
- Developer ID, notarization, updater (slice 10), Windows and Linux builds (MVP+). `docs/architecture.md` and `docs/roadmap.md`: nothing to add to §3 and §15 of the PRD yet.

## What “done” looks like
One main window with the standard macOS frame. The whole area is filled with `color.surface.background`. At the top left, a single line “Kinige 0.1.0” in `color.text.primary` at size `font.size.body`. There are no other elements and only one state. There are no design system components yet. The Storybook story «Каркас / Пустое окно» (“Skeleton / Empty window”) matches the window except for the version line, which reads “mock”. Colors, sizes and fonts outside `tokens.css` do not appear in any file under `web/src`.

## Checks
- Rust: unit test of the `kinige-domain` function, test of the command. wasm32 build in CI.
- Frontend: a test on `MockApi` that the skeleton renders the version line. A visual snapshot of the story in one theme and normal text (large text comes with D).
- CI: a link to a green run in the Gitea PR. A step with a deliberately broken token or type turns red: check this once and describe it in the PR.
- Screenshots: `docs/screens/0/window.png` taken by the script, next to it `storybook.png` and the output of the cold start measurement.
- No data is written: a kill-the-process check is not needed.
- Docker is remote (ubuntu-server): no bind mounts of Mac paths (files through the build context or `docker cp`), prefix `kinige-`, temporary containers are cleaned up afterwards; `kinige-act-runner` is permanent.

## Touches sensitive areas
- IPC: yes, the first capability and the CSP. A `slice-reviewer` review is enough.
- Schema, sync, encryption: no (ADR-0001 is a separate task).
- The signing keys ADR concerns CI secrets. Secrets do not go into the repository or `.env` (per `research/tauri-stack.md`, the updater key does not work through `.env`).

## Decisions (19.10.2026)
1. ADR “protocol + keys” is a separate task, ADR-0001, in parallel with slice D1, before slice 1; reviewed by `sync-crypto-guardian` and `codex-second-opinion`.
2. Repository and CI: Gitea on the homelab, checks on a Linux runner; the `.app` is built and signed locally. The “signing keys” ADR starts from this: there are no signing secrets in CI yet.
3. Budget: 4 days.
4. Large text in snapshots starts with slice D1.

## Open questions
None. Precondition: the user creates an empty `kinige-claude` repository on Gitea and provides the remote address.
