# D1 palette: graphite (neutral graphite)

There is one palette: `graphite` (dark, gray surfaces with no color tint, a light blue accent). Chosen by the user on 20.10.2026 (ADR-0004), after the dark themes of ChatGPT and Claude. The other candidates were deleted together with their tests and stories: the original `umber`, `moss`, `dusk`, `catppuccin-mocha` and `catppuccin-macchiato`, as well as the blue-graphite Nord, which was the D1 palette before this decision (the file `nord.json` was renamed to `graphite.json`, the surfaces were replaced). The roles are described in `PALETTE_ROLES` (`web/src/ui/tokenSource.ts`), the contrast is checked by `web/src/ui/tokens.test.ts` (output: `docs/screens/D1/tokens-contrast.txt`). Which palette is the default is set by `../active.json`.

## Where things come from

- **Surfaces, text and the divider are original.** Neutral gray, saturation 0 (all three color channels are equal; the test checks this). The reference is the dark themes of ChatGPT (around `#212121`) and Claude; the surfaces are even darker, as the user asked ("darker and whiter").
- **The accent and the focus ring are the official `nord8` `#88c0d0`**, as before.
- **The error color is an adjusted `nord11`** (`#bf616a`, Aurora red), lightness 56.5 % → 73.5 %: `#d89fa4`.

The official Nord values for verification are in `design/sources/official-palettes.json` (only `nord8` and `nord11`; source: the `nordtheme/nord` repository, file `src/nord.css`, commit `1cef71605416a222e57225b544540ce0fcec18d4`, branch `develop`, package 0.2.1; the same values are on https://www.nordtheme.com/docs/colors-and-palettes, checked 20.10.2026; license per `package.json`: MIT).

The test `web/src/ui/palettes.provenance.test.ts` keeps the labeling honest (`$extensions.kinige.origin` on each role, visible in Storybook under "Основа / Цвета" (Foundation / Colors)):

- "авторский" (original) — the color is a neutral gray and matches no Nord color (it can't be passed off as official);
- an official Nord name (`nord8 …`) — the color must match the source by value and by name;
- "подгонка" (adjusted) — the color must not match any official one.

## What was changed after looking at the live window (20.10.2026)

1. First revision: official Nord, entry plate `#3b4252`. From the user: "the background is too bright, the font is gray".
2. Second: the same blue-graphite tones (220°), but darker, the text almost white.
3. Third (this one): **neutral graphite**, without the blue. The tint of the surfaces is gone; the light blue accent stays.

Hierarchy (checked by the test): the window background is darker than the feed paper, the entry plate is lighter than the paper, the code block is darker than the plate, a highlighted plate is lighter than a normal one, a focused field is lighter than a field.

## Roles and contrast

The user's targets and the result on the `surface.message` plate (`#2b2b2b`):

| Color                      | Target | Result  |
|----------------------------|--------|---------|
| `text.primary` `#f5f5f5`   | ≥ 12:1 | 12.99:1 |
| `text.secondary` `#dedede` | ≥ 9:1  | 10.52:1 |
| `text.tertiary` `#ababab`  | ≥ 5:1  | 6.17:1  |
| `status.error` `#d89fa4`   | ≥ 5:1  | 6.35:1  |
| `accent.default` `#88c0d0` | ≥ 5:1  | 7.08:1  |

| Role                                                               | Hex       | Origin                                                                                                                                                      |
|--------------------------------------------------------------------|-----------|-------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `surface.background`                                               | `#141414` | original, lightness 8 %                                                                                                                                     |
| `surface.paper`                                                    | `#1b1b1b` | original, 10.6 %                                                                                                                                            |
| `surface.message`                                                  | `#2b2b2b` | original, 16.9 %; 1.22 times lighter than the paper                                                                                                         |
| `surface.code`                                                     | `#171717` | original, 9 %; 1.27 times darker than the plate                                                                                                             |
| `surface.day`, `surface.selected`, `surface.card`, `surface.field` | `#2b2b2b` | original: the same value as `surface.message`; `surface.card` is the "On this day" card (slice 5)                                                           |
| `surface.field-active`                                             | `#343434` | original, 20.4 %; 1.14 times lighter than the field                                                                                                         |
| `text.primary`                                                     | `#f5f5f5` | original, 96 %                                                                                                                                              |
| `text.secondary`                                                   | `#dedede` | original, 87 %                                                                                                                                              |
| `text.tertiary`                                                    | `#ababab` | original, 67 %                                                                                                                                              |
| `accent.default`                                                   | `#88c0d0` | **nord8**, official                                                                                                                                         |
| `border.divider`                                                   | `#333333` | original, 20 %; the sidebar border                                                                                                                          |
| `highlight.background`                                             | `#3a3a3a` | original, 22.7 %; 1.25 times lighter than the plate, text on it ≥ 4.9:1                                                                                     |
| `highlight.row`                                                    | `#474747` | original, 27.8 %; the search row selected with the arrow keys, 1.52 times lighter than the plate (threshold 1.5), primary text on it 8.5:1, secondary 6.9:1 |
| `highlight.mark`                                                   | `#555555` | original, 33.3 %; the found word (slice 4), 1.8 times lighter than the plate, primary text on it 7.0:1                                                      |
| `focus.ring`                                                       | `#88c0d0` | **nord8**, official                                                                                                                                         |
| `status.error`                                                     | `#d89fa4` | adjusted **nord11**                                                                                                                                         |

In total, of 17 roles: 2 official (`accent.default`, `focus.ring`), 1 adjusted Nord (`status.error`), 14 original.

The roles `border.control` (field border and empty checkbox) and `highlight.border` were removed: fields have no outline (ADR-0004), the empty checkbox and the quote bar are drawn with `text.tertiary`. When a field is focused, the thin line is drawn by `focus.ring`.

## How far the palette departs from §11.2 (PRD v0.7)

Neutral gray with no tint; a muted light blue accent; no purple and no warm tones. The "warm dark paper" principle was canceled in v0.7 (ADR-0004).
