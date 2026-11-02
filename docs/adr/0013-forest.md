# ADR-0013: Forest grid and renderer (spike S3)

Date: 2026-11-02. Status: **accepted** (KINIG-14; the user chose grid B on 02.11.2026). Evidence: `spikes/S3-forest/README.md`, `spikes/S3-forest/results/`, `docs/screens/S3/`. PRD v0.16 (§7.1, Q4, §14) was brought in line.

## Context
Spike S3 compared two boards (A: 12 × 31 month × day; B: 7 × 53 weekday × week) and two renderers (Canvas 2D; PixiJS v8) on GitHub runners in headless Chromium and WebKit. A gives bigger trees (a 55 px tile against 40 px at 1280 × 800) and reads dates directly. The user chose B anyway, for the weekday rhythm and the continuous "river" of the year.

## Decisions

**Grid B.** Seven rows are the weekdays counted from the first weekday, one column per week. The board width is a function of the year: `cols = ceil((january_offset + days_in_year) / 7)`, where `january_offset` (0..6) is how many cells sit above 1 January in the first column. That is 53 columns, and **54** in a leap year whose 1 January is the last day of the week: 2012, 2040, 2068, 2096 with a Monday start; 2000, 2028, 2056, 2084 with a Sunday start. Cells around the year are empty ground without interaction. Grid A is documented only in the spike.

**First weekday follows the system locale; Monday by default.** Read it with `new Intl.Locale(navigator.language).getWeekInfo()` (`firstDay` is 1..7, 7 = Sunday) and fall back to the older `weekInfo` property, then to Monday. Checked in Playwright's Chromium 156 and WebKit 27.2 (Linux builds): `getWeekInfo()` exists, `weekInfo` does not, `en-US` gives 7, `ru-RU`, `en-GB`, `de-DE` give 1. Not verified in the real WKWebView, WebKitGTK and WebView2 (older engines may lack `getWeekInfo`); the Monday fallback covers them. A user setting can come later.

**Rendering: Canvas 2D with a layer cache, no PixiJS.** Pan and zoom blit a cached bitmap of the board (re-rendered when content changes, 140 ms after the camera rests, or when the view leaves it), the hover outline is drawn over it. Three levels of detail by tile width: dot below 14 px, simplified below 32 px, detailed from 32 px. Measured (worst of pan, zoom, pan+zoom, grid B unless noted): macOS WebKit 58..60 FPS plain and cached; Linux Chromium 60 at 1x; Linux WebKit without GPU raster 50 plain and 61 cached at 1x but 20 plain and 24 cached on a 2x screen (an empty canvas gets 44..48); Chromium with software GL 4..13 plain, 29..60 cached. The cases below 60 are floors of a software path; WebGL on those machines goes through the same software path (the Linux Chromium WebGL renderer is SwiftShader, WebKit hides it), so PixiJS would not fix them. PixiJS was never built or measured; revisit only if the final art needs many animated sprites per frame.

**Hit-test by the inverse isometric transform** (point → world → cell), never by sprites; a tall tree overlaps neighbours but does not take their click. 21,900 real clicks, 0 misses. A `NaN` point must give no cell.

**`crates/kinige-domain/src/forest.rs` is the single source of truth for slice 6:** `day_position`, `date_at_cell`, `grid_dims`, `season_of_month`, `day_weight`, `stage_of` (PRD §7.2, §7.3). It holds only grid B. The TypeScript copy in `spikes/S3-forest/src/calendar.ts` stays in the spike; slice 6 decides how the frontend gets the rules (a wasm build or a generated table) and adds the IPC shape and ts-rs types.

**Collapsed strip: open.** An isometric board in a 250 px high strip is 11 px tiles (dots); a flat weekday × week heat map is probably better there. Slice 6 decides.

## Not verified
- Real hardware: WKWebView inside Tauri, WebKitGTK, WebView2. The macOS and Linux runners are VMs without a GPU and headless.
- Linux WebKit on a 2x screen without a GPU (24..26 FPS cached): check on a real machine before slice 6 closes.
- Trackpad pinch through `gesturechange` (Safari, WKWebView): written from the API, never run.
- Real sprites: a scaled bitmap costs more than vector shapes in Chromium's CPU path; atlases should be close to the drawn size.
