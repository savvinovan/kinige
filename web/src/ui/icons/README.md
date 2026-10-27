# Значки

Линейные SVG из набора [Lucide](https://lucide.dev) через npm-пакет `lucide-react` (версия закреплена в `web/package.json`, на момент подключения 1.60.0). Лицензия пакета — ISC; значки, унаследованные от проекта Feather, — MIT (текст обеих лицензий лежит в `node_modules/lucide-react/LICENSE`, копия в репозиторий не кладётся: пакет — зависимость).

| Значок в `Icon` | Lucide | Лицензия значка |
|---|---|---|
| `attach` | `paperclip` | ISC |
| `emoji` | `smile` | MIT (из Feather) |
| `voice` | `mic` | ISC |
| `search` | `search` | MIT (из Feather) |
| `chevron` | `chevron-down` | MIT (из Feather) |
| `send` | `arrow-up` | MIT (из Feather) |
| `copy` | `copy` | ISC |
| `edit` | `pencil` | ISC |
| `task` | `square-check` | ISC |
| `favorite` | `star` | ISC |
| `more` | `ellipsis` (прежнее имя `more-horizontal`) | MIT (из Feather) |
| `done` | `check` | MIT (из Feather) |
| `close` | `x` | MIT (из Feather) |
| `alert` | `circle-alert` (прежнее имя `alert-circle`) | MIT (из Feather) |
| `info` | `info` | MIT (из Feather) |
| `trash` | `trash-2` | MIT (из Feather) |
| `calendar` | `calendar` | MIT (из Feather) |
| `pin` | `pin` | ISC |
| `unpin` | `pin-off` | ISC |
| `archive` | `archive` | ISC |
| `unarchive` | `archive-restore` | ISC |
| `plus` | `plus` | MIT (из Feather) |
| `collapse` | `panel-left-close` | ISC |
| `expand` | `panel-left-open` | ISC |
| `undo` | `undo-2` | ISC |
| `chevronRight` | `chevron-right` | MIT (из Feather) |
| `database` | `database` | MIT (из Feather) |

Происхождение значков слайса 3 сверено со списком Feather в `LICENSE` пакета 1.60.0: `trash-2`, `calendar`, `plus`, `chevron-right`, `database` там есть, остальные — ISC.

Какие значки произошли от Feather, видно по списку в `LICENSE` пакета; проверено по версии 1.60.0.

Правила: размер — токен `icon.size`, толщина линии — `icon.stroke`, цвет — `currentColor`; значок всегда декоративный (`aria-hidden`), подпись для диктора ставится на кнопку или поле. Эмодзи как декор интерфейса запрещены (§11.2 ТЗ); эмодзи чатов — данные пользователя, их рисует системный эмодзи-шрифт (исключение описано в `design/tokens/fontsets/inter.json`).

## Значки menu bar (трей)

Знак Kinige для menu bar macOS — страница журнала с буквой K, рисованный вручную (без шрифтов и сторонних лицензий): `tray/tray.svg` (контур, обычная иконка) и `tray/tray-flash.svg` (залитая страница с вырезанной буквой, короткое мигание при успешном сохранении буфера). Это template-изображения: только чёрное с альфа-каналом, macOS сама перекрашивает их под светлую и тёмную строку меню. PNG в `src-tauri/icons/` (`tray.png` 22×22, `tray@2x.png` 44×44, `tray-flash.png`, `tray-flash@2x.png`) получены из SVG командой `rsvg-convert -w <22|44> -h <22|44> tray/<имя>.svg -o <файл>.png` (librsvg 2.60.0, из Homebrew). Иконка приложения в `src-tauri/icons/` пока заглушка (сплошной тёмный квадрат), поэтому знак нарисован отдельно; когда появится настоящая иконка, глиф трея стоит свести с ней.
