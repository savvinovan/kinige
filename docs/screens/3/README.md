# Скриншоты слайса 3

Снято в настоящем окне собранного приложения (временная база и Markdown-копия, процесс отдельный от рабочего экземпляра
пользователя) скриптом `scripts/screenshot-window.sh`. Порядок и пояснения — `live-run.txt`; вывод проверок — `tests.txt`;
холодный старт — `cold-start.txt`; папка копий и Markdown-копия после восстановления — `backups-ls.txt`.

| Файл | Что показывает |
|---|---|
| 01-main-seeded | главное окно: разделы чатов, лента, поле с кнопкой эмодзи, «Trash 2» |
| 02-menu-keyboard-open, 03-menu-arrow-make-task | меню записи с клавиатуры (Shift+F10), ход стрелками |
| 04-edit-open, 05-edit-esc-focus-on-plate, 06-edit-enter-saved | правка в плашке: открыта, Esc (фокус на плашке), Enter сохранил |
| 07-calendar-open, 08-calendar-arrows, 09-calendar-picked | календарь с клавиатуры, запись ушла на другой день |
| 10-emoji-open, 11-emoji-search-ru, 12-emoji-search-ru-cat, 13-emoji-picked-in-composer, 14-sent-with-emoji | пикер: поиск по-русски, вставка в позицию курсора, отправка |
| 15-chat-menu-open, 16-chat-rename-dialog, 17-chat-renamed | меню чата, окно переименования, результат |
| 18-tag-filter-keyboard | фильтр по тегу |
| 19-record-menu-rightclick | меню записи по правому щелчку |
| 20-deleted-plate-dimmed, 21-trash, 22-purge-confirm, 23-trash-after-purge | удалённая плашка, корзина, подтверждение «Delete forever», после |
| 24-after-kill9-relaunch | после `kill -9` и перезапуска: всё на месте, черновик в поле |
| 25-running-db-corrupt-found, 26-after-restart-startup-recovery, 27-after-restore | повреждение работающей базы -> перезапуск -> копии -> восстановлено |
| 28-tray-menu-with-chats, 29-tray-open-chat | меню значка с чатами, открытие чата из меню |
| 30-settings-backup-row | «Настройки» со строкой копии базы |
| 31-large-text-main, 32-large-text-record-menu | крупный текст |

Системный язык машины проверки — английский: подписи интерфейса на снимках английские, данные демо — русские. Русские подписи
и крупный текст всех новых компонентов — в эталонах визуальных тестов (`web/tests/visual/stories.spec.ts-snapshots`).
