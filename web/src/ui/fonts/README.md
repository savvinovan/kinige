# Fonts (D1)

The "sans + mono" set: `inter` (Inter for entry text and the interface + JetBrains Mono for code). Chosen by the user on 20.10.2026 after looking at the live window (ADR-0004): at first Literata was used for entry text, but at 14 px its serifs are noisy, so it was removed together with its files, the `font.family.serif` tokens and the entries in tests; earlier, the candidates `source` (Source Serif 4 / Sans 3 / Code Pro) and `plex` (IBM Plex) were removed. Both families are licensed under the SIL Open Font License 1.1; the text is in `OFL.txt` next to the files of each family.

## Where they come from

All files come from the Google Fonts repository, commit `47d54ca1ebb14fd864695f1d39dc734f39017c33` (branch `main`, 2026-10-16), folder `ofl/<family>/`: `https://raw.githubusercontent.com/google/fonts/47d54ca1ebb14fd864695f1d39dc734f39017c33/ofl/<family>/<file>.ttf`. `OFL.txt` and `METADATA.pb` were taken from the same place (the original sources below come from them).

The downloaded TTFs were kept in a separate folder; nothing from them was executed. The only transformation is packaging TTF into WOFF2 with the fontTools 4.67.0 library (brotli 1.2.0): `TTFont(src)`, `font.flavor = "woff2"`, `font.save(dst)`. No subsetting was done; the files were renamed (brackets and commas removed from the names). Besides packaging, fontTools drops the `DSIG` table on save (a digital signature; it becomes invalid after any re-save anyway) and rewrites the modification date `head.modified`. Glyph outlines, the axes of the variable fonts and the `name` table were not changed by the packaging (for the first two files this was checked with fontTools: 0 differing glyphs; the Inter italic was added on 20.10.2026 the same way, its glyphs were checked only by the test and the screenshots).

| Folder            | Family         | Original source (from METADATA.pb), commit     | Author           | TTF in Google Fonts, sha256 (first 16 characters)         |
|-------------------|----------------|------------------------------------------------|------------------|-----------------------------------------------------------|
| `inter/`          | Inter          | github.com/rsms/inter, `66647c0b` (Inter 4.1)  | Rasmus Andersson | `29160a80ff49ddca` (upright), `acd98e64795781b2` (italic) |
| `jetbrains-mono/` | JetBrains Mono | github.com/JetBrains/JetBrainsMono, `19371302` | JetBrains        | `48715a42ec242c21`                                        |

Inter's `OFL.txt` from Google Fonts matches the file in the repository (checked with `diff`).

## What was checked in the files

The check is the test `web/src/ui/fonts.test.ts` (it reads the woff2 files themselves: the `cmap`, `OS/2`, `fvar`, `name` tables) and, independently, fontTools 4.67.0 (the results matched). For each file: the whole Russian alphabet `А–я`, `Ё`, `ё` is present; at least 90 code points in the U+0400–U+04FF block; the license string inside the file contains "Open Font License"; the italic flag and the weight in the file match those declared in the token. The test requires from the set: sans regular, italic, semibold, bold and bold italic; mono regular and bold.

The set is described in `design/tokens/fontsets/inter.json`: families (with system fallbacks) and faces (`fontFace`: family, style, weight or range, file). All files are variable: one weight axis covers regular / semibold / bold.

| Role          | File                                          | Weight in the file                                   | Cyrillic (code points in U+0400–U+04FF) |
|---------------|-----------------------------------------------|------------------------------------------------------|-----------------------------------------|
| sans, upright | `inter/Inter-Variable.woff2`                  | wght axis 100–900, opsz 14–32                        | 248                                     |
| sans, italic  | `inter/Inter-Italic-Variable.woff2`           | wght axis 100–900, opsz 14–32 (`OS/2` marked italic) | 248                                     |
| mono          | `jetbrains-mono/JetBrainsMono-Variable.woff2` | wght axis 100–800                                    | 98                                      |

Missing: mono italic (not used). All the needed regular / italic / semibold / bold faces are present, including Inter bold italic. No Reserved Font Name.

## Exception: the system emoji font

Chat emoji in the sidebar are drawn by the system emoji font (`font.family.emoji`: Apple Color Emoji, Segoe UI Emoji, Noto Color Emoji). This is the only exception to the "bundled fonts only" rule: emoji are user data (`chats.icon`), we don't bundle color emoji (the files are large, the licenses vary), and emoji look different on every OS by definition. The test `fonts.test.ts` checks that the `emoji` role has no bundled faces and that there are no other roles without files. Interface and entry text don't use system fonts.

## What is not included and not verified

- Faces that aren't among the D1 roles were not taken: Thin–Light, ExtraBold–Black, mono italic.
- Font rendering was checked in Chromium (Storybook, Linux container) and in the real Tauri window on macOS (WKWebView, `docs/screens/D1/app-main-window.png`). Windows and Linux: slice 10.
- Size: the two folders together are about 0.8 MB in woff2.
