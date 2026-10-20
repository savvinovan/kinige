# Скриншоты D1 (подготовительная часть)

Скриншоты историй Storybook «Основа» не дублируются: они уже лежат в эталонах визуальных тестов (Linux-контейнер, `scripts/visual-tests.sh`), и эти файлы — единственный источник.

| История | Обычный текст | Крупный текст |
|---|---|---|
| Цвета | `web/tests/visual/stories.spec.ts-snapshots/основа-цвета--кандидаты-chromium-linux.png` | `…/основа-цвета--кандидаты-large-chromium-linux.png` |
| Типографика | `…/основа-типографика--начертания-chromium-linux.png` | `…/основа-типографика--начертания-large-chromium-linux.png` |
| Отступы | `…/основа-отступы--шкала-chromium-linux.png` | `…/основа-отступы--шкала-large-chromium-linux.png` |
| Анимации | `…/основа-анимации--длительности-chromium-linux.png` | `…/основа-анимации--длительности-large-chromium-linux.png` |

Здесь лежат:

- `app-empty-window-dev.png` — настоящее окно Tauri (dev-режим, только окно) с новыми токенами и встроенным шрифтом.
- `tokens-contrast.txt` — вывод теста контраста по всем палитрам-кандидатам (`npx vitest run src/ui/tokens.test.ts --reporter=verbose`).
