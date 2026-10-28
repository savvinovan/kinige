# Kinige v2

[![CI](https://github.com/savvinovan/kinige/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/savvinovan/kinige/actions/workflows/ci.yml)
[![Coverage Rust](https://codecov.io/gh/savvinovan/kinige/branch/main/graph/badge.svg?flag=rust)](https://app.codecov.io/gh/savvinovan/kinige?flags%5B0%5D=rust)
[![Coverage Web](https://codecov.io/gh/savvinovan/kinige/branch/main/graph/badge.svg?flag=web)](https://app.codecov.io/gh/savvinovan/kinige?flags%5B0%5D=web)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.94](https://img.shields.io/badge/rust-1.94-orange.svg)](rust-toolchain.toml)

Личный журнал в формате «чата с собой». Десктоп на Tauri 2 + Rust + React/TypeScript. Нормативный документ — [`docs/plan/02-prd.md`](docs/plan/02-prd.md).

Состояние: слайс 1 (Inbox): пишешь в Inbox, запись сохраняется в SQLite и в Markdown-копию `~/Documents/Kinige`, ссылки открываются в системном браузере, настройки (папка копии, размер текста), интерфейс на английском и русском. Приёмка пользователем — по `docs/screens/1/`.

## Требования

- macOS с Xcode Command Line Tools (`xcode-select --install`).
- Rust: версия закреплена в [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` поставит её сам.
- Node: версия в [`.node-version`](.node-version) (в CI и в контейнере визуальных тестов — она же; локально проверялось также на Node 26.8.1).
- Python 3 (для замера холодного старта), Docker (для визуальных тестов; Docker на этой машине удалённый).

## Команды

| Что | Команда |
|---|---|
| Запустить приложение (dev) | `scripts/run-app.sh` |
| Открыть Storybook (http://localhost:6006) | `scripts/storybook.sh` |
| Собрать `Kinige.app` (release, ad-hoc подпись, `codesign --verify`) | `scripts/build-app.sh` |
| Снять скриншот окна Kinige | `scripts/screenshot-window.sh docs/screens/0/window.png` (окно захвата — третий аргумент `capture`, меню значка — `menu`, список окон — `list`) |
| Замерить холодный старт | `scripts/measure-cold-start.sh [прогонов]` |
| Визуальные тесты Storybook | задание `visual` в CI на GitHub |
| Обновить эталоны снимков (в ветке, не в `main`) | `gh workflow run visual-update.yml --ref <ветка>`, затем `gh workflow run ci.yml --ref <ветка>` |
| Пересобрать `tokens.css` и `themes.css` из `design/tokens/**/*.json` | `npm --prefix web run tokens` |

Остальные проверки (те же, что в CI) — в разделе «Сборка и тесты» файла [`CLAUDE.md`](CLAUDE.md).

## Устройство

- `crates/kinige-domain` — правила предметной области, без IO, собирается под `wasm32`: HLC, канонический JSON групп (JCS), группы новой записи, `journal_date`, проверка ссылок, формат Markdown-копии, типы API.
- `crates/kinige-store` — SQLite (`rusqlite`): миграции, один поток-писатель, WAL, фоновая запись Markdown-копии.
- `src-tauri/` — тонкий слой Tauri: два окна (главное и окно быстрого захвата — на macOS панель NSPanel без активации приложения, [ADR-0006](docs/adr/0006-quick-capture.md)), команды, глобальные сочетания, значок в menu bar, буфер обмена (`arboard`), плагины opener и dialog (только из Rust), `on_navigation`, у каждого окна своя capability (`capabilities/main.json`, `capabilities/capture.json`), CSP.
- `web/` — React + TS + Vite; `src/api` (`KinigeApi`, `TauriApi`, `MockApi`, сгенерированные типы), `src/ui/tokens.css` (генерируется).
- `design/tokens/**/*.json` — токены в формате DTCG → Style Dictionary → `web/src/ui/tokens.css` (роли, `:root`) и `web/src/ui/themes.css` (`@font-face`, Reduce Motion). Цвета, размеры и шрифты вне этих файлов в `web/src` запрещены (проверяет тест `no-raw-values`).
- Дизайн-система D1: тёмная палитра graphite — нейтральный графит с акцентом и ошибкой из Nord (`design/tokens/palettes/graphite.json`; происхождение цветов — в `README.md` рядом) и набор шрифтов Inter + JetBrains Mono (`design/tokens/fontsets/inter.json`; файлы — `web/src/ui/fonts/`, источники и лицензии — в его `README.md`); оба выбраны пользователем (ADR-0004), `design/tokens/active.json` указывает на них. Эмодзи чатов рисует системный эмодзи-шрифт (единственное исключение из «шрифты встроенные»). Значки — Lucide через `lucide-react` (`web/src/ui/icons/README.md`).
- Плотность и размер текста: все размеры шрифта, отступы, радиусы, высоты строк и значки в rem, где 1rem = размер текста записей (`font.root.default`, 13 px — выбран пользователем среди 13 / 14 / 15 px); смена корневого размера меняет всю плотность целиком. `data-text-scale="large"` на `<html>` умножает корневой размер (`font.scale.large`, ×1,25). Reduce Motion: `prefers-reduced-motion` или атрибут `data-reduce-motion`.
- Экран в приложении — настоящее главное окно на `TauriApi` (слайс 1); `MockApi` реализует тот же `KinigeApi` в памяти для тестов и Storybook. Строки интерфейса — `web/src/locales/{en,ru}.json` (i18next, [ADR-0005](docs/adr/0005-i18n.md)); язык выбирает Rust по языкам системы.
- Данные: база — `~/Library/Application Support/me.savvinov.kinige/kinige.db`, Markdown-копия — `~/Documents/Kinige/<чат>/YYYY/YYYY-MM-DD.md` (папку меняют в настройках). Для проверки на чистых данных: переменные окружения `KINIGE_DATA_DIR` (каталог базы) и `KINIGE_MIRROR_DIR` (папка копии по умолчанию). `KINIGE_TRACE=1` пишет в stderr жизнь окна захвата (`show`, `key`, `resign`, `hide`, `flash`; без текста записей) — для проверки в живом окне.
- Лог приложения: macOS — `~/Library/Logs/me.savvinov.kinige/kinige.log` (Windows — `%LOCALAPPDATA%\me.savvinov.kinige\logs`, Linux — `~/.local/share/me.savvinov.kinige/logs`; по документации `tauri-plugin-log`, на этих ОС не проверялось). Ротация по размеру: файл до 1 МБ, хранится до 5 файлов (текущий и архивные с датой в имени). Уровень по умолчанию `info`; `KINIGE_LOG=debug` (или `trace`, `warn`, `error`, `off`) меняет уровень для кода Kinige, чужие крейты пишут только `warn` и выше. В dev-сборке и при заданных `KINIGE_LOG`, `KINIGE_TIMING`, `KINIGE_TRACE` лог дублируется в stderr. В лог не попадают тексты записей, названия чатов, пути пользователя и содержимое импорта — только виды событий и категории ошибок. Метки `KINIGE_TIMING` и `KINIGE_TRACE` идут только в stderr, в файл не пишутся.
- Типы Rust → TS: `ts-rs`, см. [ADR-0002](docs/adr/0002-ts-types-via-ts-rs.md). Ключи подписи: [ADR-0003](docs/adr/0003-signing-keys.md).

## CI (GitHub Actions, Linux)

Workflow — [`.github/workflows/ci.yml`](.github/workflows/ci.yml): `rust` (fmt, clippy, test, wasm32, актуальность TS-типов), `web` (актуальность `tokens.css` и `themes.css`, tsc, ESLint, vitest), `visual` (снимки Storybook в контейнере Playwright), `coverage` (`cargo llvm-cov` и `npm run test:coverage` → [Codecov](https://app.codecov.io/gh/savvinovan/kinige), флаги `rust` и `web`; пока информационно, PR не блокирует).
`.app` собирается и подписывается локально; macOS-раннер — не раньше слайса 10.

## Лицензия

[MIT](LICENSE). Сторонние шрифты, значки, данные и палитры, их авторы и лицензии — в [`ATTRIBUTION.md`](ATTRIBUTION.md).

## Методика замера холодного старта

Цель — < 1,5 с (§15 ТЗ). Замер: `scripts/measure-cold-start.sh 10` после `scripts/build-app.sh`.
- Машина и ОС выводятся скриптом; результаты фиксируются вместе с ними.
- Только release-сборка (`target/release/kinige`), 10 прогонов, каждый — новый процесс, между прогонами пауза 1 с.
- Измеряется время от запуска процесса до события загрузки страницы в WebView (Rust пишет метку в stderr через `log` при `KINIGE_TIMING=1`). Это верхняя граница времени до первого кадра с содержимым. Кэши ОС не сбрасываются (для сброса нужен `sudo purge`); поэтому отдельно указывается первый прогон, а решающая цифра — медиана.
- Результат слайса 0: [`docs/screens/0/cold-start.txt`](docs/screens/0/cold-start.txt).

## Правила

См. [`CLAUDE.md`](CLAUDE.md): один слайс = одна ветка = один PR; визуал только из токенов; без правок исходников скриптами замены.
