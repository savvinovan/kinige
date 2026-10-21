# Синхронизация: писать своё или взять готовое

Дата: 2026-10-21. Вопрос: делать собственный протокол из ADR-0001 (HLC, группы полей LWW, ревизии конфликтов, tombstones, карантин, конверты XChaCha20-Poly1305 на libsodium) или взять готовый движок.
Нормативны §4, §10, §12.3 и §15 ТЗ (`docs/plan/02-prd.md`). ADR-0001 читался в ветке `adr/0001-protocol-keys`, ревизия 3.
Предыдущее сравнение — `research/tauri-stack.md` §7 и `01-review.md`: тогда решили «свой op-log, готовые движки не брать». Этот документ проверяет то решение заново, с E2EE в центре и на свежих данных.

**Как проверялось.** Даты, версии и лицензии взяты из GitHub (API, `releases.atom`, LICENSE), crates.io, npm и официальной документации на 2026-10-21. Для каждого утверждения дана ссылка. Ключевые факты я перепроверил сам: статус cr-sqlite (issue #444), правило схемы у SQLite Sync, `%ELO` в README Loro Protocol. Что не удалось проверить, помечено «не проверено».

## Коротко

- **Готового движка, который закрывает §10 ТЗ, нет.** Каждый кандидат попадает в одну из трёх групп:
  - сервер читает данные: PowerSync, ElectricSQL, Turso, Ditto, Zero, Jazz 2;
  - E2EE есть, но только в JS или Go и со своей моделью данных: Evolu, Jazz Classic, any-sync;
  - E2EE помечено как экспериментальное или pre-alpha: Loro `%ELO`, Keyhive/Subduction для Automerge.
- **Готовые решения закрывают самую дешёвую часть ADR-0001, а самую опасную оставляют нам.** Самое дешёвое — слияние по HLC. Самое опасное — ключи, слова восстановления, сервер, purge и горизонт tombstones, поколения после бэкапа, совместимость версий. Даже лучшие кандидаты прямо пишут, что ключи «на приложении»: Loro — «Keys and key agreement are application‑provided and out of scope», secsync — key exchange вне рамок.
- **Рекомендация: своё, как в ADR-0001, без чужого движка.** Свериться стоит с готовыми открытыми реализациями той же схемы (Actual Budget, Notesnook, Loro Protocol, secsync) — подробности в разделе «Рекомендация». Подключить Loro внутрь конверта `body` можно позже, если понадобится автослияние текста.

## Что требуется от решения (из ТЗ)

| # | Требование | Где в ТЗ |
|---|---|---|
| R1 | Сервер не видит текст, названия, `journal_date`, теги, вложения. Время изменений и размеры видеть может, это принято | §10.1, §10.3 |
| R2 | Неизменяемые конверты, push идемпотентен по `op_id`, AEAD покрывает HLC, флаг purge и тип | §10.2, §10.3 |
| R3 | Rust владеет SQLite (один писатель), FTS5 остаётся, схема — наша | §3.2, §4.2 |
| R4 | Корзина 30 дней, восстановление побеждает старое удаление, `purged`, tombstones 180 дней, карантин без воскрешения | §10.2 |
| R5 | Старый и новый клиент работают вместе, неизвестные поля сохраняются | §10.4 |
| R6 | Свой сервер: `docker compose`, бэкап и восстановление с «поколением» и полной пересинхронизацией | §10.1 |
| R7 | Пароль отдельно от KEK, 24 слова, смена пароля без перешифровки | §10.3 |
| R8 | Первая привязка после импорта Telegram — порядка 100k записей | §2.1, ADR §6.2 |
| R9 | Будущий веб-клиент (WASM, libsodium.js) и мобильный | §2.3, §2.4, §3.2 |

## Кандидаты

### 1. Движки поверх SQLite

**cr-sqlite (vlcn-io)** — CRDT-расширение SQLite.
- Последний релиз — v0.16.3 от 2024-01-17. Коммиты 2026-08 — только сборочные правки сторонних людей. MIT. [releases](https://api.github.com/repos/vlcn-io/cr-sqlite/releases?per_page=3)
- Автор 2024-12-30: «I'm full time on https://zero.rocicorp.dev/ these days» и ещё «at least 1-2 years» без времени на проект. [issue #444](https://github.com/vlcn-io/cr-sqlite/issues/444)
- Как устроено:
  - LWW по колонке; чтение и запись изменений идут через `crsql_changes`;
  - нельзя проверяемые FK, а также UNIQUE кроме PK [constraints](https://vlcn.io/docs/cr-sqlite/constraints);
  - вставка в CRR в 2,5 раза медленнее [README](https://github.com/vlcn-io/cr-sqlite).
- Сети нет. Изменения можно зашифровать пакетом и передать через свой сервер — это вывод из API, примера нет.
- **Вывод:** проект фактически заброшен — не брать.

**SQLite Sync (sqliteai/sqlite-sync)** — живое CRDT-расширение на C.
- v1.2.0 от 2026-09-28, коммиты 2026-10-20, Apache-2.0. [releases](https://api.github.com/repos/sqliteai/sqlite-sync/releases?per_page=3), [LICENSE](https://raw.githubusercontent.com/sqliteai/sqlite-sync/main/LICENSE)
- Сборки есть под macOS, Windows, Linux, iOS, Android и WASM; грузится как loadable extension. [README](https://github.com/sqliteai/sqlite-sync)
- Модель конфликтов:
  - для таблиц — Causal-Length Set (по умолчанию), Delete-Wins, Add-Wins;
  - для текста — «Block-Level LWW»: строки одной заметки сливаются независимо. [README](https://github.com/sqliteai/sqlite-sync)
- Есть `cloudsync_payload_encode` и `cloudsync_payload_apply`: изменения можно выгрузить как BLOB и применить. [API.md](https://github.com/sqliteai/sqlite-sync/blob/main/API.md) Зашифровать BLOB и отправить через свой сервер — мой вывод, примера в документации нет.
- E2EE в документации не упоминается.
- Серверная часть — сервис CloudSync. Даже со «своим» Postgres его нужно зарегистрировать в панели `dashboard.sqlitecloud.io`. [postgres quickstart](https://github.com/sqliteai/sqlite-sync/blob/main/docs/postgresql/quickstarts/postgres.md) Самостоятельный сервис синхронизации не найден (не проверено, что его нет).
- **Блокер для R5:** «sqlite-sync computes a schema hash… The server rejects payloads whose schema hash it does not recognize». Все участники «must have the same set of synced tables with identical structure». [schema.md](https://github.com/sqliteai/sqlite-sync/blob/main/docs/schema.md) Значит, пока одно устройство обновилось, а другое нет, они не синхронизируются.
- Сборка мусора tombstones не проверена.

**Evolu (evoluhq/evolu)** — по духу ближе всех к нам.
- `@evolu/common` 8.23.2 от 2026-10-17, MIT, очень активен. [releases](https://api.github.com/repos/evoluhq/evolu/releases?per_page=3)
- E2EE по умолчанию, XChaCha20-Poly1305. Relay видит OwnerId, **время изменений в открытом виде**, NodeId, примерный размер и IP. [privacy](https://www.evolu.dev/docs/privacy) По R1 это примерно то же, что видит наш сервер по ADR-0001 §10: HLC тоже открыты.
- Блокеры:
  - **Rust нет**, только TypeScript-пакеты [packages](https://github.com/evoluhq/evolu/tree/main/packages); база жила бы в webview, а не в Rust (R3);
  - схема своя, на TypeScript, с системными колонками [schema](https://www.evolu.dev/docs/schema); FTS5 и своя SQL-схема не описаны;
  - эволюция схемы только добавлением; версии протокола никто не согласует («nothing negotiates a version») [Protocol](https://www.evolu.dev/docs/api-reference/common/local-first/Protocol);
  - relay на Node.js, а не на axum.
- **Вывод:** хороший образец протокола, но как зависимость не подходит.

**PowerSync.**
- Rust SDK 0.1.1 — «Experimental». `tauri-plugin-powersync` в статусе alpha («Expect breaking changes»). [crates.io](https://crates.io/crates/powersync), [tauri](https://docs.powersync.com/client-sdks/reference/tauri.md)
- Сервис под FSL-1.1-ALv2. [LICENSE](https://raw.githubusercontent.com/powersync-ja/powersync-service/main/LICENSE)
- Сервер — источник истины и выполняет SQL по строкам Postgres. [consistency](https://docs.powersync.com/architecture/consistency.md)
- E2EE возможно только как «непрозрачная колонка с шифртекстом», и тогда id и поля фильтров открыты. [blog](https://www.powersync.com/blog/building-an-e2ee-chat-app-with-powersync-supabase)
- **Вывод:** не подходит — лишний сервис, и сервер видит структуру данных.

**ElectricSQL.**
- «read-path sync engine for Postgres», записи синхронизирует не он. [writes](https://electric.ax/docs/guides/writes)
- Сервер на Elixir, клиента на SQLite нет. Компания уходит в «agent platform». [electric.ax](https://electric.ax/)
- **Вывод:** не подходит.

**Turso / libSQL.**
- Turso Sync в бете. Конфликты — «last push wins» на уровне строки; сервер отдаёт физические страницы базы, то есть читает её. [conflict-resolution](https://docs.turso.tech/sync/conflict-resolution.md)
- Собственный sync-сервер описан для разработки и тестов. [local-sync-server](https://docs.turso.tech/sync/local-sync-server.md)
- Движок Turso не поддерживает FTS5 и loadable-расширения. [COMPAT.md](https://github.com/tursodatabase/turso/blob/main/COMPAT.md)
- **Вывод:** несовместим с E2EE в принципе.

### 2. CRDT-библиотеки

**Automerge + Keyhive/Subduction.**
- Ядро на Rust живое: `automerge` 0.12.0 от 2026-09-16, MIT. [crates.io](https://crates.io/crates/automerge)
- E2EE-слой:
  - Keyhive: «pre-alpha», «DO NOT use this release in production applications», аудита нет [keyhive](https://github.com/inkandswitch/keyhive);
  - Subduction, преемник Beelay, тоже: «DO NOT use for production use cases at this time» [subduction](https://github.com/inkandswitch/subduction).
- Документ хранит всю историю, обрезку истории «not prioritised»; миграции схемы — вручную. [modeling-data](https://automerge.org/docs/cookbook/modeling-data/)
- **Вывод:** E2EE не готово, следить за проектом.

**Yjs / Yrs + secsync.**
- `yrs` 0.28.0 (pre-1.0), MIT, нативный Rust. [y-crdt](https://github.com/y-crdt/y-crdt)
- Tombstones удалённых структур не собираются. [yjs README](https://github.com/yjs/yjs#yjs-crdt-algorithm)
- Готовые варианты транспорта:
  - secsync (E2EE-транспорт) — «beta», TypeScript, последний коммит 2024-09-21: проект спит. Сервер хранит зашифрованные снимки и обновления, сливают клиенты. [secsync](https://github.com/nikgraf/secsync)
  - y-sweet читает документы на сервере, поэтому с E2EE несовместим. [y-sweet](https://github.com/jamsocket/y-sweet)
- **Вывод:** годится только как библиотека слияния текста; транспорт и ключи — наши.

**Loro + Loro Protocol.**
- `loro` 1.16.2, MIT, Rust + WASM + Swift. Формат стабилен с 1.0. [loro](https://github.com/loro-dev/loro), [v1.0](https://loro.dev/blog/v1.0)
- Shallow snapshots обрезают историю. Цена: пиры синхронизируются, только если у них есть версии после точки обрезки. [shallow snapshots](https://loro.dev/docs/concepts/shallow_snapshots)
- Loro Protocol, режим `%ELO`:
  - E2EE на AES-GCM; «The server never decrypts; it indexes plaintext headers only to support backfill and routing»;
  - «Keys and key agreement are application‑provided and out of scope»;
  - есть минимальный Rust-сервер `loro-websocket-server`. [protocol](https://github.com/loro-dev/protocol)
- Блог называет `%ELO` экспериментальным с меняющимся форматом. Это со слов исследовательского агента; сам я страницу не открыл (403), в README такой пометки нет. [blog](https://loro.dev/blog/loro-protocol)
- **Вывод:** лучший кандидат, если понадобится слияние текста внутри сообщения. Транспорт и ключи всё равно наши.

### 3. Готовые E2EE-системы

**Anytype any-sync.**
- v0.13.8 от 2026-10-07, MIT, только Go. [any-sync](https://github.com/anyproto/any-sync)
- Изменения — подписанный зашифрованный граф (DAG). Своё состояние приложение строит само: «Applications using any-sync can utilize the objects received from it to construct their own internal state». [overview](https://raw.githubusercontent.com/anyproto/tech-docs/main/any-sync/overview.md)
- Self-host:
  - состав: coordinator, 3 sync-узла, file-узел, consensus-узел, MongoDB, Redis и Garage, около 1 ГБ RAM;
  - README: «designed for personal use, review, or testing purposes» [dockercompose](https://github.com/anyproto/any-sync-dockercompose).
- Эталонный middleware распространяется под source-available лицензией, а не MIT. [LICENSE](https://raw.githubusercontent.com/anyproto/anytype-heart/main/LICENSE.md)
- **Вывод:** тяжело, Go-sidecar, модель состояния всё равно своя.

**Jazz.**
- Jazz 2.0 — alpha. Сервер проверяет права построчно и фильтрует чтения, то есть видит данные; слово «encrypt» в документации не встречается. Rust-клиент хранит данные в своём RocksDB, не в нашей SQLite. [permissions](https://jazz.tools/docs/auth/permissions), [jazz](https://github.com/garden-co/jazz)
- Jazz Classic с E2EE — JS-стек, к тому же больше не основная линия. [classic](https://classic.jazz.tools/llms-full.txt)
- **Вывод:** не подходит.

**Ditto.**
- Rust SDK проприетарный, только Linux и macOS ARM64. [crates.io](https://crates.io/crates/dittolive-ditto), [compatibility](https://docs.ditto.live/sdk/latest/compatibility/rust.md)
- Облако — «trusted peer». [cloud](https://docs.ditto.live/cloud/overview.md)
- Self-managed — только Enterprise.
- **Вывод:** не подходит.

**Replicache / Zero.**
- Replicache в режиме поддержки, репозиторий в архиве. [replicache](https://github.com/rocicorp/replicache)
- Zero: «Zero is not local-first… authoritative server», «doesn't support offline writes». [when-to-use](https://zero.rocicorp.dev/docs/when-to-use.md)
- **Вывод:** не подходит.

**Triplit.**
- Последний релиз — 2025-07-31; команда ушла в Supabase. [supabase](https://supabase.com/blog/triplit-joins-supabase)
- Права проверяет сервер, поэтому он видит данные.
- **Вывод:** не подходит.

### 4. Без своего сервера синхронизации

**Syncthing.**
- v2.1.6, MPL-2.0. [syncthing](https://github.com/syncthing/syncthing)
- Зашифрованные папки для недоверенных устройств: «should still be considered beta / testing only». [untrusted](https://raw.githubusercontent.com/syncthing/docs/main/users/untrusted.rst)
- Конфликты решаются на уровне файла: появляется копия `.sync-conflict-…`. [syncing](https://raw.githubusercontent.com/syncthing/docs/main/users/syncing.rst)
- Открытую SQLite синхронизировать нельзя — по словам мейнтейнеров на [форуме](https://forum.syncthing.net/t/bahaviour-of-st-on-sqlite-files-in-various-scenarios/5921) и по [sqlite.org/howtocorrupt](https://www.sqlite.org/howtocorrupt.html).

**iCloud Drive.**
- Apple: «a SQLite database's store file must never be stored in iCloud». [iCloud Design Guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/iCloudDesignGuide/Chapters/iCloudFundametals.html)
- «Оптимизация хранилища» может убрать файл с диска до повторной загрузки. [Apple support](https://support.apple.com/guide/mac-help/optimize-storage-space-sysp4ee93ca4/mac)
- Получит ли ad-hoc-подписанный Tauri entitlements для iCloud — не проверено.

**S3 / WebDAV.**
- Готового Rust-крейта «зашифрованный append-only журнал поверх объектного хранилища» не нашлось.
- Есть строительный блок: Apache OpenDAL — единое API для S3, WebDAV и других. [opendal](https://github.com/apache/opendal)

**Вывод по разделу.** Это только транспорт для неизменяемых зашифрованных файлов журнала, как в `advice-fable.md` п. 1. Слияние, карантин, ключи и purge остаются теми же, что в ADR-0001. Экономится часть S4c, теряются Kinige Cloud (§2.6) и простой путь для веб-клиента: браузер не читает папку Syncthing.

### 5. Готовые приложения той же схемы — образцы, не зависимости

**Actual Budget**
- v26.10.0, MIT, живой. [actual](https://github.com/actualbudget/actual)
- Схема:
  - HLC и сообщение на одну ячейку (LWW по колонке);
  - конверт `{timestamp,isEncrypted,content}` [sync.proto](https://raw.githubusercontent.com/actualbudget/actual/master/packages/crdt/src/proto/sync.proto);
  - сервер хранит `messages_binary(timestamp, is_encrypted, content)` и находит расхождение реплик по Merkle-дереву [sync-simple.js](https://raw.githubusercontent.com/actualbudget/actual/master/packages/sync-server/src/sync-simple.js).
- Сборка мусора — «reset sync»: история сжимается, остальные устройства откатываются. [sync docs](https://raw.githubusercontent.com/actualbudget/docs/master/docs/getting-started/sync.md)
- В десктопной сборке ключ выводится через PBKDF2 с 10 000 итераций — слабо. [encryption-internals](https://raw.githubusercontent.com/actualbudget/actual/master/packages/loot-core/src/server/encryption/encryption-internals.electron.ts)
- Это та же архитектура, что ADR-0001, только проще и слабее по ключам. Наш план не экзотика: такая схема годами работает в живом проекте.

**Notesnook**
- Каждый элемент шифруется отдельно, XChaCha20-Poly1305 из libsodium, на всех платформах. Сервер хранит `{alg, cipher, iv, salt, id}` «without further processing». [help](https://notesnook.com/help/how-is-my-data-encrypted)
- Правки одной заметки с разницей от минуты — ручной конфликт. [merge conflicts](https://notesnook.com/help/faqs/what-are-merge-conflicts)
- Сервер — beta, AGPL-3.0, .NET. [sync-server](https://github.com/streetwriters/notesnook-sync-server)
- Подтверждает наш выбор примитива и модель «сервер — тупое хранилище конвертов».

**Standard Notes**
- Протокол 004: Argon2id + XChaCha20-Poly1305; в associated data лежит UUID элемента. [encryption](https://standardnotes.com/help/security/encryption)
- С 2024 года проект принадлежит Proton.

## Сводная таблица

| Кандидат | Жив (2026-10) | E2EE из коробки | Self-host | Rust владеет SQLite + FTS5 | Совместимость версий (R5) | Итог |
|---|---|---|---|---|---|---|
| cr-sqlite | нет (автор ушёл) | нет, можно поверх | нет сервера | да (расширение) | не описана | нет |
| SQLite Sync | да, 1.2.0 | нет, можно поверх (не проверено) | нет: сервис SQLite Cloud | да (расширение) | **нет**: одинаковая схема у всех | нет |
| Evolu | да, 8.x | да | да, Node | **нет**: только TS | только добавление | образец |
| PowerSync | да | нет | да, но TS + Postgres-источник | Rust SDK experimental, Tauri alpha | не проверено | нет |
| ElectricSQL | смена курса | нет | Elixir | нет | — | нет |
| Turso Sync | бета | невозможно | для тестов | нет FTS5 | — | нет |
| Automerge + Keyhive | да / pre-alpha | pre-alpha | Subduction, «не для прода» | ядро на Rust | вручную | следить |
| Yrs + secsync | да / спит | secsync — TS, beta | нет | Yrs на Rust | — | только слияние |
| Loro + `%ELO` | да | экспериментально | минимальный Rust-сервер | да, Rust | формат стабилен с 1.0 | только слияние, образец |
| any-sync | да | да | да, тяжёлый | нет, Go | не проверено | нет |
| Jazz 2 / Classic | alpha / legacy | нет / да в JS | да | нет | — | нет |
| Ditto, Zero, Triplit | — | нет | нет / Enterprise | — | — | нет |
| Syncthing / iCloud | да | Syncthing — beta | без сервера | только транспорт файлов | — | только транспорт |
| **ADR-0001 (своё)** | — | да, по ТЗ | axum + Postgres | да | да, `sv` по группам | — |

## Сравнение с собственным протоколом

**Риск потери данных.**
- **Своё:** риск — ошибки в нашем коде. Проверка: 39 property-тестов S4a и 12 тестов S4b (приложение ADR §B), ревью `sync-crypto-guardian`, внешний аудит перед публичным релизом (§10.3).
- **Чужой движок:** к нашим ошибкам на стыке добавляются ещё три риска:
  - ошибки самого движка;
  - несовпадение его модели с ТЗ: у SQLite Sync устройства разных версий не синхронизируются, у Loro узлы без общей точки после обрезки истории не синхронизируются;
  - смерть проекта: так случилось с cr-sqlite, Triplit, Replicache.
- **Страховка вне синхронизации есть в любом варианте:** Markdown-копия, 7 копий базы, проверка целостности (§2.1, §12.3).

**Сроки.**
- В §14 на синхронизацию заложено 14 дней спайков: S4a — 5, S4b — 4, S4c — 5. Сколько дней у слайса 11, в §15 не указано.
- Codex уже отмечал, что эти дни — проверка гипотезы, а не оценка готовности (`review-codex.md` п. 19).
- Движок может заменить только S4a (слияние, HLC). Это оценка, не замер. Но интеграция и прототип «зашифрованный payload через свой сервер» тоже займут дни. S4b, S4c и слайс 11 не сокращаются ни в одном варианте.

**Что придётся писать самим в любом варианте.**
- Конверт и AEAD над открытыми полями.
- Иерархия ключей: Argon2id, обёртки DEK, 24 слова, смена пароля, восстановление.
- Сервер: аккаунты, `seq` под блокировкой, blob'ы, квоты, бэкап с поколениями.
- Шифрование вложений.
- Purge и горизонт tombstones, карантин.
- Первая привязка после импорта на 100k записей.
- Матрица версий.

Ни один кандидат из списка выше этого не даёт.

**Привязка к вендору.**
- **Своё:** привязки нет. Формат описан в ADR и публичен для self-hosted (§10.4).
- **SQLite Sync:** серверная часть завязана на SQLite Cloud.
- **PowerSync:** FSL-лицензия.
- **Evolu, Loro, Yrs:** MIT, но формат обновлений и протокол принадлежат им.

**Соответствие ТЗ.**
- Требования R1–R9 закрывает только ADR-0001.
- По R1 у ADR-0001 и у Evolu одинаковое честное ограничение: сервер видит, когда и сколько пишут. ТЗ это принимает (§10.3).

## Варианты

**A. Своё, как в ADR-0001 (рекомендую).**
- Плюсы:
  - точное соответствие ТЗ, включая смешанные версии, карантин и поколения;
  - Rust-ядро в `kinige-domain` собирается под wasm32 для веба, без чужого рантайма;
  - формат и сервер наши, привязки нет;
  - примитивы те же, что у Notesnook, архитектура та же, что у Actual Budget.
- Минусы:
  - вся ответственность за корректность наша;
  - ADR уже большой — 400 строк плюс приложение на 233 строки;
  - сроки спайков — гипотеза.
- Снижение риска:
  - S4a как жёсткие ворота: симуляция с отказами по тест-плану ADR, до любого кода сервера;
  - сверить решения с Actual Budget (как устроен «reset sync», где Merkle-дерево против `seq`) и Loro Protocol / secsync (снимки плюс журнал на тупом сервере);
  - внешний аудит перед публичным релизом.

**B. Готовое CRDT-расширение SQLite + своё шифрование и сервер.**
- Реалистично только SQLite Sync (cr-sqlite заброшен).
- Плюсы:
  - готовое слияние по колонкам и построчное слияние текста;
  - сборки под мобильные платформы и WASM.
- Минусы:
  - нарушает R5: разная схема — отказ;
  - серверная часть завязана на SQLite Cloud;
  - сборка мусора tombstones не проверена;
  - purge, карантин и ключи всё равно наши;
  - серверные проверки по заголовкам из ADR (упорядочивание purge, `stale_purge`) непрозрачному payload недоступны.
- Экономит в лучшем случае S4a, а добавляет зависимость от C-расширения на всех ОС.

**C. Гибрид: своё (A) + Loro (или Yrs) только для текста `body`.**
- Тело сообщения хранится как документ Loro внутри зашифрованного конверта. Остальные группы — как в ADR.
- Плюсы: одновременные правки одного сообщения сливаются автоматически, без ревизии `conflict`.
- Минусы:
  - ТЗ прямо говорит «CRDT для текста не нужен: сообщения короткие и редко правятся» (§10.2);
  - автослияние прячет конфликт, а ADR показывает обе версии — это ближе к принципу 9;
  - растёт размер записи, история документа требует обрезки, появляется вторая модель данных.
- Имеет смысл только если пользователь решит, что правки с двух машин частые. Добавить можно позже, новой группой или повышением `sv` (ADR §4).

Отдельно не рекомендую вариант «без сервера» (Syncthing или iCloud). Протокол тот же, Apple запрещает класть SQLite в iCloud, зашифрованные папки Syncthing — бета. Плюс он ломает Kinige Cloud и веб-клиент из ТЗ.

## Рекомендация

**Вариант A.** Ни один готовый движок не берёт на себя то, что действительно опасно: ключи, восстановление, purge и tombstones, поколения, смешанные версии. Те, что берут шифрование, либо не на Rust, либо в стадии alpha или эксперимента. Поэтому «взять готовое» добавляет зависимость и не снимает риск.

Что конкретно взять у других:
- примитивы libsodium (уже в ADR);
- образец простого сервера и HLC-журнала — Actual Budget;
- образец «тупой сервер хранит зашифрованные снимки и обновления» — secsync и Loro `%ELO`;
- образец конверта на элемент — Notesnook.

Если S4a выйдет за двойной срок (§12.1a), запасной путь — упростить протокол, а не покупать движок. Например, LWW на запись целиком вместо групп полей. Это решает пользователь, см. вопрос 2.

## Вопросы пользователю

1. **Одновременная правка одного сообщения на двух машинах:** показывать обе версии с отметкой «есть другая версия» (как в ADR) или сливать автоматически построчно (вариант C)? Это единственное, что реально даёт готовая библиотека.
2. **Если S4a не уложится в двойной срок (10 дней):** упрощать протокол (меньше гарантий: например, одновременная правка флагов и переноса — по LWW целиком) или останавливаться и пересматривать сроки MVP+?
3. **Приватность метаданных:** сервер видит, когда и сколько вы пишете (время изменений, размеры с точностью до 1 КБ) — так у ADR-0001, Evolu и Actual Budget. ТЗ это принимает. Подтвердите, что для журнала это приемлемо: скрыть ритм — отдельный сложный протокол.

## Не проверено

- Можно ли гнать зашифрованный payload SQLite Sync через свой сервер на практике. Есть ли у них самостоятельный sync-сервис. Как у них собираются tombstones.
- Пометка «experimental» у `%ELO` в блоге Loro — страница отдала 403, данные со слов агента. Поддерживает ли Rust-сервер Loro режим `%ELO` напрямую.
- Память и накладные расходы CRDT-документов на 100k сообщений — ни у одного кандидата нет замера.
- Пользователи в проде у Automerge, Loro, Evolu, SQLite Sync — первоисточники их не называют.
- Работают ли OPFS и Shared Workers в WKWebView под схемой Tauri (важно для Evolu).
- Entitlements iCloud для ad-hoc-подписанного приложения.
- Число дней на слайс 11 — в §15 ТЗ не указано.
