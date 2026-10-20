# Скриншоты D1

Снимки сделаны 20.10.2026. Файлы с префиксом `main-window-*`, `typography.png`, `colors.png` сняты в Chromium из Storybook (dev) на macOS, 2× (2560 px по ширине); эталоны визуальных тестов (Linux-контейнер, Chromium) лежат в `web/tests/visual/stories.spec.ts-snapshots/` и считаются единственным источником для сравнения. `app-main-window.png` — настоящее окно Tauri (WKWebView, dev-режим, только окно, `scripts/screenshot-window.sh`).

| Файл | Что |
|---|---|
| `main-window.png` | «Главное окно», обычный текст (14 px), 1280 × 800 |
| `main-window-large.png` | то же, крупный текст (×1,25) |
| `main-window-text-13.png`, `main-window-text-15.png` | то же при размере текста записей 13 и 15 px (кандидаты; 14 px — `main-window.png`) |
| `main-window-narrow.png` | узкое окно (ширина `layout.window.narrow`) |
| `typography.png` | «Основа / Типографика»: начертания (regular, italic, semibold, bold; sans; mono) и компактная шкала |
| `colors.png` | «Основа / Цвета»: роли Nord, образец плашек, таблица контраста |
| `app-main-window.png` | настоящее окно Tauri с временным главным окном на `MockApi` |
| `tokens-contrast.txt` | вывод теста контраста (`npx vitest run src/ui/tokens.test.ts --reporter=verbose`): все пары проходят |
| `app-empty-window-dev.png` | окно слайса 0 с новыми токенами (предыдущий круг, для истории) |

Эталоны других историй (компоненты: сообщение, лента, поле ввода, тост, сайдбар, кнопки и поля; «Основа»; экран «Главное окно» в вариантах обычный / узкий / 13 px / 15 px / пустой чат / длинный текст) — в `web/tests/visual/stories.spec.ts-snapshots/`: каждая история снимается в обычном и в крупном тексте (`*-large-*`).
