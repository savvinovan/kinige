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

Какие значки произошли от Feather, видно по списку в `LICENSE` пакета; проверено по версии 1.60.0.

Правила: размер — токен `icon.size`, толщина линии — `icon.stroke`, цвет — `currentColor`; значок всегда декоративный (`aria-hidden`), подпись для диктора ставится на кнопку или поле. Эмодзи как декор интерфейса запрещены (§11.2 ТЗ); эмодзи чатов — данные пользователя, их рисует системный эмодзи-шрифт (исключение описано в `design/tokens/fontsets/inter.json`).
