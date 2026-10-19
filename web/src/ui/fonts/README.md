# Шрифты (кандидаты D1)

Три кандидатных набора «serif + sans + mono». Выбор делает пользователь по истории «Основа / Типографика» в Storybook; после выбора остальные папки удаляются вместе с их токенами (`design/tokens/fontsets/`). Лицензия у всех — SIL Open Font License 1.1, текст лежит в `OFL.txt` рядом с файлами каждого семейства.

## Откуда взяты

Все файлы — из репозитория Google Fonts, коммит `47d54ca1ebb14fd864695f1d39dc734f39017c33` (ветка `main`, 2026-10-16), папка `ofl/<семейство>/`: `https://raw.githubusercontent.com/google/fonts/47d54ca1ebb14fd864695f1d39dc734f39017c33/ofl/<семейство>/<файл>.ttf`. Там же брались `OFL.txt`, `METADATA.pb` и `upstream_info.md` (из них — первоисточники ниже).

Скачанные TTF лежали в отдельной папке, ничего из них не запускалось. Единственное преобразование — упаковка TTF в WOFF2 библиотекой fontTools 4.67.0 (brotli 1.2.0): `TTFont(src)`, `font.flavor = "woff2"`, `font.save(dst)`. Подмножества (subsetting) не делались, файлы переименованы (убраны скобки и запятые из имён). Помимо упаковки, fontTools при сохранении изменил два служебных места: удалил таблицу `DSIG` (цифровая подпись; она была в 11 из 15 исходных файлов, подпись всё равно становится недействительной после любого пересохранения) и переписал дату изменения `head.modified`. Контуры глифов (координаты, флаги, концы контуров во всех 15 файлах), оси переменных шрифтов и таблица `name` совпали с исходными (сверено fontTools: 0 отличающихся глифов).

| Папка | Семейство | Первоисточник (из METADATA.pb), коммит | Автор | TTF в Google Fonts, sha256 (первые 16 знаков) |
|---|---|---|---|---|
| `literata/` | Literata | github.com/googlefonts/literata, `0c2761b7` | TypeTogether | `b41138c9373112f3` (roman), `d483dfaeba9cbf4c` (italic) |
| `inter/` | Inter | github.com/rsms/inter, `66647c0b` (Inter 4.1) | Rasmus Andersson | `29160a80ff49ddca` |
| `jetbrains-mono/` | JetBrains Mono | github.com/JetBrains/JetBrainsMono, `19371302` | JetBrains | `48715a42ec242c21` |
| `source-serif-4/` | Source Serif 4 | github.com/adobe-fonts/source-serif, `b3980ade` | Frank Grießhammer (Adobe) | `97b2d4da6e3cb494` (roman), `15fbc7e4679489a5` (italic) |
| `source-sans-3/` | Source Sans 3 | github.com/adobe-fonts/source-sans, `272b22b0` | Paul D. Hunt (Adobe) | `042fe2cc0b933e32` |
| `source-code-pro/` | Source Code Pro | github.com/adobe-fonts/source-code-pro, `803b7e23` | Paul D. Hunt (Adobe) | `b400fc584e10aff2` |
| `ibm-plex-serif/` | IBM Plex Serif | github.com/googlefonts/plex, `43279c15` | Mike Abbink, Bold Monday (IBM) | Regular `e882efa9c41949a5`, Italic `4b75b38be4673527`, SemiBold `27ce4d105591b48f`, Bold `534c02c295999dd8` |
| `ibm-plex-sans/` | IBM Plex Sans | github.com/googlefonts/plex, `3e312890` | то же | `3b031aa421617420` |
| `ibm-plex-mono/` | IBM Plex Mono | github.com/googlefonts/plex, `9ab3b5b3` | то же | Regular `6a3412f058c7d8df`, Bold `ac27abd6450a64dd` |

## Наборы и что проверено по файлам

Проверка — тест `web/src/ui/fonts.test.ts` (читает сами woff2: таблицы `cmap`, `OS/2`, `fvar`, `name`) и независимо fontTools 4.67.0 (результаты совпали). Для каждого файла: есть весь русский алфавит `А–я`, `Ё`, `ё`; не меньше 90 кодовых точек в блоке U+0400–U+04FF; в строке лицензии внутри файла — «Open Font License»; курсив и вес в файле совпадают с заявленными в токене.

