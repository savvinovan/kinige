# KINIG-87: image viewer, close on click, zoom, rotate

Budget: ~3 h to a green CI with screens; stop and report at 6 h. Depends on: slice 7 (`Lightbox`, ADR-0014) on `main`. Huly: KINIG-87; branch `feat/viewer-zoom-rotate`, PR and first commit `KINIG-87: …`. Release: a patch (ADR-0012). Frontend only: no Rust, no schema, no IPC change. Source: user feedback of 05.11.2026 (the viewer closes only through the cross; no zoom; upside-down photos cannot be turned).

## Scenario
1. I open a photo from the feed. I click the dark area around it: the viewer closes. I open it again and click the photo itself: it closes too (after a short wait of 0.3 s, see "Decisions").
2. I open it again and press the rotate button (or `R`): the photo turns 90 degrees counter-clockwise; a landscape photo now fits the window height. Another press turns it again. I switch to the next photo: it is upright again.
3. I hold Cmd (Ctrl on Windows and Linux) and scroll the wheel over the photo: it zooms around the cursor. The toolbar shows "150%". A plain wheel does nothing.
4. I drag the zoomed photo: it pans and cannot be pulled off the screen. A click on the photo at this point does nothing; a click on the dark area still closes. A double click returns it to fit; another double click zooms to 2× at the cursor.
5. `+`, `-` and `0` step the zoom and reset it. Clicking "150%" resets it to fit. Zoom stays between 100% (fit) and 800%.
6. Clicking an arrow, the counter, the toolbar or "Open in default app" never closes the viewer. Esc still closes it. Nothing about the turn or the zoom is saved: after closing and reopening, the photo is upright and at fit.

## Included
- **Closing:** a press and release that moved less than 4 px closes the viewer when it started on the backdrop (the scrim, the empty stage, the empty bar), or on the photo while it is at fit size. While the photo is zoomed it runs under the bar, so the empty part of the bar counts as the photo then (a click there does nothing; at fit size it closes). A close that waits for a double click is cancelled by any zoom, pan or turn inside the wait (key, button, Cmd/Ctrl + wheel, pinch), and the timer checks once more, when it fires, that the photo is as it was at the click. Started on a button, the counter or the toolbar: never. A drag (4 px or more) never closes. Decided on pointer events at the dialog root (the stage covers the scrim, so a scrim click handler alone never fired).
- **Zoom (`useViewerTransform`, `viewerMath`):** Cmd/Ctrl + wheel (a non-passive native listener on the dialog, so the page zoom is prevented) and the WebKit `gesturechange` pinch zoom around the cursor; plain wheel untouched. Double click toggles fit and 2× at the cursor. Keys `+`/`=`, `-`, `0`; the toolbar buttons zoom in and out by the ladder 100, 150, 200, 300, 400, 600, 800%. Between fit and 8× (`clampZoom`). Transitions (`motion.duration.short`, zero under Reduce Motion) only for button, key, double click and rotate steps; wheel and drag follow the input directly. A drag pans when zoomed (cursor grab / grabbing), clamped so that an edge of the photo never comes inside the viewport (a photo smaller than the viewport does not move). The percentage in the toolbar is relative to fit (fit = 100%); clicking it resets to fit.
- **Rotate:** a rotate icon button and the `R` key (also the Cyrillic layout key, by `code`) turn 90 degrees counter-clockwise, cumulative so that the transition takes the short way. View only: not stored. The fit size is computed for the turned photo (`viewerMath.fitScale`), zoom and pan work in the turned frame.
- **Reset:** switching to another photo resets zoom, pan and rotation; closing unmounts the viewer.
- **Toolbar:** counter on the left; on the right "Open in default app", rotate, zoom out, the percentage, zoom in, the cross. Icons from the existing Lucide set: `RotateCcw`, `ZoomOut`, `ZoomIn` (README of the icons; the library line in `ATTRIBUTION.md` already covers Lucide). Every button has an accessible name (ru/en) and a tooltip with its key. The zoom buttons are muted (`aria-disabled`) at the limits. No outlines beyond the app's `:focus-visible` rule (`base.css`).
- **Stories:** zoomed (150%), zoomed and panned (300%), rotated, rotated and zoomed (200%), 800% in English (zoom in muted), large text zoomed; the states are reached by a scripted key press and drag right after mount (`Scripted` in the stories file). **Tests:** see "Checks".
- **Docs:** PRD §6 gets a "Photo viewer" bullet; the icons README gets the three icons; one new token, `layout.attachment.lightbox-percent-width`.

## Not included
- Plain wheel to step between photos, keyboard panning with arrows (arrows keep stepping between photos), pinch on touch screens, a minimap, rotation by an arbitrary angle, saving the rotation to the file or the database (EXIF) or syncing it, zoom of the feed thumbnails, a zoom level kept between photos or between openings.
- The "No preview" and "File is missing" tiles: no zoom and rotate buttons (nothing to turn); they still close on a click.
- Video and audio players, the voice notes slice (7a).

## What "done" looks like
1280 × 800 window with the viewer open. Toolbar as in "Included", the percentage readable at normal and large text. Screens (`docs/screens/viewer/`, `scripts/screens/viewer.sh`, macOS runner, AX input only, so the toolbar buttons): 01 viewer at fit with the new toolbar; 02 rotated; 03 rotated and zoomed to 200%; 04 the same in large text; 05 next photo (back at fit and upright); 06 the feed after closing (by a backdrop click if an AX press can reach it, otherwise the cross; the answer is written to `backdrop-click.txt` and the click closing is covered by vitest only); 07 English. Stories: the states above; baselines via `visual-update.yml`.

## Checks
- **vitest (`LightboxClose.test.tsx`, `LightboxZoom.test.tsx`, `LightboxRotate.test.tsx`, `viewerMath.test.ts`, the Tab order in `Lightbox.test.tsx`):** close on a backdrop click and on an image click at fit (after the double click window); no close after a drag, after a click on the zoomed image, on a toolbar, arrow or counter click; Cmd and Ctrl + wheel zoom around the cursor, plain wheel does not; keys `+ - 0 R`; zoom limits; pan clamp; rotation and the fit scale after a turn (landscape fits the height); reset on switching photos; double click; maths on garbage input (`NaN`, `Infinity`, huge deltas, zero-size stage, missing natural size).
- **CI:** `npm run typecheck`, `lint`, `test`, `build-storybook`, the `visual` job.
- Pinch zoom on a real WKWebView and WebView2 trackpad: **not verified** (no input device on the runner; the handlers are covered by synthetic events).

## Touches sensitive areas
No: no schema, sync, keys or IPC. No new packages; three Lucide icons from the installed package.

## Decisions made by the coordinator on 05.11.2026 and by the implementer
1. Closing, zoom, rotation and toolbar as in the task (coordinator).
2. A click on the photo at fit size and a double click on it need each other's input: the close is therefore delayed by `DOUBLE_CLICK_MS` (300 ms; 250 in the first version, raised after the first review to sit closer to a normal double click) and cancelled by a second click; backdrop clicks close at once (implementer).
3. Cmd or Ctrl are accepted on every platform (`ctrlKey || metaKey`): Chromium (WebView2) delivers a trackpad pinch as a wheel with `ctrlKey`; Safari (WKWebView) delivers `gesture*` events, so both are handled (implementer).
