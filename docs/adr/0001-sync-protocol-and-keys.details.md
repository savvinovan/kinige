# ADR-0001, приложение: форматы, проверки, схема, тест-план

Приложение к [`0001-sync-protocol-and-keys.md`](0001-sync-protocol-and-keys.md), статус тот же — «предложено». Нормативна связка «ТЗ → ADR → это приложение». Ссылки вида «§3.5» указывают на разделы ADR, если не сказано «ТЗ».

## A. Форматы и протокольные таблицы

### A.0. Группы полей

Группа `core` пишется при создании. Производное (`body_search`, `day_activity`, `updated_at` чата, теги) не синхронизируется (ТЗ §4.1). `attachment_local` не синхронизируется (ТЗ §4.2).

| Сущность | Группа | Поля |
|---|---|---|
| message | `core` | `created_at`, `created_tz`, `reply_to_id` |
| | `body` | `body_md`, `edited_at`, `base` (§3.4) |
| | `flags` | `is_task`, `task_done_at`, `favorite_at`, `pinned_at` |
| | `placement` | `chat_id`, `journal_date`, `moved_from_chat` (пометка «чат удалён», §3.6) |
| | `deleted` | `deleted_at` |
| chat | `core` | `created_at`, `is_inbox` (у Inbox `core` — константа) |
| | `meta` | `title`, `icon`, `color` |
| | `order` | `sort_key` (при равных ключах порядок решает `id`) |
| | `state` | `pinned_at`, `archived_at` |
| | `privacy` | `ai_excluded` — отдельно, чтобы не потеряться при одновременном переименовании |
| | `deleted` | `deleted_at` |
| attachment | `core` | `message_id`, `kind`, `sha256`, `filename`, `mime`, `byte_size`, `width`, `height`, `duration_ms`, `url`, `link_title`, `link_site`, `remote_blob_id`, `blob_key` |
| | `order` | `sort_index` |
| | `deleted` | `deleted_at` |
| message_revision | `core` | `message_id`, `snapshot_json` (для `conflict` — только `body` версии P, §3.4), `saved_at`, `reason` |

### A.1. Конверт и обёртки DEK — байты

Все целые — big-endian. Строки-метки — ASCII без завершающего нуля. `account_id` и прочие UUID в AD — 16 сырых байт.

**Конверт** (`format_version = 1`). В таком же виде он лежит в `change_outbox.envelope` и хранится на сервере.

```text
off    len     поле
0      1       format_version = 0x01
1      4       key_id (u32), 1 = первый DEK
5      4       header_len H (u32), 1 ≤ H ≤ 4096
9      H       header: UTF-8 JSON (§5.2)
9+H    24      nonce = randombytes_buf(24)
33+H   N+16    crypto_aead_xchacha20poly1305_ietf_encrypt(
                 m  = sodium_pad(payload, 1024), N кратно 1024, 1024 ≤ N ≤ 1 МиБ + 1024,
                 ad = AD_env, npub = nonce, k = K_env[key_id])
AD_env = "kinige.env.v1" (13 байт) ‖ account_id (16) ‖ bytes[0 .. 9+H)
```

**Обёртка DEK** (`wrap_pw`, `wrap_rc`):

```text
off  len  поле
0    1    wrap_version = 0x01
1    24   nonce = randombytes_buf(24)
25   48   crypto_aead_xchacha20poly1305_ietf_encrypt(m = DEK (32), ad = AD_wrap, npub = nonce, k = KEK_pw | KEK_rc)
итого 73 байта
AD_wrap = "kinige.wrap.v1" (14) ‖ wrap_version (1) ‖ account_id (16) ‖ key_id (u32, 4) ‖ kind (u8: 1 = пароль, 2 = восстановление)
          ‖ при kind = 1: salt (16) ‖ kdf_profile (u8)
```

Профиль и соль входят в AD: если сервер подменит их без смены пароля, обёртка не расшифруется.