Наборы описаны в `design/tokens/fontsets/{literata,source,plex}.json`: семейства (с запасными системными) и начертания (`fontFace`: семейство, стиль, вес или диапазон, файл).

### `literata`: Literata + Inter + JetBrains Mono

| Роль | Файл | Вес в файле | Кириллица (кодовых точек в U+0400–U+04FF) |
|---|---|---|---|
| serif, прямой | `literata/Literata-Variable.woff2` | ось wght 200–900, ось opsz 7–72 | 118 |
| serif, курсив | `literata/Literata-Italic-Variable.woff2` | ось wght 200–900, opsz 7–72 | 118 |
| sans | `inter/Inter-Variable.woff2` | ось wght 100–900, opsz 14–32 | 248 |
| mono | `jetbrains-mono/JetBrainsMono-Variable.woff2` | ось wght 100–800 | 98 |

Нет: курсива sans и mono (не используется). Все нужные regular / italic / semibold / bold есть; курсив serif любого веса 200–900 тоже (в том числе жирный курсив). Без Reserved Font Name.

### `source`: Source Serif 4 + Source Sans 3 + Source Code Pro

| Роль | Файл | Вес в файле | Кириллица |
|---|---|---|---|
| serif, прямой | `source-serif-4/SourceSerif4-Variable.woff2` | wght 200–900, opsz 8–60 | 150 |
| serif, курсив | `source-serif-4/SourceSerif4-Italic-Variable.woff2` | wght 200–900, opsz 8–60 | 150 |
| sans | `source-sans-3/SourceSans3-Variable.woff2` | wght 200–900 (экземпляр по умолчанию в файле — 200, ExtraLight; в `@font-face` задан диапазон) | 156 |
| mono | `source-code-pro/SourceCodePro-Variable.woff2` | wght 200–900 (по умолчанию 200) | 150 |

Нет: курсива sans и mono (не используется). Все нужные начертания есть. **Source Sans 3 и Source Code Pro объявлены с Reserved Font Name «Source»** (первая строка `OFL.txt`); по тексту OFL смена формата считается изменением, поэтому для них (при выборе этого набора) нужно взять официальные woff2 апстрима или убедиться в правовой чистоте конвертации. Не проверено юридически.

### `plex`: IBM Plex Serif + IBM Plex Sans + IBM Plex Mono

| Роль | Файл | Вес в файле | Кириллица |
|---|---|---|---|
| serif regular | `ibm-plex-serif/IBMPlexSerif-Regular.woff2` | 400 | 168 |
| serif italic | `ibm-plex-serif/IBMPlexSerif-Italic.woff2` | 400, курсив | 168 |
| serif semibold | `ibm-plex-serif/IBMPlexSerif-SemiBold.woff2` | 600 | 168 |
| serif bold | `ibm-plex-serif/IBMPlexSerif-Bold.woff2` | 700 | 168 |
| sans | `ibm-plex-sans/IBMPlexSans-Variable.woff2` | wght 100–700, wdth 75–100 | 192 |
| mono regular | `ibm-plex-mono/IBMPlexMono-Regular.woff2` | 400 | 168 |
| mono bold | `ibm-plex-mono/IBMPlexMono-Bold.woff2` | 700 | 168 |

Нет: курсива sans и mono, жирного курсива и полужирного курсива serif (файлы `*-BoldItalic`, `*-SemiBoldItalic` в Google Fonts есть, но не взяты; браузер синтезирует жирный курсив из курсива, если встретится `***текст***`). **Семейство объявлено с Reserved Font Name «Plex»**: то же замечание о смене формата, что у Source.

## Что не входит и не проверено

- Не взяты начертания, которых нет в ролях D1: Thin–Light, ExtraBold–Black, курсив sans/mono.
- Рендер шрифтов проверялся в Chromium (Storybook, Linux-контейнер). WKWebView (настоящее окно Tauri) на этих файлах не проверялся; переменные шрифты WKWebView поддерживает, но это ещё предстоит увидеть глазами. Windows и Linux — слайс 10.
- Размер: все девять папок вместе около 2,7 МБ в woff2. После выбора набора остальные удаляются.
