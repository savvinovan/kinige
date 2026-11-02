# Spike S3: the forest (grid and renderer)

Date: 2026-11-02 (Huly KINIG-14, prerequisite of slice 6). Limit per the PRD: 4 days; this took well under that, most of the wall time was CI. **Outcome (02.11.2026): the user chose grid B**, Canvas 2D with a layer cache, no PixiJS: `docs/adr/0013-forest.md`, PRD v0.16 (§7.1, Q4, §14). The report below is the evidence as it was written, with the recommendation of grid A that was not taken; "What the PRD has to learn" is already absorbed into the PRD.

Code: `spikes/S3-forest/` (a standalone Vite + TypeScript page, outside the workspace and outside the app; `npm ci && npm run dev` there to look at it, `npm test` for the unit tests). The Rust candidate of the rules is `crates/kinige-domain/src/forest.rs`. Measurements run only in GitHub Actions (`.github/workflows/spike-s3.yml`, started by hand on the spike branch; it ran on every push while the spike was open): the numbers are in `spikes/S3-forest/results/` (`summary.md` has every row), the screenshots in `docs/screens/S3/`. Nothing was run on a developer's Mac and no real data was used: the year is synthetic (`syntheticYear`, a fixed seed).

## Short answer

- **Grid: A (12 x 31) is what I would take; the choice is yours.** On a 1280 x 800 window the whole year is drawn with a 55 px tile for A against 40 px for B (1.9 times the area per day), 26 against 19 px in a 640 px window. A reads "October 16" directly; B shows weekdays but needs two sets of labels and is a thin ribbon. Pros and cons are in the table below.
- **Renderer: Canvas 2D, with a layer cache for pan and zoom. PixiJS was not added.** The criterion was "add PixiJS only if Canvas 2D fails 60 FPS". Drawing the whole board every frame holds 60 FPS on macOS WebKit (the engine of Tauri's WKWebView, here in a VM) and on Linux Chromium (CPU raster) at 1x, where it already wobbles on a 2x screen (37..54); it fails on Linux WebKit (the engine of WebKitGTK) without GPU raster and on Chromium with software GL (and on the macOS VM's Chromium, which cannot draw an empty canvas at 60 either). The cache fixes Linux WebKit at 1x (36..38 to 61) and lifts software-GL Chromium from 4..7 FPS to 29..60 (two runs). What is still below 60 is Linux WebKit on a 2x screen, where even an empty canvas gets 48 FPS: that is a floor of the browser, and WebGL on such a machine goes through the same software path, so PixiJS would not fix it (details below).
- **Hit-test: correct for every day, both grids**: 21,900 clicks (5 browser configurations), 0 misses through real pointer events, plus a unit test that clicks the centre of every cell of both grids, for several years (including 2012 and 2028 with 54 columns), both first weekdays, under 23 different cameras.
- **The PRD's table has two facts wrong** (below): grid A is not "an almost square rhombus", and grid B is not always 53 columns wide.

## What was built

