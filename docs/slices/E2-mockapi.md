# KINIG-104: tech debt E2, MockApi by areas

**Budget:** about 3 h to merge. Stop and report at 6 h.

**Task:** Huly KINIG-104, under KINIG-49. Branch `chore/E2-mockapi`. The PR title and the first commit start with `KINIG-104: …`. No release: nothing changes for the user.

`MockApi` is what every story and every frontend test runs on: about 1 300 lines in a chain of seven classes, one per slice (`MockApiBase` → `Slice4` → `4a` → `4b` → `5` → `6` → `7` → `MockApi`). Slice 11 would add another link. The rules it repeats from Rust drift silently, because nothing compares the two.

## Included
- **Composition by area instead of inheritance by slice.** Split it into modules by what they do (for example chats, entries, trash, search and import, settings, agents, attachments and voice, forest), over one shared state object. `MockApi` stays the one class that `implements KinigeApi`.
- **The public surface does not change.** Everything the stories and tests call stays with the same name and meaning (`failNext`, `openedUrls`, `addAgentLog…`, `mockMessage`, `mockAttachment`, `DEFAULT_SETTINGS`, `MockApiOptions`, and the rest). If a test has to change, say why in the PR body. Files stay under 400 lines.
- **Shared JSON fixtures for the rules the mock repeats.**
  - First list the rules in `web/src/api/Mock*.ts` and `mock*.ts` that claim to do what Rust does (comments like "as in Rust", "like `LOG_PAGE`").
  - Keep the ones where a difference would mislead a test or a story. Candidates: the chat colour rule and Inbox staying grey, the page sizes, the trash days, date validity (`isRealDate`), the append target, the reply quote states (`live` / `trash` / `missing` / moved), the attachment error codes.
  - For each kept rule, one file under `fixtures/api-contract/` with input and expected output, read by both a vitest test on the mock and a `cargo test` on the real rule.
  - Where the mock is deliberately simplified (the entry day in UTC, rough plain text), do not pin it. Say so in the mock's comment and list it in the PR body.
  - Where the fixture shows that the mock and Rust already disagree, change the mock to match Rust, and list each case in the PR body. Rust is not changed in this task.
- The fixtures are synthetic. No new dependency.

## Not included
- Any change to what the app does, to the Rust rules, or to `KinigeApi` or `TauriApi`.
- Any change to the stories' look: the `visual` job passes with the existing baselines, and `visual-update` is not run. If a baseline would have to change, that is a finding to report.
- The Rust debt: that is E1 (KINIG-103), in parallel. Do not touch `crates/*/src` or `src-tauri/src`. The Rust side of the fixtures is one new test file per crate, or one in total.

## What "done" looks like
- CI is green on all jobs, with the `visual` job against unchanged baselines.
- No `MockApiSlice*` class is left, and no file in the mock is over 400 lines.
- Each fixture file is read by both vitest and `cargo test`. A deliberate wrong value in a fixture fails both sides (shown once in the PR body).
- The PR body lists: the rules pinned, the rules left simplified on purpose, and every disagreement found and fixed.

## Touches sensitive areas
No: test support code only. `slice-reviewer` reviews.
