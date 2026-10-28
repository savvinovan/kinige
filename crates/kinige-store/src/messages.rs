//! Действия с записью (слайс 3): удалить и восстановить, правка текста (с ревизией `edit`), флаги, день записи, пункт
//! задачи. Каждое — одна транзакция: перезапись одной группы (`kinige_domain::rewrite`), HLC, колонки из новых байтов групп,
//! теги, ревизия и отметка Markdown-копии. Без оптимистичного показа: при ошибке ничего не записано.

use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use crate::send::mark_dirty;
use crate::view::load_view;
use crate::{Now, Store, StoreError};
use kinige_domain::append::{CurrentBody, rewrite_body};
use kinige_domain::groups::normalize_body;
use kinige_domain::history::edit_revision;
use kinige_domain::journal::is_valid_journal_date;
use kinige_domain::message_ops::{self, FlagChange, MessageColumns, message_columns};
use kinige_domain::rewrite::{Groups, Rewritten, is_read_only};
use kinige_domain::tasks::{TaskItemError, set_task_item};
use kinige_domain::{MessageFlag, MessageView};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use uuid::Uuid;

/// Запись, как её видит перезапись: байты групп и признак «подтверждена сервером».
pub(crate) struct Loaded {
    pub(crate) id: String,
    pub(crate) hlc_json: String,
    pub(crate) groups_raw: String,
    pub(crate) server_known: bool,
    pub(crate) columns: MessageColumns,
}

/// Запись по id; нет записи — `MessageNotFound`, повреждённые группы — `Internal`.
pub(crate) fn load(conn: &Connection, id: &str) -> Result<Loaded, StoreError> {
    let (hlc_json, groups_raw, server_known): (String, String, i64) = match conn.query_row(
        "SELECT hlc_json, groups_raw, server_known FROM message WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ) {
        Ok(found) => found,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(StoreError::MessageNotFound),
        Err(error) => return Err(read_err(error)),
    };
    let columns = message_columns(&groups_raw)?;
    Ok(Loaded {
        id: id.to_string(),
        hlc_json,
        groups_raw,
        server_known: server_known != 0,
        columns,
    })
}

impl Loaded {
    pub(crate) fn groups(&self) -> Groups<'_> {
        Groups {
            groups_raw: &self.groups_raw,
            hlc_json: &self.hlc_json,
        }
    }

    /// Запись с группой в неподдерживаемой `sv` — только для чтения (ADR-0001 §4 п. 4).
    pub(crate) fn ensure_writable(&self) -> Result<(), StoreError> {
        if is_read_only(&self.hlc_json)? {
            return Err(StoreError::ReadOnly);
        }
        Ok(())
    }
}

/// Записывает новые байты групп и все колонки, которые из них строятся. Неизменяемые колонки (`created_*`, `origin`,
/// `ext_json`, `reply_to_id`) не трогаются: их группа `core` не переписывается.
pub(crate) fn apply(
    tx: &Transaction<'_>,
    id: &str,
    rewritten: &Rewritten,
) -> Result<MessageColumns, StoreError> {
    let columns = message_columns(&rewritten.groups_raw)?;
    let changed = tx
        .execute(
            "UPDATE message SET chat_id = ?1, body_md = ?2, journal_date = ?3, edited_at = ?4, is_task = ?5, \
             task_done_at = ?6, favorite_at = ?7, pinned_at = ?8, deleted_at = ?9, body_base = ?10, \
             body_supersedes = ?11, placement_base = ?12, placement_supersedes = ?13, hlc_json = ?14, \
             schema_version = ?15, groups_raw = ?16 WHERE id = ?17",
            rusqlite::params![
                columns.chat_id,
                columns.body_md,
                columns.journal_date,
                columns.edited_at,
                columns.is_task,
                columns.task_done_at,
                columns.favorite_at,
                columns.pinned_at,
                columns.deleted_at,
                columns.body_base,
                columns.body_supersedes,
                columns.placement_base,
                columns.placement_supersedes,
                rewritten.hlc_json,
                rewritten.schema_version,
                rewritten.groups_raw,
                id
            ],
        )
        .map_err(write_err)?;
    if changed != 1 {
        return Err(StoreError::Internal("запись для изменения исчезла".into()));
    }
    Ok(columns)
}

