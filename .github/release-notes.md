## Установка (macOS, Apple Silicon)

Скачайте `Kinige-*-macos-arm64.dmg` (или `.zip`) и перетащите **Kinige** в «Программы».

Сборка подписана ad-hoc, без Developer ID и нотаризации Apple (так будет до слайса 10), поэтому macOS при первом запуске остановит её. Любой из способов:

- правый клик по `Kinige.app` -> «Открыть» -> «Открыть»;
- или в Терминале: `xattr -dr com.apple.quarantine /Applications/Kinige.app`.

Проверка файла: `shasum -a 256 -c SHA256SUMS` рядом со скачанными файлами.

## Данные

- База: `~/Library/Application Support/me.savvinov.kinige/`.
- Копия записей в Markdown: `~/Documents/Kinige`.

## Что изменилось