**Ограничения на разбор** — проверяются **до** выделения памяти по длинам:
- `H ≤ 4096`;
- длина шифротекста кратна 1024 плюс 16 и не больше `1 МиБ + 1024 + 16`;
- общая длина совпадает с длиной, объявленной транспортом.

### A.2. Blob вложения

```text
P = sodium_pad(file, 1024)            # всегда ≥ 1 байта добавки → |P| ≥ 1024, пустой файл = 1024 байта
chunks = P, нарезанный по 1 048 576 байт; последний чанк короче или равен, пустым не бывает
blob = header (24, init_push(blob_key))
     ‖ для каждого чанка: push(chunk, ad = пусто, tag = MESSAGE | FINAL для последнего) → |chunk| + 17
```

Чтение:
- `pull` по чанкам того же размера плюс 17;
- конец файла до `TAG_FINAL` — ошибка; байты после `TAG_FINAL` — ошибка;
- затем `sodium_unpad`, сверка `sha256` и `byte_size` с группой `core`.

Повторная публикация того же `remote_blob_id` — ответ «уже есть», это успех. Незавершённую загрузку можно перезаписать, завершённый blob неизменяем.

### A.3. Ответы на push по каждому `op_id`

| Ответ | Когда | Клиент |
|---|---|---|
| `ok(seq)` | принят | `acked_seq`, `server_known = 1` |
| `duplicate(seq)` | тот же `op_id`, те же байты | как `ok` |
| `op_conflict` | тот же `op_id`, другие байты | ошибка клиента, синхронизация остановлена, отчёт пользователю |
| `purged` | сущность уже удалена | запись с детьми — в карантин, tombstone (§3.5) |
| `stale_purge` | есть конверт с HLC `deleted` больше `purged_of` | pull, переоценка |
| `clock_skew` | `wall_ms` в любом HLC заголовка больше `server_now + SKEW_MAX` | выход из `clock_skew` (§2) |
| `too_old` | версия протокола ниже минимальной | «обновите клиент» |

### A.4. Порядок проверок

**Сервер, push:**
1. Длины (§A.1) и разбор заголовка; ошибка → 400.
2. Сессия и аккаунт.
3. Дедупликация `op_id` (`duplicate` или `op_conflict`) — **раньше** всех правил purge, чтобы повтор принятого конверта не получил `purged`.
4. `too_old`.
5. `clock_skew`.
6. Сущность уже удалена (`purged`) — для любого `kind`, кроме повторного `purged`, который даёт no-op `ok`.
7. Для `purged`: проверка `stale_purge`.
8. Присвоение `seq` под блокировкой аккаунта, запись.

**Клиент, pull:**
1. Длины.
2. `format_version` и `key_id`; неизвестные → `pending_unknown`, курсор двигается.
3. AEAD; ошибка → страница не применяется, курсор стоит, ошибка видна пользователю.
4. Сверка `op` и `id` заголовка с объявленными транспортом.
5. Строгий разбор: HLC, `sodium_unpad`, JSON без повторяющихся ключей, набор групп нагрузки равен `header.groups`. Ошибка → карантин с причиной `malformed`, курсор двигается, ошибка видна.
6. Часы получают все HLC заголовка (§2).
7. Tombstone и правила purge (§3.5).
8. Слияние, страница и курсор — одной транзакцией.

**Логи** (§8.8): тела запросов регистрации, входа, восстановления и смены пароля не логируются; токены и `auth_key` — никогда, хэши — допустимо.

## B. Тест-план (S4a — протокол, S4b — конверт и ключи)

Всё, кроме тест-векторов, — property-тесты: 2–3 клиента в одном процессе, со случайными задержками, дубликатами, перестановками и обрывами (ТЗ §14). Критерии ТЗ §10.2 S4a входят сюда без повторения.

**S4a — синхронизация**