fn begin(conn: &mut Connection) -> Result<Transaction<'_>, StoreError> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)
}

/// Правка текста внутри открытой транзакции: ревизия `edit` (снимок до правки), новая группа `body`, теги, отметка копии.
/// Тот же текст — ничего не пишется.
pub(crate) fn edit_body(
    tx: &Transaction<'_>,
    loaded: &Loaded,
    body_md: &str,
    now: &Now,
) -> Result<(), StoreError> {
    if loaded.columns.body_md == body_md {
        return Ok(());
    }
    let hlc = next_hlc(tx, now.ms)?;
    let revision = edit_revision(
        &Uuid::new_v4().to_string(),
        &loaded.id,
        &loaded.groups_raw,
        &loaded.hlc_json,
        now.ms,
        &hlc,
    )?;
    let rewritten = rewrite_body(
        &CurrentBody {
            groups_raw: &loaded.groups_raw,
            hlc_json: &loaded.hlc_json,
            // Outbox появится в слайсе 11: тогда подтверждённость — и колонка, и строка outbox (ADR-0001 §3.4).
            body_confirmed: loaded.server_known,
        },
        body_md,
        now.ms,
        &hlc,
    )?;
    let groups = Rewritten {
        groups_raw: rewritten.groups_raw,
        hlc_json: rewritten.hlc_json,
        schema_version: rewritten.schema_version,
    };
    apply(tx, &loaded.id, &groups)?;
    tx.execute(
        "INSERT INTO message_revision (id, message_id, snapshot_json, saved_at, reason, hlc_json, server_known, \
         schema_version, groups_raw) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
        rusqlite::params![
            revision.id,
            revision.message_id,
            revision.snapshot_json,
            revision.saved_at,
            revision.reason,
            revision.hlc_json,
            revision.schema_version,
            revision.groups_raw
        ],
    )
    .map_err(write_err)?;
    crate::tags::replace_tags(tx, &loaded.id, body_md).map_err(write_err)?;
    crate::search::reindex(tx, &loaded.id, body_md).map_err(write_err)?;
    mark_dirty(tx, &loaded.columns.chat_id, &loaded.columns.journal_date).map_err(write_err)?;
    Ok(())
}