- Two grids in 2:1 isometry on one board: A (month x day) and B (weekday x week, Monday or Sunday first), one synthetic year (2026: 4356 notes, 284 photos, 29 empty days including a week-long gap; trees per stage 0..6: 41 56 107 74 45 29 13), seasons by date, simple procedural shapes, no external assets (nothing new for `ATTRIBUTION.md`). The forest colours are placeholders in `src/draw.ts`; everything around the canvas uses design system tokens (`web/src/ui/tokens.css`, `themes.css`, `base.css` are imported as they are).
- Hit-test by the inverse isometric transform, not by sprites (`src/iso.ts`): a tall tree overlaps its neighbours but never takes their click. Selected cell: a filled outline under the trees and a thin one over them; the trees in front of it that overlap it are drawn semi-transparent (through a scratch layer, so overlaps do not show as rings).
- Three levels of detail by the tile width on screen: dot below 14 px, simplified below 32 px, detailed from 32 px. The grid does not change between them. At 1280 x 800 the whole year is detailed for both grids, in a 640 px window it is simplified, in a 300 px high strip (the collapsed forest of PRD 7.4) A is simplified and B is dots.
- Gestures: drag = pan, wheel = zoom about the cursor, `ctrl` + wheel (how Chromium reports a trackpad pinch), `gesturechange` (how Safari and WKWebView report it; written, not verifiable headless), two-pointer pinch, double click = zoom x2, the "Whole year" button, arrows: left and right are the previous and next day, up and down are the next or previous month in A (clamped to the month's length) and the next or previous week in B, Escape clears the selection. Hover shows a card (date, notes, photos, stage, first line). The selected day is announced in a live region ("October 16, 5 notes, a large tree").
- Growth animation only when today's stage rises (700 ms, ease-out); nothing animates otherwise; with `prefers-reduced-motion` the tree jumps. The demo button "Add a note today" raises today's weight to the next stage.
- A layer cache (`drawCached` in `src/forest.ts`): the board is rendered into a bitmap a quarter of the window larger on each side; every gesture frame is one `drawImage` of the visible part; the bitmap is re-rendered when the data or selection changes, 140 ms after the camera rests, or when the view leaves it or the scale differs by more than 0.6..1.7. The hover outline is drawn over it and never re-renders the board.

## What the PRD has to learn

- **B is 7 x 53 only in most years.** A leap year whose 1 January is the last day of the week needs a 54th column (6 empty cells before 1 January + 366 days = 372 > 371): 2012, 2040, 2068 and 2096 with a Monday start, 2000, 2028, 2056 and 2084 with a Sunday start. The board width has to be a function of the year (`grid_dims`), and the first column shifts by the weekday of 1 January from year to year.
- **A is not "an almost square rhombus".** In isometry a 12 x 31 board is a diagonal strip of 1:2.6; B is 1:7.6. Neither fills a 16:10 window: the board covers about 29% of the canvas for A and 15% for B.
- A has 7 empty cells in a common year and 6 in a leap year (about 2% of the board), all at the ends of the short months: "empty ground without interaction" works (they are drawn dark and the hover and the click ignore them).
- The day position, the inverse, the season, the weight and the stage are pure functions (Rust: `forest.rs`, 7 tests; TypeScript: `calendar.ts` and `iso.ts`, 50 tests in all). The TypeScript copy keeps both grids and a 20-row golden table; after the choice of B the Rust module holds **only grid B** (the A branch, the grid enum and the A rows of the golden table were removed; A stays in the TypeScript spike) and checks the 14 B rows, plus the bijection "every day of 1990..2100 has one cell inside the board and the inverse returns the date". It is the single source of truth for slice 6 (ADR-0013), not wired into the API and without ts-rs types; slice 6 decides the IPC shape. It builds for `wasm32`.
- A garbage-input bug found by a test: the hit-test returned a cell for a `NaN` point (all range checks are false for `NaN`); it now returns nothing.

## How it was measured, and what the numbers are worth

Playwright (1.64.1, the version the repository pins) on GitHub runners: Chromium 156 and WebKit 27.2 on `ubuntu-24.04` (4 vCPU), Chromium and WebKit on `macos-15` (a 3-core Apple M1 VM). Scripted camera, three phases of 3 s each: a circular pan, a zoom sweep across all levels of detail, both together; the rAF intervals are recorded. The table cells are the **worst of the three phases** in FPS.

- It is headless: the FPS is what the renderer delivers in a loop capped at 60 Hz, an approximation of a real window, not a display measurement. Variance between CI runs of the same job is large (Linux Chromium at 2x moved from 54 to 37 FPS, Linux WebKit at 1x from 25 to 39); the text quotes ranges over the runs I looked at: read the numbers as ranges. Every row is in `results/summary.md`.
- None of these machines has a GPU. The Linux Chromium WebGL renderer string is SwiftShader (software); WebKit reports "Apple GPU" even on Linux, so its WebGL cannot be identified. The macOS Chromium cannot exceed ~40 FPS here **even when it draws nothing** (the "floor" column below), so its rows say nothing about the renderer; they are in `results/` only for completeness. The macOS WebKit rows are the closest thing to Tauri's WKWebView, in a VM.
- "xN" means the scene is drawn N times per frame: a way to see how much headroom is left under the 16.7 ms budget. "Floor" is a full-canvas fill per frame and nothing else.

### Whole board redrawn every frame (plain Canvas 2D)

| engine                        | 1280x800 A | 1280x800 B | 1280x800 @2x A | 1280x800 @2x B | 640x800 A | 640x800 B |
| ----------------------------- | ---------- | ---------- | -------------- | -------------- | --------- | --------- |
| Linux Chromium (CPU raster)   | 60         | 60         | 37             | 56             | 60        | 60        |
| Linux Chromium, software GL   | 4          | 5          | -              | -              | 8         | 41        |
| Linux WebKit (CPU raster)     | 38         | 50         | 16             | 20             | 55        | 62        |
| macOS WebKit                  | 58         | 60         | 60             | 60             | 60        | 60        |
| macOS Chromium (VM, no GPU)   | 25         | 24         | 25             | 27             | 27        | 28        |
| macOS Chromium, --disable-gpu | 28         | 27         | 27             | 23             | 25        | 28        |

### With the layer cache (pan and zoom blit a bitmap)

| engine                        | 1280x800 A | 1280x800 B | 1280x800 @2x A | 1280x800 @2x B | 640x800 A | 640x800 B |
| ----------------------------- | ---------- | ---------- | -------------- | -------------- | --------- | --------- |
| Linux Chromium (CPU raster)   | 60         | 60         | 59             | 60             | 60        | 60        |
| Linux Chromium, software GL   | 60         | 29         | -              | -              | -         | -         |
| Linux WebKit (CPU raster)     | 61         | 61         | 25             | 24             | 62        | 62        |
| macOS WebKit                  | 60         | 59         | 58             | 60             | 60        | 58        |
| macOS Chromium (VM, no GPU)   | 33         | 29         | 25             | 27             | 27        | 23        |
| macOS Chromium, --disable-gpu | 24         | 22         | 28             | 28             | 29        | 32        |

### Headroom, 2x screen, grid A (the heavier one)

| engine                        | floor | x1 | x2 | x4 | cached x1 | cached x4 | cached x8 | sprites x1 | density 1 |
| ----------------------------- | ----- | -- | -- | -- | --------- | --------- | --------- | ---------- | --------- |
| Linux Chromium (CPU raster)   | 60    | 37 | 22 | 12 | 59        | 21        | 11        | 22         | 60        |
| Linux Chromium, software GL   | -     | -  | -  | -  | -         | -         | -         | -          | -         |
| Linux WebKit (CPU raster)     | 44    | 16 | 16 | 15 | 25        | 15        | 11        | 14         | 26        |
| macOS WebKit                  | 58    | 60 | 59 | 59 | 58        | 60        | 59        | 59         | 60        |
| macOS Chromium (VM, no GPU)   | 41    | 25 | 19 | 14 | 25        | 12        | 7         | 14         | 42        |
| macOS Chromium, --disable-gpu | 40    | 27 | 19 | 17 | 28        | 13        | 6         | 14         | 42        |

"sprites x1": the detailed trees drawn from pre-rendered bitmaps, the cost model of the final art pipeline (atlas frames). "density 1": the canvas backing store at density 1 on the 2x screen (what lowering the density during a gesture would cost).

What they say:

- macOS WebKit holds 58..60 FPS in every plain case, including 2x and the x4 load, and with the cache up to x8. This is the primary platform, and a plain Canvas 2D board is enough there.
- Linux Chromium (CPU raster) holds 60 at 1x; at 2x it is 37..54 for A and 56..60 for B plain (A: x2 load halves it, so no headroom) and 59..60 cached.
- Linux WebKit (CPU raster) is the weak case: 1x plain A is 36..38 and the cache brings it to 61 (62 in the narrow window). At 2x it is 16..21 plain and 24..26 cached against an empty-canvas floor of 44..48: the 2x canvas is too big for the software path of this browser whatever is drawn on it. Dropping the backing store to density 1 gives 26..28 (grid A). Only a GPU or a smaller canvas helps; WebGL does not.
- Chromium with software GL (SwiftShader composites the page): plain in the 1280 window 4..7 FPS for A and 5..13 for B; with the cache A 47..60 and B 29..51 (the zoom phase re-renders the layer and suffers most).
- Bitmap sprites are **not** cheaper than vector shapes in Chromium's CPU path (a scaled `drawImage` costs more than a few arcs: 22..30 against 37..54 FPS at x1 for A); in WebKit, on Linux and macOS, they make no measurable difference (the floor dominates). Art atlases should come in a size close to the drawn size (one set per level of detail) so that nothing is scaled down much. Not verified with real sprites.

### Hit-test, gestures, motion

- Real pointer clicks at the centre of every day: 21,900 clicks (5 browser configurations), 0 misses. Two views per case (the whole year and a zoom to a 110 px tile), both grids, B with Monday and Sunday first, a 1x and a 2x window; Linux and macOS, Chromium and WebKit.
- Gestures and keys through real input events: 16 checks (wheel in and out, `ctrl` + wheel, double click about x2, drag pans by the drag distance and does not select, the "Whole year" button, arrows across a month boundary in both grids, Escape, the end of the year) pass in all five browser configurations.
- No gratuitous motion: 0 frames in 1.5 s of idleness, 0 frames after the growth animation ends; the growth animation draws 24..44 frames in 1.2 s (the runner's frame rate) and starts only when the stage rises (adding notes to a maxed-out day starts nothing); with reduced motion it does not animate.

## Screenshots (`docs/screens/S3/`, Chromium on `ubuntu-24.04`, synthetic data)

A and B file names start with `a-` and `b-`.

| picture                                         | file                                                                        |
| ----------------------------------------------- | --------------------------------------------------------------------------- |
| whole year, 1280 x 800                          | `a-year-1280x800.png`, `b-year-1280x800.png`                                |
| whole year, selected day (Oct 16) + hover card  | `a-year-selected-hover-1280x800.png`, `b-...`                               |
| zoomed in, tall trees in front of the selection | `a-zoom-selected-1280x800.png`, `b-...`                                     |
| narrow window 640 x 800, also with the card     | `a-year-640x800.png`, `a-year-selected-hover-640x800.png`, `b-...`          |
| collapsed strip 1280 x 300                      | `a-strip-1280x300.png`, `b-strip-1280x300.png`                              |
| three levels of detail (dot, simple, detailed)  | `a-lod-dot-1280x800.png`, `a-lod-simple-...`, `a-lod-detailed-...`, `b-...` |
| growth of today's tree, before and after        | `a-growth-before-1280x800.png`, `a-growth-after-...`, `b-...`               |
| the same year in WebKit (engine of Tauri)       | `webkit/` (whole year, selected and hover, narrow)                          |

All four seasons are visible in the whole-year pictures: winter (pale ground, snowy dark green) at the ends of the year, spring (light green, pink blossoms), summer (deep green), autumn (ochre, orange).

## A against B in practice

| criterion                   | A. 12 x 31                                                                            | B. 7 x 53                                                                                                  |
| --------------------------- | ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| tile in a 1280 x 800 window | 55 px, a stage-6 tree is about 113 px tall                                            | 40 px, about 83 px (about 0.53 of A's area per day)                                                        |
| tile in a 640 x 800 window  | 26 px (simplified trees)                                                              | 19 px (simplified trees, close to dots)                                                                    |
| tile in a 300 px high strip | 15 px (simplified)                                                                    | 11 px (dots)                                                                                               |
| reading a date              | "October 16" is row Oct, column 16; month names on one edge, day numbers on the other | needs month labels (top edge) and weekday letters (left edge); a date is found by month, then by counting  |
| seasons                     | three-row bands; winter is split between both ends of the board                       | about 13 columns each, one continuous ribbon; winter split between both ends                               |
| time reads as               | a table (a grid of months)                                                            | a river: one long axis, left to right                                                                      |
| weekly rhythm               | not visible                                                                           | visible (weekday rows)                                                                                     |
| empty cells                 | 7 (6 in a leap year), dark, ignore the pointer                                        | none inside the year; the first column has 0..6 empty cells above 1 January                                |
| board size                  | fixed 12 x 31                                                                         | 53 columns, 54 in 4 leap years per century (for a given first weekday); the first column shifts every year |
| screen use                  | strip 1:2.6, about 29% of the canvas                                                  | strip 1:7.6, about 15% of the canvas, most of the window is empty                                          |
| hit-test, 60 FPS            | same                                                                                  | same                                                                                                       |

In the screenshots A reads like a landscape with months as rows and gives the user a tree he can pick out; B reads like a timeline ribbon and each tree is small at 1280 x 800. In both, a stage-6 tree is twice as tall as its tile, so on a busy stretch the front trees cover the back ones and the day under the pointer is the *tile* under it, not the tree it seems to touch; the highlight of the tile and the hover card are what tell the user which day it is.

## Recommendation

1. **Grid A (12 x 31).** It is what the PRD expected (Q4), gives the largest trees, needs no weekday-offset logic and no 54th column, and a date can be read without a hint. What it costs: 7 dead cells, no weekdays. B's real advantages are the weekday rhythm and the "river" look; if the weekday matters to the user, B is viable but it is a third smaller at 1280 x 800 and needs two label sets.
2. **Canvas 2D + the layer cache + three levels of detail.** No PixiJS in slice 6. Revisit only if the final art needs many animated sprites per frame (wind, particles), which is not planned. PixiJS was never built, so nothing here says how it would perform; the claim is only that it would not help in the cases where Canvas misses 60 FPS.
3. A different view for the collapsed strip: an isometric board in a 250 px high strip is 15 px tiles (A) or dots (B); a flat month x day heat map is probably better there. Not tried.

## Risks and what is not verified

- **Real hardware.** Everything was measured in headless browsers in VMs without a GPU. Not measured: WKWebView inside Tauri on a real Mac (macOS WebKit in the VM is the closest), WebKitGTK on a real Linux machine, Windows WebView2. The Linux WebKit result on a 2x screen (24..26 FPS cached) is the risk to look at on a real machine before slice 6 closes.
- **Pinch.** `gesturechange` (Safari and WKWebView) is implemented from the API and has not run; the `ctrl` + wheel path (Chromium, Firefox, and how WebView2 reports it) and the two-pointer path ran only as synthetic events.
- **Art pipeline.** Placeholders only. A scaled bitmap costs differently in each engine (above); sprite sizes, atlases per level of detail, and the cost of semi-transparent front trees with real bitmaps are open. Tall sprites need the hit-test rule above to be explained in the UI.
- **Cache.** After a gesture the board is re-rendered 140 ms later (a short blur while zooming, then it sharpens). With a stage-6 tree at the edge of the layer a tree may be clipped until that re-render. A re-render on selection or data change costs one plain frame (about 1..5 ms on the CI machines).
- Keyboard mapping (left and right = day, up and down = month or week) is a proposal, not a tested design; the Forest / Calendar accessible alternative of PRD 7.4 was not built.
- Labels: month and weekday labels collide with canopies in dense stretches; they have a dark halo and are thinned on small boards. Their placement is a slice 6 design question.

## What is left for slice 6

- Wire `forest.rs` to the store (`day_activity`, weights from attachments), ts-rs types and the IPC shape; configurable `w_max`; decide where the TypeScript copy of the rules goes (a wasm build of `kinige-domain` or a generated table).
- The real view: feed link (click scrolls the feed, the current day's tree highlighted while it scrolls), the forest collapsed above the feed, season quick-zoom, the Southern Hemisphere setting, the Forest / Calendar switcher with screen reader text, `prefers-reduced-motion` and the settings switch.
- Tokens for the forest palette in `design/tokens/`, art (Codex concepts, Blender sprites), decor, the "undergrowth" later.
- Re-measure on a real Mac and a real Linux box with the real art.

