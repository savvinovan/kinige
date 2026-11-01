## Установка (macOS, Apple Silicon)

Скачайте `Kinige-*-macos-arm64.dmg` (или `.zip`) и перетащите **Kinige** в «Программы».

Сборка подписана ad-hoc, без Developer ID и нотаризации Apple (так будет до слайса 10), поэтому macOS при первом запуске остановит её. Любой из способов:

- macOS 15 и новее: запустите `Kinige.app` один раз, затем «Системные настройки» -> «Конфиденциальность и безопасность» -> внизу «Всё равно открыть» у Kinige;
- macOS 13-14: правый клик по `Kinige.app` -> «Открыть» -> «Открыть» (на macOS 15 и новее это больше не обходит проверку);
- на любой версии, в Терминале: `xattr -dr com.apple.quarantine /Applications/Kinige.app`.

Проверка файла: `shasum -a 256 -c --ignore-missing SHA256SUMS` рядом со скачанными файлами.

## Данные

- База: `~/Library/Application Support/me.savvinov.kinige/`.
- Копия записей в Markdown: `~/Documents/Kinige`.