1. Сходимость состояния записей при любом порядке доставки; повтор push идемпотентен.
2. Сравнение строк HLC совпадает со сравнением кортежей; переполнение счётчика монотонно.
3. Перезапуск после приёма «будущей» метки не выдаёт повторных HLC.
4. Часы тикают от конвертов, отброшенных по `op_id` или tombstone: следующая локальная метка больше.
5. Две правки офлайн на одном устройстве (с сжатием outbox и без), затем синхронизация — ревизии `conflict` **не** создаются.
6. Настоящая одновременная правка — ровно одна ревизия `conflict` с одинаковым id и содержимым на всех устройствах.
7. Конфликт текста сохраняет обе версии (ТЗ).
8. Первая привязка двух баз с записями старше 180 дней, обе с Inbox — всё уходит на push, карантин пуст, Inbox один.
9. Purge без аккаунта, затем привязка копии базы — `purged` уходит, запись не воскресает.
10. Восстановление против purge в обоих порядках `seq`, плюс нечестный порядок сервера (purge принят после восстановления) — клиент не теряет запись и детей, расхождения нет.
11. Ack purge при одновременном локальном восстановлении — запись не удаляется молча, уходит в карантин.
12. Ответ `purged` на push живой правки — запись с детьми в карантине у автора, у остальных её нет, ленты совпадают.
13. Ребёнок приходит после `purged` родителя — удалён локально, `purged(child)` отправлен, повтор — no-op.
14. Устройство вернулось через 200 дней с правкой удалённой записи — карантин, без воскрешения (ТЗ).
15. Восстановление сервера из бэкапа (`generation`) с неподтверждённым outbox — всё дошло, `server_known`-записи вернулись на сервер.
16. `pending_unknown` → обновление клиента → применение даёт то же состояние, что прямое применение.
17. Смешанные `sv` по группам: старый клиент правит `flags`, новый — `body`; обе правки выживают, неизвестная группа не теряется при переотправке старым клиентом.
18. Клиент с часами на 25 ч вперёд — push отвергнут; после исправления часов синхронизация восстанавливается без потери локальных правок, `base` и id ревизий пересчитаны.
19. Восстановление из корзины доходит до другого устройства (ТЗ).
20. Удаление чата с поздним переносом сообщения в него — сообщение в Inbox с пометкой, ничего не потеряно.
21. Копия базы с тем же `device_id`: совпадающий HLC разрешается детерминированно, тело проигравшего в ревизии.
22. Конверт, который проходит AEAD, но не проходит разбор, — карантин `malformed`, синхронизация продолжается.
23. Сервер: параллельные push одного аккаунта — курсор не пропускает записи (S4c).

**S4b — конверт и ключи**

1. Тест-векторы с фиксированными ключом, nonce и входом и ожидаемыми байтами для: конверта, `AD_env`, обеих обёрток с `AD_wrap`, `crypto_kdf`-подключей, выравнивания, id ревизии `conflict`, blob из 0 байт, 1 МиБ − 1, ровно 1 МиБ, 1 МиБ + 1.
2. Подмена каждого открытого поля (HLC, `purged`, `sv`, `key_id`, `format_version`, `account_id`) и перестановка конверта к другой записи обнаруживаются (ТЗ).
3. Слишком большой `header_len` или длина отвергаются до выделения памяти.
4. Обрезанный blob, blob без `TAG_FINAL`, байты после `TAG_FINAL` — ошибка.
5. Смена пароля без перешифровки; восстановление по 24 словам (ТЗ).
6. После восстановления старые слова недействительны; перевыпуск слов работает.
7. Сервер прислал профиль ниже виденного или неизвестный — клиент отказывает до вывода ключа.
8. Неизвестный логин: соль 16 байт, тот же профиль; ответы восстановления неразличимы.
9. Мнемоника → `to_entropy` (не `to_seed`); опечатка ловится контрольной суммой.
10. Логи сервера и клиента после прогона не содержат `auth_key`, `rc_auth_key`, токенов, текста.
11. Сборка выбранной привязки libsodium на macOS, Windows, Linux — в начале S4b.

## C. Последствия для схемы §4.2 ТЗ

