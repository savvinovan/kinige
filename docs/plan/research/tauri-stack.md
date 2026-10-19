# Исследование стека: local-first заметки на Tauri 2 + React + Rust (на 2026-10-19)

Легенда: CONFIRMED = подтверждено первичным источником (офиц. доки / репозиторий / API), которое я открыл в ходе работы; UNCERTAIN = источник не открылся, противоречив, либо это моё знание без проверки; WRONG = предпосылка неверна.
Важно: WebFetch отдаёт сжатую выжимку страниц, поэтому там, где страница не показала версию/дату, я так и пишу, а не угадываю. Все числа ниже либо из источника, либо помечены как моя арифметика.

---

## 1. Tauri 2

### Версия
- CONFIRMED: ветка 2.x жива. GitHub Releases: `tauri v2.12.4` (14 окт 2026), `tauri-runtime-wry 2.12.2`, `tauri-utils 2.10.2`. https://github.com/tauri-apps/tauri/releases
- Расхождение: страница https://v2.tauri.app/release/tauri/ показывает 2.12.3 от 10 окт 2026 (скорее просто отстаёт от GitHub). Брать "2.12.x".
- CONFIRMED: параллельно идёт **Tauri v3 alpha** (`tauri v3.0.0-alpha.5`, 14 окт 2026, помечен "Latest" на GitHub, в changelog нет описания breaking changes). Для продакшена не годится; v2 остаётся стабильной веткой. Стабильной даты v3 я не нашёл (UNCERTAIN).
- Все плагины требуют Rust >= 1.90 (из страниц плагинов global-shortcut/updater/sql/single-instance).

