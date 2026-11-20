# KINIG-150: fix 5b, the top bar and small fixes

**Budget:** code 1 day (8 h: top bar 2 h, forest memory 1 h, voice on hide 3 h, page scroll 2 h), review and verification 24 h (verification has been about 3× the code). Stop and report at 2× (64 h in total).
**Task:** Huly KINIG-150, with KINIG-156 and KINIG-76. **Branch:** `fix/KINIG-150-top-bar-and-small-fixes`. The PR title and first commit start with `KINIG-150: …`. **Base:** `origin/main` at 0.9.2 (7b and KINIG-153 merged). Ships in the same delivery as 5a (reactions) and the stickers and GIFs slice; their specs are not written yet, so the overlap is a guess: likely only `MainWindowView.tsx` and the PRD, and whoever merges second rebases. **Sources:** the user's decisions of 19.11.2026 (title, forest) and the coordinator's for KINIG-156; PRD §5, §5.1, §6, §7.4, §12.3; ADR-0015 round 4; slice 7a spec, edge case 10.

## Scenario
1. I open Inbox. The thin bar at the top of the feed column, where the forest switch is, now says "Inbox" at its left end; the switch stays at the right end. I open "Проекты": the bar says "Проекты". I rename the chat: the bar follows at once. A chat with a 200-character name shows one line cut with an ellipsis, and the switch does not move. The trash and search results have no bar, as now.
2. I expand the forest into the board and press the switch: the forest disappears. I press it again: the board comes back, not the strip. With the strip it is the same: hide, show, the strip. "Hide forest" / "Show forest" in the profile menu do exactly the same.
3. I hide the expanded board, kill the app with `kill -9` and start it: the forest is hidden; one press brings the board back.
4. In "Проекты" I press the microphone and talk for 8 seconds, then close the main window with the red button (or ⌘W). The macOS microphone indicator goes off. I open the quick capture window and record a note there: it records, no "The microphone is unavailable".
5. I open the main window from the menu bar icon. In place of the field: "Recording stopped · 0:08" with the trash button and Send; the focus is on the bar, so Space or Enter does nothing by accident. Send saves the note in "Проекты". Had I recorded under a second, the line "Too short to save" would be there instead.
6. In Settings I pick large text and shrink the main window to its smallest size (640 × 400). With the forest hidden, as a strip or expanded, and with a reply strip above the field, the window never shows a scroll bar of its own: only the feed and the sidebar list scroll, and the field is whole at the bottom.