ТЗ §4.2 — черновик для ADR. В слайсе 1 нужно:
- **во всех синхронизируемых таблицах:** `hlc_json` вида `{"<группа>":{"h":"HLC","sv":1}}`, `server_known INTEGER NOT NULL DEFAULT 0`, `unknown_json` вида `{"groups":{…},"fields":{"<группа>":{…}}}`;
- `message`: в группе `placement` — `moved_from_chat`, в `body` — `base`;
- `message_revision`: `hlc_json` (только `core`), `schema_version`, `unknown_json`;
- `attachment`: `blob_key`, `unknown_json`;
- новые таблицы:
  - `tombstone(entity, id, purged_of, purged_at)`;
  - `quarantine(id, entity, entity_id, payload, reason, created_at)`;
  - `pending_unknown(seq, envelope)`;
- `change_outbox`: `acked_seq`;
- `sync_state`: `device_id`, `hlc_l`, `hlc_c`, `generation`, `cursor`, `account_id`, `kdf_profile_seen`.

В слайсе 1 синхронизации нет, но `hlc_json`, `sv = 1`, `device_id`, часы HLC и локальные tombstones работают с первой записи.

## D. Примитивы libsodium

| Назначение | Функция | Константы (заголовки ветки `stable`) |
|---|---|---|
| Конверт, обёртки | `crypto_aead_xchacha20poly1305_ietf_*` | ключ 32, nonce 24, тег 16 |
| Blob | `crypto_secretstream_xchacha20poly1305_*` | ключ 32, заголовок 24, `ABYTES` 17, `TAG_FINAL = PUSH \| REKEY` |
| Пароль | `crypto_pwhash`, `ALG_ARGON2ID13` | соль 16; `MODERATE` 3 / 268435456; `SENSITIVE` 4 / 1073741824; одна полоса |
| Подключи | `crypto_kdf_derive_from_key` (BLAKE2b) | мастер-ключ 32, контекст 8, подключ 16–64 (берём 32) |
| Хэши, PRF | `crypto_generichash` (BLAKE2b) | выход 16–64 (32 по умолчанию), ключ 16–64 |
| Выравнивание | `sodium_pad` / `sodium_unpad` (ISO/IEC 7816-4) | блок 1024 |
| Прочее | `randombytes_buf`, `sodium_memcmp`, `sodium_memzero` | — |

## E. Источники

- Документация libsodium: https://doc.libsodium.org/ — разделы `secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction`, `secret-key_cryptography/secretstream`, `password_hashing/default_phf`, `key_derivation`, `padding`.
- Заголовки и исходники libsodium (`stable`): `include/sodium/crypto_{aead_xchacha20poly1305,secretstream_xchacha20poly1305,pwhash_argon2id,kdf_blake2b,generichash_blake2b}.h`, `crypto_pwhash/argon2/pwhash_argon2id.c` (одна полоса), `configure.ac` (1.0.22): https://github.com/jedisct1/libsodium/tree/stable
- Крейты:
  - https://docs.rs/libsodium-rs/latest/libsodium_rs/ — 0.3.0 от 2026-10-14, крейт создан 2025-04-04 (crates.io);
  - https://github.com/jedisct1/libsodium-sys-stable — 1.24.3;
  - https://docs.rs/bip39/latest/bip39/struct.Mnemonic.html — 3.0.0, `parse_in_normalized`, `to_entropy`;
  - https://docs.rs/chacha20poly1305/ — 0.11;
  - `sodiumoxide` в архиве с 2022 года: https://github.com/sodiumoxide/sodiumoxide;
  - версии `argon2` 0.6.0, `blake2` 0.11.0, `aead-stream` 0.6.0, `uuid` 1.28.0 — API crates.io, 2026-10-20.
- RFC 9106, §4: https://www.rfc-editor.org/rfc/rfc9106.html
- BIP-39: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki и `bip-0039/bip-0039-wordlists.md`
- Kulkarni et al., «Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases», 2014, рис. 5: https://cse.buffalo.edu/tech-reports/2014-04.pdf
