# Kinige v2

Личный журнал в формате «чата с собой». Десктоп на Tauri 2 + Rust + React/TypeScript. Нормативный документ — [`docs/plan/02-prd.md`](docs/plan/02-prd.md).

Состояние: слайс 0 (каркас). Окно показывает строку «Kinige <версия>» из Rust через `KinigeApi`.

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
| Снять скриншот окна Kinige | `scripts/screenshot-window.sh docs/screens/0/window.png` |
| Замерить холодный старт | `scripts/measure-cold-start.sh [прогонов]` |
| Визуальные тесты Storybook (Linux-контейнер) | `scripts/visual-tests.sh` |
| Обновить эталоны снимков | `scripts/visual-tests.sh --update` |
| Пересобрать `tokens.css` и `themes.css` из `design/tokens/**/*.json` | `npm --prefix web run tokens` |

Остальные проверки (те же, что в CI) — в разделе «Сборка и тесты» файла [`CLAUDE.md`](CLAUDE.md).

## Устройство

- `crates/kinige-domain` — правила предметной области, без IO, собирается под `wasm32`.
- `src-tauri/` — тонкий слой Tauri: окно, команды, capability главного окна (`capabilities/main.json`), CSP.
- `web/` — React + TS + Vite; `src/api` (`KinigeApi`, `TauriApi`, `MockApi`, сгенерированные типы), `src/ui/tokens.css` (генерируется).
- `design/tokens/**/*.json` — токены в формате DTCG → Style Dictionary → `web/src/ui/tokens.css` (роли, `:root`) и `web/src/ui/themes.css` (`@font-face`, Reduce Motion). Цвета, размеры и шрифты вне этих файлов в `web/src` запрещены (проверяет тест `no-raw-values`).
- Дизайн-система D1: палитра Nord (`design/tokens/palettes/nord.json`; источник и три подогнанные роли — в `README.md` рядом) и набор шрифтов Literata + Inter + JetBrains Mono (`design/tokens/fontsets/literata.json`; файлы — `web/src/ui/fonts/`, источники и лицензии — в его `README.md`); оба выбраны пользователем (ADR-0004), `design/tokens/active.json` указывает на них. Эмодзи чатов рисует системный эмодзи-шрифт (единственное исключение из «шрифты встроенные»). Значки — Lucide через `lucide-react` (`web/src/ui/icons/README.md`).
- Плотность и размер текста: все размеры шрифта, отступы, радиусы, высоты строк и значки в rem, где 1rem = размер текста записей (`font.root.default`, 14 px); смена корневого размера меняет всю плотность целиком. ВРЕМЕННО: в панели Storybook «Размер текста» 13 / 14 / 15 px (атрибут `data-text-size`) — кандидаты на выбор пользователя, после выбора лишние удаляются. `data-text-scale="large"` на `<html>` умножает корневой размер (`font.scale.large`, ×1,25). Reduce Motion: `prefers-reduced-motion` или атрибут `data-reduce-motion`.
- Экран в приложении сейчас — временное главное окно на `MockApi` (слайс D1); слайс 1 заменяет его настоящим.
- Типы Rust → TS: `ts-rs`, см. [ADR-0002](docs/adr/0002-ts-types-via-ts-rs.md). Ключи подписи: [ADR-0003](docs/adr/0003-signing-keys.md).

## CI (Gitea Actions, Linux)

Workflow — [`.gitea/workflows/ci.yml`](.gitea/workflows/ci.yml): `rust` (fmt, clippy, test, wasm32, актуальность TS-типов), `web` (актуальность `tokens.css` и `themes.css`, tsc, ESLint, vitest), `visual` (снимки Storybook в контейнере Playwright).
`.app` собирается и подписывается локально; macOS-раннер — не раньше слайса 10.

Раннер: постоянный контейнер `kinige-act-runner` (act_runner 0.6.1) на ubuntu-server, зарегистрирован на репозиторий. Поднять или пересоздать (токен регистрации — в Gitea: Настройки → Actions → Раннеры; в репозиторий не кладётся):

```sh
GITEA_RUNNER_REGISTRATION_TOKEN=<токен> docker compose -f ci/act-runner/compose.yaml -p kinige-act-runner up -d
```

## Методика замера холодного старта

Цель — < 1,5 с (§15 ТЗ). Замер: `scripts/measure-cold-start.sh 10` после `scripts/build-app.sh`.
- Машина и ОС выводятся скриптом; результаты фиксируются вместе с ними.
- Только release-сборка (`target/release/kinige`), 10 прогонов, каждый — новый процесс, между прогонами пауза 1 с.
- Измеряется время от запуска процесса до события загрузки страницы в WebView (Rust печатает метку при `KINIGE_TIMING=1`). Это верхняя граница времени до первого кадра с содержимым. Кэши ОС не сбрасываются (для сброса нужен `sudo purge`); поэтому отдельно указывается первый прогон, а решающая цифра — медиана.
- Результат слайса 0: [`docs/screens/0/cold-start.txt`](docs/screens/0/cold-start.txt).

## Правила

См. [`CLAUDE.md`](CLAUDE.md): один слайс = одна ветка = один PR; визуал только из токенов; без правок исходников скриптами замены.