### WebView по ОС
- CONFIRMED (https://v2.tauri.app/reference/webview-versions/): Windows = WebView2 (Chromium/Edge, на Win11 предустановлен, обновляется сам); macOS/iOS = WKWebView (версия WebKit привязана к ОС); Android = системный Android WebView; Linux = WebKitGTK (`webkit2gtk`, версия зависит от дистрибутива).
- Таблица версий на странице неполная (в ней максимум 2.36 из Ubuntu 22.04 и т.п.); актуальные версии WebKitGTK надо смотреть по целевым дистрибутивам.

### Linux / WebKitGTK: известные проблемы
- CONFIRMED (офиц. https://v2.tauri.app/develop/debug/linux-graphics/):
  - DMABUF-рендерер WebKitGTK на NVIDIA: белое окно, мерцание при ресайзе, краш на ресайзе, ошибки `AcceleratedSurfaceDMABuf ...` и `Wayland Error 71`. Обходы по порядку: `nvidia_drm.modeset=1`, `__NV_DISABLE_EXPLICIT_SYNC=1`, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (медленнее), `WEBKIT_DISABLE_COMPOSITING_MODE=1` (последний шанс). Безусловно включать не советуют.
  - WebGL/canvas могут "молча" упасть на медленный путь: высокая задержка ввода/низкий FPS при том, что остальное приложение выглядит нормально; WebGL2-контекст создаётся даже на софтварном растеризаторе; `WEBGL_debug_renderer_info` на любом Linux возвращает "Apple GPU" (маскировка), реальный GPU не определить.
  - Официальная рекомендация: для WebGL-тяжёлых видов иметь non-WebGL fallback на Linux и пользовательский переключатель.
- Шрифты/рендеринг: есть issue tauri#14286 "Font-weight offset of 100 on webkitgtk (linux)" и #6224 "Incorrect DPI scaling when using non-integer scaling on Linux/X11" (названия получены из поиска по GitHub; содержимое не читал, статус не проверен). WebKit Bugzilla 271714 про размытость при fractional scaling в GTK. Вывод: различия рендеринга шрифтов и дробный масштаб на Linux реальны, но чёткой "официальной" страницы про шрифты нет. UNCERTAIN в деталях.
- Производительность WebKitGTK в целом: утверждение, что WebKitGTK исполняет WebGL-вызовы на главном потоке UI, а Chromium выносит в GPU-поток, нашлось только в стороннем блоге (instagit.com). UNCERTAIN.
- Tray на Linux: CONFIRMED, события мыши по иконке не эмитятся (иконка и контекстное меню работают). https://v2.tauri.app/learn/system-tray/
- WebRTC/камера на WebKitGTK: потребовала сборки WebKitGTK с флагами (discussion #8426, 2023-24). Для нас не критично.

### Выход: CEF (важно для риска Linux)
- CONFIRMED (https://v3.tauri.app/develop/cef/): в Tauri **3.0 alpha** появился runtime на Chromium Embedded Framework (`tauri-runtime-cef`, alpha.6 на 14 окт). В Tauri 2.0 недоступен. Плюс: одинаковый Chromium на всех десктопах, CDP, Chromium sandbox. Минус: бандл на сотни МБ больше, больше памяти, первая сборка скачивает ~1 ГБ CEF на платформу. Мобайл остаётся на wry.
- Есть сторонний порт `SableClient/tauri-runtime-cef` для опубликованных crates (не официальный).
- Вывод: если Linux-рендеринг окажется болью, путь отхода это v3+CEF, а не v2.

### Tauri 2 Mobile
- CONFIRMED (https://v2.tauri.app/blog/tauri-20/, пост 2.0 stable): iOS/Android поддерживаются, но команда сама пишет, что "not completely happy about the developer experience", не все официальные плагины работают на мобайле, `tauri-action` не умел автосборки для мобайла (на момент поста; актуальность не проверял), "Not all plugins are as stable as Tauri itself". Свежих официальных заявлений о повышении зрелости я не нашёл (UNCERTAIN).
- Нативный код: плагины содержат Kotlin (Android, `@TauriPlugin`, `@Command`) и Swift (iOS, Swift package, `@objc func x(_ invoke: Invoke)`); Rust вызывает через `run_mobile_plugin`. CONFIRMED: https://v2.tauri.app/develop/plugins/develop-mobile/
- Share Extension / виджеты (iOS): официального руководства я не нашёл. Это отдельные Xcode-таргеты (Share Extension, WidgetKit) в сгенерированном `gen/apple` проекте с App Group для обмена данными; сторонние примеры: плагин `tauri-plugin-share` (описан docs.rs как требующий создания Share Extension target в Xcode и передачу файлов через app group) и скрипт `setup-ios-share-extension.sh` в приложении DecentPaste. Проблема из источника: `gen/apple` регенерируется, версии/настройки дрейфуют. UNCERTAIN, это ручная нативная работа, не "из коробки". Для Android share target делается через intent-filter в манифесте (моё знание, не проверял).

### Плагины
- global-shortcut: CONFIRMED только десктоп (`#[cfg(desktop)]`, зависимость под macOS/Windows/Linux), права выдаются явно (`global-shortcut:allow-register` и т.д.). https://v2.tauri.app/plugin/global-shortcut/ Нюанс Wayland (глобальные хоткеи там ограничены протоколом) в источнике не проверялся. UNCERTAIN.
- tray: CONFIRMED, feature `tray-icon` у `tauri`; см. Linux-ограничения выше.
- clipboard-manager: CONFIRMED, есть права `allow-read-image` / `allow-write-image`; на Android/iOS только plain-text. https://v2.tauri.app/plugin/clipboard/ (сигнатуры image-API на странице не показаны, смотреть API reference).
- drag&drop файлов: CONFIRMED, `dragDropEnabled` включён по умолчанию, генерирует DragDropEvent; на Windows его нужно отключить, чтобы работал HTML5 drag&drop во фронтенде (Tauri подменяет обработчик WebView2). Т.е. внутренний DnD (перестановка) и файловый DnD из ОС конфликтуют на Windows. https://v2.tauri.app/reference/config/
- single-instance: CONFIRMED, десктоп; на Linux через DBus, для snap/flatpak по умолчанию не работает без доп. настроек (slot/`--talk-name`). https://v2.tauri.app/plugin/single-instance/
- updater: CONFIRMED, подпись обязательна и не отключается (приватный ключ через env `TAURI_SIGNING_PRIVATE_KEY`, `.env` не работает); можно статический JSON (S3/GitHub Releases) либо свой сервер (204 если нет обновления) - подходит под домашний сервер. Примеры только desktop. https://v2.tauri.app/plugin/updater/
- sql plugin vs свой rusqlite: tauri-plugin-sql CONFIRMED построен на sqlx (SQLite/MySQL/Postgres), миграции на Rust, по умолчанию права только close/load/select, для записи нужен `sql:allow-execute`; про FTS5/расширения страница ничего не говорит. Он отдаёт SQL фронтенду, что для нашего случая (FTS, векторы, sync-логика в Rust) лишний слой. Рекомендация: rusqlite в Rust-ядре, фронт ходит через собственные typed-команды. Это моё архитектурное суждение, не факт из доков.

### Модель безопасности
- CONFIRMED (https://v2.tauri.app/security/capabilities/): capabilities в `src-tauri/capabilities`, привязка к label окон/вебвью (не к title), слияние прав при нескольких capability, по умолчанию все зарегистрированные команды вызываемы из всех окон (ограничивается `AppManifest::commands` в build.rs), удалённым URL доступ только через `remote.urls`; на Linux и Android iframe неотличим от окна. Capabilities не защищают от дыр в Rust-командах, слишком широких scope и уязвимостей самого WebView.
- CONFIRMED (https://v2.tauri.app/security/csp/): CSP применяется только если задан в конфиге; Tauri сам добавляет nonce/hash для локальных скриптов и стилей; рекомендовано максимально строго, без CDN. 
- Из поиска (не первичный источник, UNCERTAIN в версии): в мае 2026 был фикс, при котором ядро стало применять ACL к IPC-запросам с удалённых origin даже без AppManifest (источник: all-versions страница changelog).
- Untrusted HTML/Markdown: специальной офиц. страницы нет (не нашёл). Практика: рендерить MD в HTML и прогонять через санитайзер (см. п.2), строгий CSP (`default-src 'self'`, без `unsafe-inline` для script, `img-src` с `asset:`/`data:` по необходимости), не давать окну с пользовательским контентом лишних capabilities, внешние ссылки открывать через opener-плагин, а не навигацией в том же вебвью.

---

## 2. Редактор и Markdown

- Obsidian на CM6: CONFIRMED, цитата с https://docs.obsidian.md/Plugins/Editor/Editor+extensions: "Obsidian uses CodeMirror 6 (CM6) to power the Markdown editor"; Live Preview меняется через editor extensions.
- `@codemirror/lang-markdown`: CONFIRMED, MIT; `markdownLanguage` по умолчанию включает GFM (+ subscript/superscript/emoji), `commonmarkLanguage` строгий; парсер на Lezer. Репозиторий на GitHub **архивирован** и перенесён на code.haverbeke.berlin (сам CodeMirror переехал с GitHub), учесть при слежении за релизами. https://github.com/codemirror/lang-markdown
- `@codemirror/view` актуален: в changelog запись 6.43.0 от 2026-05-14; в форуме упоминается 6.43.1 (https://codemirror.net/docs/changelog).
- Live-preview = decorations (replace/mark/widget) по синтаксическому дереву; это ровно путь Obsidian. Тут и основной риск IME.
- IME / кириллица: прямых багов именно с кириллицей не нашёл (UNCERTAIN, по русской раскладке отдельных репортов нет; русский ввод это обычный посимвольный ввод без композиции, проблемы IME в основном касаются CJK и диакритики/dead keys). Подтверждённые IME-проблемы CM6: (a) в Safari 26.5 на 6.43.1 композиция рядом с widget может пересоздавать `cm-widgetBuffer` (форум discuss.codemirror.net, тред ещё открыт); (b) mark decoration, оборачивающая несколько replace-decoration, ломает selection при IME; (c) в 6.39.16 исправлена порча DOM при смене decorations прямо перед завершением композиции. Позиция автора: трогать DOM/selection во время композиции нельзя (иначе композиция прерывается). Практика: не менять decorations во время `view.composing`. Ссылки: https://discuss.codemirror.net/t/ime-input-may-break-when-cursor-is-adjacent-to-widget-due-to-cm-widgetbuffer-reconstruction/9799
- Альтернативы (по одной строке):
  - Milkdown: WYSIWYG-фреймворк на ProseMirror + remark, MIT, ~12k звёзд (https://github.com/Milkdown/milkdown); версию не удалось прочитать.
  - ProseMirror: низкоуровневый rich-text toolkit (основа Milkdown/TipTap), документ-модель не равна Markdown-тексту, нужна сериализация туда-обратно (моё знание).
  - Lexical (Meta): фреймворк редактора, импорт/экспорт JSON/Markdown/HTML, MIT, ~24k звёзд (https://github.com/facebook/lexical); тоже не text-first.
  - Вывод: если "сырой Markdown с live-preview как в Obsidian" это цель, то CM6; WYSIWYG-модели создают риск расхождения исходного Markdown при round-trip (моё суждение).
- Рендер ленты:
  - markdown-it: активен, но на GitHub нет Releases (страница пустая; репозиторий упоминает v15 и migration guide); точную версию подтвердить через npm не удалось (403). Расширения через плагины; "Safe by default" заявлено в README.
  - unified/remark/micromark: экосистема активна (обсуждения в org micromark до июля 2026); AST (mdast/hast) удобнее для кастомных узлов (теги, упоминания, спойлеры) и санитизации на уровне дерева. Точные версии remark не подтверждены (на странице релизов remark-cli 12.0.0 / 12.0.1, это CLI).
  - Санитизация: rehype-sanitize CONFIRMED: работает по allow-list схеме (GitHub-подобный `defaultSchema`), ставить после последнего небезопасного шага, префиксует `id`/`name` как `user-content-` против DOM clobbering, расширение схемы расширяет поверхность атаки. https://github.com/rehypejs/rehype-sanitize
  - DOMPurify CONFIRMED: v3.4.16, активно правится (в README недавние GHSA, 3.4.12-3.4.14 - ужесточения; security-релизы анонсируются только в рассылке), тесты на Chromium/Firefox/WebKit. https://github.com/cure53/DOMPurify Ключевой момент: сам по себе DOMPurify это "парсинг в DOM и чистка", для прилагаемых Markdown-вставок нужен пакет "рендер -> DOMPurify -> innerHTML", а не наоборот.
  - Рекомендация: хранить исходный Markdown, рендерить в HTML (markdown-it без raw HTML, `html:false`, ИЛИ remark без rehype-raw), затем DOMPurify как второй слой; плюс строгий CSP. Это совет, не факт из доков.

---

## 3. Виртуализация ленты в React

- TanStack Virtual: CONFIRMED (https://tanstack.com/virtual/latest/docs/api/virtualizer и /docs/chat): динамические высоты через `measureElement`, `scrollToIndex({align})`, `shouldAdjustScrollPositionOnItemSizeChange`; для чата/prepend есть `anchorTo: 'end'` (при prepend запоминает видимый элемент по ключу и корректирует offset), нужен стабильный `getItemKey` по id, `followOnAppend`, `scrollToEnd`, `isAtEnd`. Версии на 16 окт 2026: `@tanstack/virtual-core 3.19.6`, `@tanstack/react-virtual 3.14.21`. В какой версии появился `anchorTo`, источник не говорит (UNCERTAIN), поэтому фиксируйте версию и тестируйте сценарий "подгрузка вверх" на своих данных. На iOS WebKit scroll-writes откладываются при touch/momentum (из docs).
- virtua: CONFIRMED (https://github.com/inokawa/virtua): MIT, динамические размеры измеряются из DOM, reverse scroll (проп `shift` для prepend; README говорит о "scroll position adjustment while reverse scrolling"), smooth scroll, `scrollToIndex`; релиз 0.53.5 от 12 окт 2026, активная разработка одним мейнтейнером. Детали prepend-механики на странице не раскрыты (UNCERTAIN: надо читать docs/примеры chat).
- react-virtuoso: CONFIRMED лицензирование (https://virtuoso.dev/pricing): бесплатные и MIT `Virtuoso`, `GroupedVirtuoso`, `VirtuosoGrid`, `VirtuosoTable`, `Masonry`; **коммерческий только `VirtuosoMessageList`** ($168/seat/год или Pro $312/seat/год, dev-режим 30 дней без лицензии). То есть "message list коммерческий" это CONFIRMED. Обычный `Virtuoso` с `firstItemIndex` (prepend) и `followOutput` (из моей памяти; страница API отдала редирект без содержимого) = UNCERTAIN, но это штатный бесплатный путь для чата.
- Рекомендация: для pet-проекта без лицензий выбирать между TanStack Virtual (новый `anchorTo`, headless, мейнтейнится активно) и virtua (проще "из коробки", меньше кода). Для 100k сообщений ключевое не виртуализатор, а то, что данные подгружаются страницами (keyset-пагинация по `(ts,id)` из SQLite), а в DOM хранится окно.

---

## 4. Лес: рендеринг

- PixiJS v8: CONFIRMED активен: v8.22.0 (1 окт 2026), частые релизы (8.20-8.22 за август-октябрь). Рендереры WebGL и опционально WebGPU. https://github.com/pixijs/pixijs/releases , https://pixijs.com/8.x/guides/getting-started/intro
- pixi-viewport: CONFIRMED v6.0.0 "moves pixi-viewport to pixi.js v8+"; есть drag, pinch, wheel zoom, clamp. Активность не подтверждена (страница не показала дат; 136 open issues): UNCERTAIN. https://github.com/pixijs-userland/pixi-viewport Альтернатива: писать pan/zoom самому (камера = матрица, 3 жеста), это ~100 строк.
- Пригодность: ~372 тайла + спрайты это очень лёгкая сцена (моя оценка; цифр производительности из источников не получал). WebGL нужен не по производительности, а, возможно, ради эффектов (шейдеры, свечение). Для такой нагрузки Canvas 2D (с оффскрин-кэшем тайлов, `drawImage`, hit-testing по изометрической сетке математически) обычно достаточен (моё суждение, UNCERTAIN без замеров).
- WebKitGTK caveat: CONFIRMED выше (молчаливый медленный путь WebGL на Linux, рекомендация иметь non-WebGL fallback). Следствие: Canvas 2D как основной движок или Pixi с `preference: 'webgl'` плюс fallback; у Pixi v8 есть canvas-renderer fallback (мое знание, не проверял в этой сессии, UNCERTAIN).
- macOS WKWebView WebGL/WebGPU: в этой сессии не проверял (UNCERTAIN); WebGL на WebKit работает давно.

---

## 5. Rust: SQLite, FTS5, русский, векторы

- rusqlite: CONFIRMED v0.40.1 с бандлом SQLite 3.53.2 (README). Сборка `bundled` всегда задаёт `-DSQLITE_ENABLE_FTS5` (читал build.rs libsqlite3-sys): FTS5 включён. https://github.com/rusqlite/rusqlite
- Свой FTS5-токенизатор из rusqlite: в списке feature'ов rusqlite (docs.rs/crate/rusqlite/latest/features) нет `fts5`/`tokenizer`; есть `vtab` (read-only виртуальные таблицы), `loadable_extension`, `load_extension`. То есть высокоуровневого API токенизаторов нет. Путь: либо сырой FFI (`fts5_api` через `sqlite3_prepare("SELECT fts5(?1)")` pointer-passing; моё знание, не проверял), либо C-расширение. UNCERTAIN по трудозатратам, но это нетривиально.
- Встроенные токенизаторы (CONFIRMED, https://www.sqlite.org/fts5.html): `unicode61` (по умолчанию, регистронезависим, корректно режет кириллицу как буквы L*, diacritics убираются только у латиницы), `ascii`, `porter` (только английский), `trigram` (подстроки, `case_sensitive`/`remove_diacritics`, ускоряет LIKE/GLOB; запросы короче 3 символов ничего не находят в FTS, для LIKE без 3 символов линейный скан). Доки не указывают, делает ли `unicode61` регистр для кириллицы; "case-insensitive" заявлено без ограничения латиницей (на практике работает, но проверьте тестом: UNCERTAIN).
- Русская морфология:
  - Вариант A (рекомендую): хранить в FTS5-таблице **пред-стеммленный текст** (Rust `rust-stemmers`, Snowball; Russian в списке алгоритмов CONFIRMED, https://github.com/CurrySoftware/rust-stemmers, свежесть коммитов/версию не подтвердил), и стеммить запрос тем же кодом; индекс `unicode61`. Плюсы: чистый Rust, без FFI, кроссплатформенно (включая мобайл). Минусы: колонка дублирует текст (лучше `content=''`/contentless-delete >=3.43.0 либо external content и отдельная колонка stem), подсветка/сниппеты по оригиналу требуют доп. работы. Моя рекомендация, не факт.
  - Вариант B: C-расширение `abiliojr/fts5-snowball` (CONFIRMED, BSD-3, `tokenize='snowball russian'`, проект маленький, 22 коммита, загрузка из кода "untested" по README). Для Tauri кроссплатформы и мобайла - лишняя сборка C.
  - Вариант C: trigram как дополнительный индекс для подстрок/опечаток (CONFIRMED семантика). Дороже по размеру индекса (источник цифр не даёт).
  - Комбинация: stem-FTS для основного поиска + trigram по необходимости.
- sqlite-vec: CONFIRMED (https://github.com/asg017/sqlite-vec): README "pre-v1, expect breaking changes"; последний стабильный релиз v0.1.9 (31 мар 2026), далее только alpha v0.1.10-alpha.1..4 (до 18 мая 2026); Rust crate `sqlite-vec`, регистрируется через `sqlite3_auto_extension` (пример с rusqlite bundled + zerocopy, https://alexgarcia.xyz/sqlite-vec/rust.html); лицензия Apache-2.0/MIT; спонсор Mozilla Builders. KNN: `vec0` (быстрее/компактнее) или ручной запрос с `vec_distance_cosine()` = brute-force (CONFIRMED). Бенчмарк-чисел в доках нет; в репо есть файлы ivf/diskann (признак работ над ANN, без пояснений - UNCERTAIN).
- Brute-force косинус по f32 blob'ам: арифметика памяти (моя): 100 000 x 1024 x 4 Б = ~410 МБ; x1536 = ~614 МБ; x3072 = ~1,23 ГБ; при 256 измерениях = ~102 МБ. OpenAI `dimensions` позволяет укорачивать вектор (CONFIRMED, https://developers.openai.com/api/docs/guides/embeddings; при ручной обрезке надо ре-нормализовать). Времени на скан я не измерял, цифр из источников нет - надо замерить прототипом. Для 100k разумно: хранить нормализованные векторы (косинус = скалярное произведение), держать матрицу в памяти в Rust (mmap/Vec<f32>/либо f16/int8 квантование), скан с SIMD; sqlite-vec нужен только если хочется SQL-слияния с метаданными. Начинать с brute-force в Rust: нет зависимости от pre-v1 расширения.

---

## 6. OpenRouter embeddings

- CONFIRMED эндпоинт: `POST https://openrouter.ai/api/v1/embeddings`; список моделей: `GET https://openrouter.ai/api/v1/embeddings/models`. https://openrouter.ai/docs/api-reference/embeddings
- Совместимость с OpenAI: доки называют API целиком "OpenAI-compatible with additional OpenRouter features"; форма ответа (`object:"list"`, `data[].embedding`, `usage`) совпадает с OpenAI; для именно этого эндпоинта прямого заявления о совместимости нет (UNCERTAIN, но на практике OpenAI SDK с `base_url` обычно работает - не проверял). Поля: `model`, `input` (строка/массив/мультимодальные объекты `content`), `dimensions`, `encoding_format` (`float`|`base64`), `input_type` (`search_query`/`search_document`), `user`, `provider` (`order`, `only`, `ignore`, `sort`, `zdr`, ...). https://openrouter.ai/docs/api/api-reference/embeddings/create-embeddings
- Модели (CONFIRMED живой каталог API на 2026-10-19, 33 модели; цена = за токен x 1e6 = за 1M токенов, пересчёт мой):
  - openai/text-embedding-3-small: $0.02/1M, ctx 8192 (1536 dim по умолч., из доков OpenAI)
  - openai/text-embedding-3-large: $0.13/1M, ctx 8192 (3072 dim)
  - baai/bge-m3: $0.01/1M, ctx 8194, 1024 dim, 100+ языков, MIT (из HuggingFace)
  - intfloat/multilingual-e5-large: $0.01/1M, ctx 512
  - qwen/qwen3-embedding-8b: $0.01/1M, ctx 40960; qwen3-embedding-4b: $0.02/1M, ctx 32768. Модели `qwen3-embedding-0.6b`, упомянутой в доках как пример, в живом списке нет (UNCERTAIN, может быть переименована/снята).
  - voyageai/voyage-4 / -4-lite / -4-large: $0.06 / $0.02 / $0.12 за 1M; google/gemini-embedding-001: $0.15, ctx 20000; mistralai/mistral-embed-2312: $0.10.
  - Бесплатные: nvidia/nemotron-3-embed-1b:free, nvidia/llama-nemotron-embed-vl-1b-v2:free (text+image), liquid/lfm-2.5-embedding-350m:free.
  - Мультиязычность с русским: bge-m3, multilingual-e5-large, qwen3-embedding (заявлено разработчиками, в этой сессии метрики по русскому не проверял: UNCERTAIN); text-embedding-3 мультиязычные (общеизвестно, не проверял бенчмарком).
- Мультимодальные (картинки): CONFIRMED в каталоге: `google/gemini-embedding-2` (+`-preview`; text, image, file, audio, video; ctx 8192; $0.20/1M), `voyageai/voyage-multimodal-3.5` (text, image; $0.12/1M), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (text, image). Вставка изображения: `image_url` в массиве `content`, можно совместно с `text`. Нюанс: векторные пространства разных моделей несовместимы, выбор модели фиксирует схему (хранить `model` и `dim` рядом с вектором).
- Ориентир стоимости (моя арифметика, допущение 100 токенов на сообщение, не из источника): 100k x 100 = 10M токенов, 10M x $0.02/1M = $0.20 для 3-small, $1.30 для 3-large, $0.10 для bge-m3. Реальная длина сообщений неизвестна.
- Приватность: CONFIRMED (https://openrouter.ai/docs/guides/privacy/data-collection и .../features/zdr): OpenRouter по умолчанию не хранит промпты/ответы без явного opt-in, метаданные (токены, латентность) хранит; логирование промптов и использование данных для улучшения - opt-in; ZDR включается (1) в настройках приватности (по группам моделей), (2) через guardrail `enforce_zdr_*`, (3) на запрос `provider: {"zdr": true}` (OR-логика, нельзя снять уже включённое). ZDR касается маршрутизации инференса и не покрывает плагины/инструменты. В примере embeddings-доков также есть `provider.data_collection: "deny"`; в странице privacy он не описан (UNCERTAIN в деталях, но поле есть в примере). Итог: для личных заметок включить ZDR в аккаунте и/или `zdr:true`; обратить внимание, что при ZDR набор доступных провайдеров/моделей сужается (моё предположение, проверить).

---

## 7. Синхронизация (свой сервер на Rust)

Статусы (CONFIRMED по репозиториям/релизам, где указано):
- Custom op-log + HLC: паттерн, не библиотека. Rust-крейт `uhlc` 0.9.0 (12 янв 2026; используется в Eclipse Zenoh; лицензия на странице docs.rs не указана) https://docs.rs/crate/uhlc/latest . HLC даёт монотонные метки с учётом дрейфа часов (отбрасывает слишком "будущие" удалённые метки).
- Automerge: CONFIRMED активен, v3.5.0 (16 сен 2026), много релизов летом 2026. Документ-ориентированный CRDT с полной историей; для "100k независимых сообщений" тяжеловат по модели (один документ на сообщение или общий документ - оба неудобны; моё суждение). https://github.com/automerge/automerge/releases
- Loro: CONFIRMED `loro-crdt 1.16.4` (30 сен 2026), Rust+WASM; сильные стороны - rich text/tree/movable list; для LWW-сообщений избыточен.
- Yjs/yrs: yrs - Rust-порт Yjs, в README таблица паритета (yrs 0.21 по таблице; актуальность версии не подтверждена). Хорош для совместного редактирования текста внутри сообщения, не для реляционного набора сообщений.
- cr-sqlite (vlcn): репозиторий не архивирован; коммиты 4-10 авг 2026 (сборки: iOS sim, Windows arm64, Android ABIs, macOS headerpad) - сопровождается, но фокус на сборке loadable-расширения; в README: вставки в CRR в 2,5 раза медленнее обычных, "causal event log" только в планах v2, часть CRDT (counter, rich text) "still being implemented". Последний релиз/версию не подтвердил. UNCERTAIN в зрелости; тянет C-расширение в Tauri (включая мобайл) и привязку к схеме cr-sqlite.
- ElectricSQL: CONFIRMED, "read-path sync engine for Postgres" (shapes, нужен Postgres с logical replication), Apache-2.0; запись - отдельно через ваш backend. Не подходит: у нас SQLite+Rust-сервер, не Postgres. https://github.com/electric-sql/electric
- PowerSync: README: источники Postgres/MongoDB/MySQL/SQL Server; сервис self-host в Docker (`journeyapps/powersync-service`), есть Enterprise Self-Hosted; Rust/Tauri SDK в репозитории JS-SDK не значится. Не подходит как лёгкий вариант (нужен внешний source DB + сервис). Условия лицензии Open Edition не подтвердил (UNCERTAIN).
- Turso/libSQL: CONFIRMED, Turso Database (Rust-переписывание SQLite) не достиг 1.0, часть функций экспериментальна, объявлено заменой libSQL. Turso Sync: стратегия конфликтов "last push wins", в доках предполагается Turso Cloud; самохостинга sync-сервера в доках не упомянуто. Не подходит. https://docs.turso.tech/sync/usage
- Что подходит для "сообщения с Markdown-телом, LWW на сообщение + история ревизий": **собственный op-log с HLC**. Каждая правка = неизменяемая запись (message_id, rev_id/hlc, device_id, op-тип, payload тела/метаданных); состояние сообщения = запись с максимальным HLC (LWW), все ревизии хранятся как история (естественно ложатся в таблицу `message_revisions`), удаление = tombstone с HLC. Сервер - append-only лог + курсор `since_hlc`/seq. CRDT понадобятся только если нужно автослияние одновременных правок внутри одного текста (не нужно при LWW). Это мой вывод из требований, подтверждён лишь описанием статусов выше. Вложения: контент-адресуемые blob'ы (sha256), синк отдельно от лога.

---

## 8. Claude Design и design tokens

- Что это: CONFIRMED (https://www.anthropic.com/news/claude-design-anthropic-labs): продукт Anthropic Labs, релиз **17 апреля 2026**, на Claude Opus 4.7, в research preview для Pro/Max/Team/Enterprise (на момент анонса). Делает дизайны, интерактивные прототипы, слайды, one-pagers, лендинги, wireframes. Экспорт: Canva, PDF, PPTX, standalone HTML, папка/zip, внутренний URL; handoff-бандл для Claude Code.
- Актуальное состояние (help center, https://support.claude.com/en/articles/14604416-get-started-with-claude-design): Export: .zip, PDF, PPTX, Google Slides, standalone HTML, сторонние инструменты (Lovable, Replit, v0, Vercel, Netlify и др.), handoff в локальный coding agent или Claude Code on the web; в Claude Code команда `/design` импортирует дизайн в кодовую базу или экспортирует код как live-прототип. Включён по умолчанию для Free/Pro/Max/Team/Enterprise (Enterprise - с 15 окт 2026); отдельный claude.ai/design закрывается **14 декабря 2026**. Ограничения: нет истории версий, базовое мультипользовательское редактирование.
- Design systems: CONFIRMED (https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design): создаются из кода (ссылка/загрузка), файлов, Figma (как вход), деков, логотипов, шрифтов; выход - палитра, типографика, компоненты, layout-паттерны (для мигрируемых: guide, tokens, components); система хранится как артефакт, доступный в любом чате и Claude Code; бета. `/design-sync` в Claude Code читает токены и компоненты из React-кода (лучше всего, если дизайн-система уже в React). **Экспорта дизайн-системы файлом/в другой инструмент в статье не описано.**
- Figma: на выходе нет (в help center Figma только как вход; сторонние блоги противоречат друг другу, не использовать как источник).
- Как перенести в React-кодовую базу: (1) handoff-бандл (файлы дизайна, чат, README с инструкцией для модели) -> вставить промпт в локальный Claude Code, он реализует компоненты; (2) `/design` в Claude Code; (3) `/design-sync` в обратную сторону (код -> Design). То есть токены/компоненты в нашу кодовую базу попадут как код, сгенерированный агентом, а не как отдельный стандартизованный экспорт. Рекомендация: самим держать токены в DTCG JSON в репозитории (источник правды), а Claude Design использовать как исследовательский/прототипный слой.
- Не покрыто BAA (в справке про BAA Claude Design указан как не покрытый; из поиска, первичную страницу не открывал: UNCERTAIN).
- W3C DTCG: CONFIRMED https://www.designtokens.org/tr/drafts/format/ - это отчёт Design Tokens Community Group (не W3C Standard); последняя опубликованная версия **2025.10** (designtokens.org/TR/2025.10/format/), страница-драфт от 08.09.2026 предупреждает "Do not attempt to implement this version". Типы: color, dimension, fontFamily, fontWeight, duration, cubicBezier, number + составные (shadow, border, typography, gradient...). Стабильность 2025.10 как "первого стабильного" сама страница не подтверждает (UNCERTAIN).
- Style Dictionary: CONFIRMED v5.7.0 (16 окт 2026), активные релизы (5.4.4 - 5.7.0 с июня по октябрь); "с v4 first-class поддержка DTCG", но по самой странице полной поддержки 2025.10 ещё нет ("work in progress in v5"); 5.7.0 добавил DTCG-тип `duration`. Были security-патчи prototype pollution (5.4.4, 5.5.1, 5.5.5) - обновляться. Встроенные Swift/Kotlin форматы на открытых страницах не подтверждены (UNCERTAIN; платформы `ios-swift`/`android` в SD существуют по моему знанию). Для CSS variables сейчас (`css/variables`) - ок; Swift/Kotlin позже - через платформы SD или кастомные форматы. https://styledictionary.com/info/dtcg/ , https://github.com/style-dictionary/style-dictionary/releases

---

## 9. Codex image generation

- Реальная функция: CONFIRMED (https://learn.chatgpt.com/docs/image-generation; developers.openai.com/codex/image-generation редиректит туда): встроенная генерация изображений использует **`gpt-image-2`**; в Codex CLI вызывается естественным языком или явно `$imagegen`; референс-картинка прикрепляется `-i/--image`; доступно также в ChatGPT desktop/web и IDE-расширении.
- Лимиты: генерация засчитывается в общие лимиты Codex, расходуя их "в 3-5 раз быстрее в среднем", чем аналогичные ходы без изображения (зависит от качества и размера); для больших пакетов советуют `OPENAI_API_KEY` и API-тарификацию.
- Скилл `imagegen` (CONFIRMED, https://raw.githubusercontent.com/openai/skills/main/skills/.system/imagegen/SKILL.md): по умолчанию встроенный `image_gen` без API-ключа, сохраняет в `$CODEX_HOME/generated_images/...`; CLI-fallback `scripts/image_gen.py` (generate/edit/generate-batch) только по явной просьбе и с `OPENAI_API_KEY`; встроенные правки только по изображениям, уже находящимся в разговоре (локальные файлы через `view_image`), маски и `input_fidelity`/`quality` только в CLI; батч = по одному вызову `image_gen` на ассет; рекомендации по консистентности: перечислять инварианты ("менять только X, остальное оставить"), одна правка за итерацию, ссылаться на входные изображения по индексу с ролью.
- Прозрачный фон: страница Codex про него молчит. В API-гайде (https://developers.openai.com/api/docs/guides/image-generation) для прозрачности - `background:"transparent"` + `output_format` png/webp, но упомянуты модели `gpt-image-2.5-sunburst`/`gpt-image-2.5-flare`, а не `gpt-image-2`; поддержка прозрачности именно в `gpt-image-2` и во встроенном инструменте UNCERTAIN. Для спрайтов это критично: заложить этап вырезки фона (вариант `background-extraction` из скилла или внешний инструмент).
- Консистентность наборов: CONFIRMED офиц. оговорка API-гайда - модель "may occasionally struggle to maintain visual consistency for recurring characters or brand elements" между генерациями. Для сета деревьев: один мастер-референс стиля, вносить по одному изменению, генерировать лист из нескольких вариантов за раз, фиксировать палитру/ракурс (изометрия 2:1) в промпте. Размеры API: кратны 16, соотношение 1:3..3:1, ребро до 3840 px, 655 360-8 294 400 пикселей всего (>2560x1440 экспериментально). Точной изометрии/пиксельной сетки модель не гарантирует (моё суждение, проверять руками).
- Прим.: блог-источники упоминают запуск gpt-image-2 21 апреля 2026 - только вторичный источник.

---

## Рекомендации по стеку

1. Оболочка: Tauri **2.12.x** (стабильная), не v3 alpha. macOS первым (WKWebView) - низкий риск. Windows (WebView2) - низкий. Linux (WebKitGTK) - средний/высокий риск: закладывать тестовый прогон на NVIDIA+Wayland и fallback без WebGL.
2. Данные: rusqlite (`bundled`) в Rust-ядре, фронт общается только типизированными командами/событиями; без tauri-plugin-sql. Миграции - свои (refinery/`user_version`).
3. Поиск: FTS5 `unicode61` + пред-стеммленная колонка (Snowball Russian через `rust-stemmers`), опционально trigram для подстрок. Не тянуть C-токенизатор, пока не упрёмся.
4. Семантика: OpenRouter `/api/v1/embeddings`, модель по умолчанию `baai/bge-m3` (дёшево, 1024 dim, 100+ языков) или `openai/text-embedding-3-small` (1536, или `dimensions`=512/768 для экономии); хранить `model`+`dim` рядом; векторы нормализованные f32 BLOB; поиск brute-force в Rust. ZDR включить. Мультимодально (картинки) - позже через `google/gemini-embedding-2`/`voyage-multimodal-3.5`, но это отдельное векторное пространство.
5. Редактор: CodeMirror 6 + `@codemirror/lang-markdown` + свои decorations для live-preview; не менять decorations во время `view.composing`; тесты IME на macOS Safari/WKWebView и Windows.
6. Лента: TanStack Virtual (`anchorTo:'end'`, `getItemKey` по id) или virtua; пагинация keyset из SQLite окнами. Не брать `VirtuosoMessageList` (платный).
7. Markdown: remark/micromark (или markdown-it) без raw HTML -> DOMPurify (+rehype-sanitize если unified) -> строгий CSP.
8. Лес: Canvas 2D (камера и hit-test самописные) как базовый вариант, PixiJS v8 как апгрейд при нужде в эффектах; на Linux по умолчанию путь без WebGL или авто-бенч FPS при старте. Спрайты: Codex `$imagegen` (gpt-image-2) + постобработка фона.
9. Синк: собственный op-log с HLC (`uhlc`), LWW на сообщение, таблица ревизий, tombstones, blob'ы по sha256; сервер на Rust как append-only лог с курсором. CRDT/cr-sqlite/Electric/PowerSync/Turso не брать на старте.
10. Дизайн: токены в DTCG JSON в репозитории -> Style Dictionary 5.x -> CSS variables сейчас, Swift/Kotlin позже; Claude Design как источник идей/прототипов с handoff через Claude Code, не как источник правды.
11. Updater: tauri-plugin-updater со статическим JSON/своим endpoint на домашнем сервере; подписывать, хранить приватный ключ вне `.env`.

## Топ-риски

1. **Linux WebKitGTK**: DMABUF/NVIDIA падения, "тихий" медленный WebGL, различия шрифтов и дробный масштаб. Митигация: Canvas 2D/fallback, тестовая матрица, при необходимости отложенный переход на CEF-runtime Tauri v3 (сейчас alpha).
2. **IME/live-preview в CM6** (Safari/WKWebView): подтверждены баги композиции рядом с widget decorations; кириллица обычно проще CJK, но проверять нужно руками.
3. **Tauri mobile**: сама команда признавала незрелый DX и неполный набор плагинов; Share Extension/виджеты - ручной нативный код в регенерируемом `gen/apple`. Не обещать мобайл рано.
4. **Embeddings-пайплайн**: смена модели = пересчёт всех векторов; зависимость от OpenRouter (приватность - ZDR; цены/каталог меняются, например, `qwen3-embedding-0.6b` из доков отсутствует в живом списке); объём в памяти для brute-force (~0,4-1,2 ГБ при 1024-3072 dim на 100k).
5. **Sync-дизайн**: самописный op-log это ответственность за корректность (HLC-дрейф, tombstones, компакция лога, вложения). Нужны property-тесты и симуляция нескольких устройств.
6. **Pre-v1/малосопровождаемые зависимости**: sqlite-vec (pre-v1, стабильный релиз 0.1.9 от марта, дальше alpha), pixi-viewport (активность не подтверждена), fts5-snowball (маленький проект), lang-markdown (репо архивировано/переехало), Tauri v3 как будущая миграция.
7. **XSS/IPC-поверхность**: пользовательский Markdown + Tauri IPC; все команды по умолчанию доступны всем окнам - сужать `AppManifest::commands`, строгий CSP, санитайзер, не открывать внешние ссылки в том же вебвью.
8. **Claude Design**: нет экспорта дизайн-системы как файла/токенов и нет Figma-выхода (по help center), продукт в бете с датой закрытия standalone 14.12.2026; не строить на нём критический путь.
9. **Генерация спрайтов**: несогласованность стиля между генерациями (офиц. оговорка), неясная поддержка прозрачного фона во встроенном `$imagegen`, расход лимитов Codex в 3-5 раз быстрее.

## Основные источники
- https://github.com/tauri-apps/tauri/releases , https://v2.tauri.app/reference/webview-versions/ , https://v2.tauri.app/develop/debug/linux-graphics/ , https://v3.tauri.app/develop/cef/ , https://v2.tauri.app/blog/tauri-20/ , https://v2.tauri.app/security/capabilities/ , https://v2.tauri.app/security/csp/
- https://v2.tauri.app/plugin/{global-shortcut,clipboard,single-instance,updater,sql}/ , https://v2.tauri.app/learn/system-tray/
- https://docs.obsidian.md/Plugins/Editor/Editor+extensions , https://github.com/codemirror/lang-markdown , https://codemirror.net/docs/changelog
- https://github.com/cure53/DOMPurify , https://github.com/rehypejs/rehype-sanitize
- https://tanstack.com/virtual/latest/docs/chat , https://github.com/inokawa/virtua , https://virtuoso.dev/pricing
- https://github.com/pixijs/pixijs/releases , https://github.com/pixijs-userland/pixi-viewport
- https://github.com/rusqlite/rusqlite , https://www.sqlite.org/fts5.html , https://github.com/asg017/sqlite-vec , https://alexgarcia.xyz/sqlite-vec/rust.html
- https://openrouter.ai/docs/api-reference/embeddings , https://openrouter.ai/api/v1/embeddings/models , https://openrouter.ai/docs/guides/features/zdr
- https://github.com/automerge/automerge/releases , https://github.com/loro-dev/loro/releases , https://github.com/vlcn-io/cr-sqlite , https://github.com/electric-sql/electric , https://docs.turso.tech/sync/usage , https://docs.rs/crate/uhlc/latest
- https://www.anthropic.com/news/claude-design-anthropic-labs , https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design , https://support.claude.com/en/articles/14604416-get-started-with-claude-design
- https://www.designtokens.org/tr/drafts/format/ , https://styledictionary.com/info/dtcg/ , https://github.com/style-dictionary/style-dictionary/releases
- https://learn.chatgpt.com/docs/image-generation , https://raw.githubusercontent.com/openai/skills/main/skills/.system/imagegen/SKILL.md , https://developers.openai.com/api/docs/guides/image-generation