## Included
- **Title in the top bar (KINIG-150, `MainWindowView.tsx`, `main-window.css`).** The bar of KINIG-134 (`.main-window__topbar`) gets the open chat's name at its left: `displayName` from `sidebar/chats.ts` (Inbox translated), one line, ellipsis, the full name as its accessible name. The switch keeps its place and size at the right end; the bar has the same height with the forest shown or hidden and with any title. The bar is shown in every open chat (today only when `forestMenu` is passed); not in the trash or search results. The comment "There is no chat-title row above the feed" goes.
- **The forest comes back as it was (KINIG-150).** `toggleHidden` in `useForestPane.tsx` shows the last shown view (strip or expanded), not always the strip. The remembered view is device-local in `app_setting`: not synced, not written to `settings.json`. An older database with `hidden` and no remembered view shows the strip; an unknown value reads as the strip. The board / calendar mode keeps its own key, unchanged.
- **Hiding the main window stops a recording and keeps it (KINIG-156).** Closing the main window (the red button, ⌘W, which are the same `CloseRequested`) calls `voice.park()` of the main window's recorder, as the capture window does: the recording stops, the tracks and the `MicrophoneClaim` are released, and the bytes stay in the page as `lost` with `cause: "hidden"` ("Recording stopped · m:ss", Send and Discard, Esc not the bar's, focus on the bar, the trash button asks over 5 seconds). A start still waiting for the system's answer is dropped; under a second is "Too short to save"; the question "Discard the recording?" open at the hide becomes the kept recording. The window hides at once, as now. ⌘Q and a crash still lose a running or kept recording (7a). The note goes to the chat it was started in.
- **No page-level scroll in the main window (KINIG-76).** At 640 × 400 and at every text size, the document never scrolls in either direction. Cause area: the main column (`main-window.css`) stacks the top bar, the forest pane, the tag filter, the feed, the reply strip and the composer, only the feed may shrink, and the document has no guard of its own, unlike the settings window (`html[data-window="settings"]`, `prefs.css`) and the capture window (`capture.css`). The fix finds the element that overflows at large text and makes it fit; a document guard (`overflow: hidden` on the main window's `html` / `body`, `data-window="main"` set in `main.tsx`) is added as well, but alone it is not the fix, because it would cut the field off.
- **File splits (rule 6).** `src-tauri/src/lib.rs` is at exactly 400 lines and `capture.rs` at 469: any change to either splits it in this PR. Recommended: the main window's close handling goes to a new `src-tauri/src/main_window.rs` and `on_window_event` calls it, so `capture.rs` is not touched.
- **Documents:** PRD, next free version at merge: §6 ("No duplicates": the bar above the feed carries the chat name and the forest switch), §7.4 (showing brings back the view it had), §5 (closing the main window stops and keeps a recording, as §5.1), the history line. ADR-0015 round 5 (reverses 7a's "hiding the window keeps recording"). `CLAUDE.md` status in the first commit.

## Not included
- KINIG-43, undo of settings changes: a task of its own.
- Reactions (5a, KINIG-138), stickers and GIFs, sync (slice 11).
- In the bar: the chat's emoji, a status line, a chat menu or any click action on the title, search in the chat. The native window title stays "Kinige".
- A forest state per chat, or synced.
- Keeping a recording across ⌘Q, a crash or a restart; recording while the window is closed; any change to the capture window or to the main window's "Microphone disconnected" bar.
- A new minimum window size; scroll fixes in the settings and capture windows.

## Reference (UI work)
- **Telegram desktop:** the chat header holds the chat name at the left in a heavier weight, with a grey status line under it ("last seen", "N members"), and search, call and menu buttons at the right; a click on the name opens the chat's info (the version is not verified). **We copy** the name at the left end of a single quiet bar, one line, cut with an ellipsis, buttons at the right. **We change:** no status line and no click on the name (a journal chat has no status, and the PRD has no chat info panel; the chat menu stays in the sidebar), no divider under the bar (KINIG-134's bar has none).
- **Voice on hide:** Telegram desktop's behaviour when its window is closed mid-recording is not verified; we copy our own capture window (PRD §5.1).

## Decisions left to the implementer
- **Where the shown view lives (local data, not schema).** Recommended: `set_forest_view` also writes a key `forest_shown_view` in the same transaction whenever the view is `strip` or `expanded`, and `Settings` carries it as `forestShownView` (never `hidden`). No migration, no new command, the command's arguments unchanged; MockApi repeats the rule. Why: one write keeps the two keys consistent, and a database without the key reads as the strip. The store test that lists the written keys (`forest_target.rs`) changes with it.
- **How the page learns of the close (IPC).** Recommended: the new close handler emits a `main-window-hiding` event to `main` before `window.hide()`, and `useVoice` calls `park()` on it. `main` already listens to Rust events, so no capability changes; an `emit` can be observed in a mock-runtime test, unlike `eval`. Not `visibilitychange`: WebKit may report the page hidden in other cases (minimized, ⌘H, possibly a fully covered window, not verified), which the open question leaves to the user. Rust does not release `main`'s claim itself: the page lets go after its recorder has stopped, so the capture window never opens the device while main still records. **Not verified:** that `MediaRecorder` finishes in a hidden web view; the real-window scenario proves it. If it does not, the page parks first and hides itself through a new `main` command, listed in the PR and in §12.3.
- **Title type:** an existing type token picked by `design-system` (recommended `type.nav`, the sidebar's chat-name style), as a heading element for the screen reader. A new token only through `design/tokens/`.
- **Which element gives way at 640 × 400:** recommended the feed first, then the expanded board (it is already a share with `overflow: hidden`); the top bar and the composer stay whole.

## What "done" looks like
- **The bar:** chat name left, switch right, no border; states: Inbox, a short name, a 200-character name, en and ru, large text, a 640 px window, forest shown and hidden. Stories next to the KINIG-134 ones in `MainWindow.slice6.stories.tsx`.
- **The forest:** the existing pane, unchanged except what "show" restores.
- **Voice:** the existing `RecordingBar` in its kept state in the main window; no new component.
- **Minimum size:** new stories of the main window in a 640 × 400 frame (forest hidden, strip, expanded; with a reply strip), normal and large text.

## Checks
| Scenario step / rule         | Test or evidence that proves it                                                                |
|------------------------------|------------------------------------------------------------------------------------------------|
| 1. Title in the bar          | vitest: `displayName`, Inbox translated, follows rename and switch; no bar in trash or search  |
| 1. Long title, fixed switch  | Playwright `visual`: one line; the switch's box is the same for any title, shown or hidden     |
| 2. Show restores the view    | vitest: expanded → hide → show → expanded, strip → strip; from the button and the profile menu |
| 2. Stored, old, garbage      | store: the key in the same transaction; no key → strip; `hidden` or garbage in it → strip      |
| 3. Survives `kill -9`        | screens: the board hidden, `kill -9`, start, show → the board; shots 04, 05                    |
| 4. Close stops, mic released | vitest (rule 12): close event → kept `hidden`, tracks stopped, `holdMicrophone(false)`         |
| 4. Edge cases of the close   | vitest: under 1 s → "Too short to save"; while starting → dropped; while asking → kept         |
| 4. Rust close handler        | mock-runtime test of `main_window.rs`: the event reaches `main` and the window is hidden       |
| 4. Real window               | screens: main closed mid-recording, then the capture window records; shots 06, 07              |
| 5. Kept recording on return  | vitest: focus on the bar, Space and Enter send nothing, Send saves in its chat; shots 08, 09   |
| 6. No page scroll            | Playwright at 640 × 400, normal and large text: no document scroll, field and switch inside    |
| 6. Real window, large text   | screens at 640 × 400: no page scroll bar in shots 10–13; the document's sizes in `scroll.txt`  |
| Garbage input                | domain table for the new key: empty, wrong case, NUL, `hidden`, an unknown word → strip        |

Screenshots in `docs/screens/5b/` (`scripts/screens/5b.sh`, macOS runner, the screens driver and the fake microphone; scroll bars always shown):
- 01 Inbox with its title, the strip; 02 a 200-character title in a 640 px window; 03 large text at 1280 × 800;
- 04 the board hidden; 05 after `kill -9`, shown again: the board;
- 06 recording in the main window; 07 the capture window recording while the main window is closed; 08 the main window reopened, "Recording stopped"; 09 the note sent;
- 10 640 × 400 large text, forest hidden; 11 the strip; 12 the board; 13 640 × 400 normal text, the board.

Nothing here is platform-specific except the close event, which Windows delivers through the same `CloseRequested`; no Windows scenario.

## Touches sensitive areas
- **Data schema:** no. One new device-local key in `app_setting` (a key-value table): no migration, not synced, not in `settings.json`.
- **Sync, encryption, server storage:** no.
- **IPC:** `Settings` gains one field, and one Rust → `main` event; no new command and no capability change. If the fallback above needs a command, the PR lists it with its negative tests.
- So `sync-crypto-guardian` is not required for this spec; the reviewer confirms the key is local only.

## Open questions
1. **Minimizing and ⌘H.** Closing the main window now stops a recording. Should minimizing it to the Dock (the yellow button, ⌘M) or hiding the app with ⌘H do the same? Recommended: no, they keep recording, as switching to another app does: the window is one click away in the Dock, and macOS keeps its microphone indicator on, so nothing records unseen in the menu bar.
   **Decided by the coordinator on 20.11.2026 (reversible, the user is told at acceptance):** the recommendation stands: minimizing and ⌘H keep recording.
