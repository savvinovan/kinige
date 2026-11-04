# Icons

Line SVGs from the [Lucide](https://lucide.dev) set via the npm package `lucide-react` (the version is pinned in `web/package.json`; 1.60.0 at the time it was added). The package license is ISC; icons inherited from the Feather project are MIT (the text of both licenses is in `node_modules/lucide-react/LICENSE`; no copy is put in the repository: the package is a dependency).

| Icon in `Icon` | Lucide                                      | Icon license       |
|----------------|---------------------------------------------|--------------------|
| `attach`       | `paperclip`                                 | ISC                |
| `emoji`        | `smile`                                     | MIT (from Feather) |
| `voice`        | `mic`                                       | ISC                |
| `search`       | `search`                                    | MIT (from Feather) |
| `chevron`      | `chevron-down`                              | MIT (from Feather) |
| `send`         | `arrow-up`                                  | MIT (from Feather) |
| `copy`         | `copy`                                      | ISC                |
| `edit`         | `pencil`                                    | ISC                |
| `task`         | `square-check`                              | ISC                |
| `favorite`     | `star`                                      | ISC                |
| `more`         | `ellipsis` (former name `more-horizontal`)  | MIT (from Feather) |
| `done`         | `check`                                     | MIT (from Feather) |
| `close`        | `x`                                         | MIT (from Feather) |
| `alert`        | `circle-alert` (former name `alert-circle`) | MIT (from Feather) |
| `info`         | `info`                                      | MIT (from Feather) |
| `trash`        | `trash-2`                                   | MIT (from Feather) |
| `calendar`     | `calendar`                                  | MIT (from Feather) |
| `pin`          | `pin`                                       | ISC                |
| `unpin`        | `pin-off`                                   | ISC                |
| `archive`      | `archive`                                   | ISC                |
| `unarchive`    | `archive-restore`                           | ISC                |
| `plus`         | `plus`                                      | MIT (from Feather) |
| `collapse`     | `panel-left-close`                          | ISC                |
| `expand`       | `panel-left-open`                           | ISC                |
| `undo`         | `undo-2`                                    | ISC                |
| `chevronRight` | `chevron-right`                             | MIT (from Feather) |
| `database`     | `database`                                  | MIT (from Feather) |
| `moveTo`       | `arrow-right`                               | MIT (from Feather) |
| `chevronLeft`  | `chevron-left`                              | MIT (from Feather) |
| `reply`        | `reply`                                     | ISC                |
| `settings`     | `settings`                                  | ISC                |
| `download`     | `download`                                  | MIT (from Feather) |
| `upload`       | `upload`                                    | MIT (from Feather) |
| `expandPane`   | `maximize-2`                                | ISC                |
| `collapsePane` | `minimize-2`                                | MIT (from Feather) |
| `hidePane`     | `eye-off`                                   | ISC                |
| `forest`       | `trees`                                     | ISC                |
| `file`         | `file`                                      | ISC                |
| `audio`        | `audio-lines`                               | ISC                |
| `video`        | `video`                                     | ISC                |

The origin of the slice 3 icons was checked against the Feather list in the `LICENSE` of package 1.60.0: `trash-2`, `calendar`, `plus`, `chevron-right`, `database` are there, the rest are ISC.

The slice 7 icons (`file`, `audio-lines`, `video`) are not in the Feather list of the package's `LICENSE` (checked against 1.60.0), so they are ISC.

Which icons came from Feather can be seen in the list in the package's `LICENSE`; checked against version 1.60.0.

Rules: the size is the `icon.size` token, the stroke width is `icon.stroke`, the color is `currentColor`; an icon is always decorative (`aria-hidden`), the screen reader label goes on the button or field. Emoji as interface decoration are forbidden (PRD §11.2); chat emoji are user data and are drawn by the system emoji font (the exception is described in `design/tokens/fontsets/inter.json`).

## Menu bar (tray) icons

The Kinige mark for the macOS menu bar is a journal page with the letter K, drawn by hand (no fonts and no third-party licenses): `tray/tray.svg` (outline, the normal icon) and `tray/tray-flash.svg` (a filled page with the letter cut out, a short flash when the clipboard is saved successfully). These are template images: black with an alpha channel only, macOS recolors them itself for the light and dark menu bar. The PNGs in `src-tauri/icons/` (`tray.png` 22×22, `tray@2x.png` 44×44, `tray-flash.png`, `tray-flash@2x.png`) were produced from the SVGs with `rsvg-convert -w <22|44> -h <22|44> tray/<name>.svg -o <file>.png` (librsvg 2.60.0, from Homebrew). The app icon in `src-tauri/icons/` is still a placeholder (a solid dark square), so the mark is drawn separately; when a real icon appears, the tray glyph should be brought in line with it.