impl Store {
    /// «Удалить»: запись уходит в корзину (группа `deleted`), счётчик чата уменьшается, из Markdown-копии она пропадает.
    /// Повторное удаление ничего не меняет.
    pub fn delete_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        self.set_deleted(id, now, true)
    }

    /// «Восстановить»: запись возвращается на своё место (чат и день не менялись).
    pub fn restore_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        self.set_deleted(id, now, false)
    }

    fn set_deleted(&self, id: &str, now: &Now, deleted: bool) -> Result<(), StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            if loaded.columns.deleted_at.is_some() == deleted {
                return Ok(());
            }
            loaded.ensure_writable()?;
            let hlc = next_hlc(&tx, now.ms)?;
            let rewritten =
                message_ops::set_deleted(&loaded.groups(), deleted.then_some(now.ms), &hlc)?;
            apply(&tx, &id, &rewritten)?;
            mark_dirty(&tx, &loaded.columns.chat_id, &loaded.columns.journal_date)
                .map_err(write_err)?;
            tx.commit().map_err(write_err)
        })?;
        self.wake_mirror();
        Ok(())
    }

    /// Правка текста: старая версия сохраняется ревизией `edit` в той же транзакции; запись остаётся на месте, с прежним
    /// временем и днём. Пустой текст — `Empty`, ничего не меняется. Текст, равный прежнему, ничего не пишет.
    pub fn edit_message(&self, id: &str, body: &str, now: &Now) -> Result<MessageView, StoreError> {
        let body = normalize_body(body)?;
        let (id, now) = (id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            edit_body(&tx, &loaded, &body, &now)?;
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// Щелчок по `- [ ]` в ленте: пункт `index` (с нуля) переводится в состояние `checked`. Это правка текста — ревизия
    /// `edit` пишется так же. Пункта нет или он уже в нужном состоянии — `TaskItemStale`: окно показывало устаревший текст.
    pub fn toggle_task_item(
        &self,
        id: &str,
        index: usize,
        checked: bool,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            let body =
                set_task_item(&loaded.columns.body_md, index, checked).map_err(
                    |error| match error {
                        TaskItemError::NotFound | TaskItemError::AlreadyThere => {
                            StoreError::TaskItemStale
                        }
                    },
                )?;
            edit_body(&tx, &loaded, &body, &now)?;
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// Флаги записи: задача, выполнено, избранное. Запись уже в нужном состоянии — ничего не пишется.
    /// «Выполнено» у записи, которая не задача, — внутренняя ошибка (в интерфейсе такой кнопки нет).
    pub fn set_message_flag(
        &self,
        id: &str,
        flag: MessageFlag,
        on: bool,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            let columns = &loaded.columns;
            let already = match flag {
                MessageFlag::Task => columns.is_task == on,
                // Снять «выполнено» у не-задачи нечего; поставить — ошибка (`change_flag`).
                MessageFlag::TaskDone if columns.is_task => columns.task_done_at.is_some() == on,
                MessageFlag::TaskDone => !on,
                MessageFlag::Favorite => columns.favorite_at.is_some() == on,
            };
            if !already {
                let change = match flag {
                    MessageFlag::Task => FlagChange::Task(on),
                    MessageFlag::TaskDone => FlagChange::TaskDone(on),
                    MessageFlag::Favorite => FlagChange::Favorite(on),
                };
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::change_flag(&loaded.groups(), change, now.ms, &hlc)?;
                apply(&tx, &id, &rewritten)?;
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })
    }

    /// «Изменить дату»: запись переезжает на другой день журнала (группа `placement`; первая перезапись пишет
    /// `base`/`supersedes` по ADR-0001 §3.4). Будущие даты разрешены. Файлы старого и нового дня в Markdown-копии
    /// перезаписываются.
    pub fn set_message_date(
        &self,
        id: &str,
        journal_date: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        if !is_valid_journal_date(journal_date) {
            return Err(StoreError::InvalidDate);
        }
        let (id, journal_date, now) = (id.to_string(), journal_date.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            if loaded.columns.journal_date != journal_date {
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::set_journal_date(
                    &loaded.groups(),
                    &journal_date,
                    // Outbox появится в слайсе 11: тогда подтверждённость — и колонка, и строка outbox (ADR-0001 §3.4).
                    loaded.server_known,
                    &hlc,
                )?;
                apply(&tx, &id, &rewritten)?;
                for day in [&loaded.columns.journal_date, &journal_date] {
                    mark_dirty(&tx, &loaded.columns.chat_id, day).map_err(write_err)?;
                }
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// «Переместить в чат»: запись переезжает в другой чат (группа `placement`, `chat_id`; `journal_date` не меняется;
    /// первая перезапись пишет `base`/`supersedes` по ADR-0001 §3.4). Ревизия `edit` не пишется: текст не менялся. День
    /// записи в старом и в новом чате помечается для перезаписи в Markdown-копии. Целевой чат может быть архивным;
    /// нет такого чата — `ChatNotFound`; запись в корзине — `MessageInTrash` (перенос только для живых записей); запись с
    /// группой в неподдерживаемой `sv` — `ReadOnly`. Тот же чат — ничего не пишется.
    pub fn move_message(
        &self,
        id: &str,
        chat_id: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, chat_id, now) = (id.to_string(), chat_id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            if loaded.columns.deleted_at.is_some() {
                return Err(StoreError::MessageInTrash);
            }
            let chat_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
                    [&chat_id],
                    |row| row.get(0),
                )
                .map_err(write_err)?;
            if !chat_exists {
                return Err(StoreError::ChatNotFound);
            }
            if loaded.columns.chat_id != chat_id {
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::move_to_chat(
                    &loaded.groups(),
                    &chat_id,
                    // Outbox появится в слайсе 11: тогда подтверждённость — и колонка, и строка outbox (ADR-0001 §3.4).
                    loaded.server_known,
                    &hlc,
                )?;
                apply(&tx, &id, &rewritten)?;
                let day = &loaded.columns.journal_date;
                mark_dirty(&tx, &loaded.columns.chat_id, day).map_err(write_err)?;
                mark_dirty(&tx, &chat_id, day).map_err(write_err)?;
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }
}
