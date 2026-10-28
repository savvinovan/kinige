//! Корзина (слайс 3, ТЗ §2.1, §4.1, §16; ADR-0001 §3.5): список, «Удалить навсегда» и автоудаление через 30 дней.
//!
//! Физическое удаление (purge) локальное, пока нет аккаунта: запись и её ревизии уходят из таблиц, а в `tombstone`
//! остаются следы со снимком `purged_of` — HLC **всех** групп, включая неизвестные. У ревизий `cascade_of` — id записи,
//! `held` — `NULL`. Следы хранятся до первой привязки к аккаунту и не меньше 180 дней; в слайсах 1–10 их никто не стирает.
//! Корзина в Markdown-копию не попадает.
//!
//! Purge читает только колонки `hlc_json` и `deleted_at`, не `groups_raw`: запись с повреждёнными байтами групп всё равно
//! можно удалить навсегда (в снимок попадает то, что читается). Запись, у которой группа `deleted` в неподдерживаемой `sv`,
//! не удаляется ни вручную, ни автоматически (ADR-0001 §4 п. 4: `purged` создаётся, только если `deleted` поддерживается).

use crate::error::{read_err, write_err};
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::TrashItem;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::{
    DAY_MS, TRASH_DAYS, days_left, deleted_group_supported, purged_of_best_effort,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

/// Сколько записей удаляется за одну транзакцию при автоудалении.
const PURGE_BATCH: i64 = 200;
/// Наибольшее число записей в списке корзины.
const TRASH_LIMIT: i64 = 1000;

/// Запись, как её видит purge: без разбора групп.
struct PurgeTarget {
    id: String,
    hlc_json: String,
    deleted_at: Option<i64>,
}

fn load_target(conn: &Connection, id: &str) -> Result<PurgeTarget, StoreError> {
    conn.query_row(
        "SELECT hlc_json, deleted_at FROM message WHERE id = ?1",
        [id],
        |row| {
            Ok(PurgeTarget {
                id: id.to_string(),
                hlc_json: row.get(0)?,
                deleted_at: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(read_err)?
    .ok_or(StoreError::MessageNotFound)
}

/// Физически удаляет запись из корзины вместе с ревизиями и пишет tombstones. Вызывающий открыл транзакцию.
fn purge(tx: &Connection, target: &PurgeTarget, now_ms: i64) -> Result<(), StoreError> {
    let message_snapshot = purged_of_best_effort(&target.hlc_json)?;
    tx.execute(
        "INSERT OR REPLACE INTO tombstone (entity, id, purged_of, purged_at, cascade_of, held) \
         VALUES ('message', ?1, ?2, ?3, NULL, NULL)",
        (&target.id, message_snapshot, now_ms),
    )
    .map_err(write_err)?;
    let revisions: Vec<(String, String)> = {
        let mut statement = tx
            .prepare("SELECT id, hlc_json FROM message_revision WHERE message_id = ?1")
            .map_err(write_err)?;
        let rows = statement
            .query_map([&target.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(write_err)?;
        rows.collect::<Result<_, _>>().map_err(write_err)?
    };
    for (revision_id, hlc_json) in &revisions {
        tx.execute(
            "INSERT OR REPLACE INTO tombstone (entity, id, purged_of, purged_at, cascade_of, held) \
             VALUES ('message_revision', ?1, ?2, ?3, ?4, NULL)",
            (
                revision_id,
                purged_of_best_effort(hlc_json)?,
                now_ms,
                &target.id,
            ),
        )
        .map_err(write_err)?;
    }
    tx.execute(
        "DELETE FROM message_revision WHERE message_id = ?1",
        [&target.id],
    )
    .map_err(write_err)?;
    tx.execute(
        "DELETE FROM message_tag WHERE message_id = ?1",
        [&target.id],
    )
    .map_err(write_err)?;
    crate::ext::remove(tx, &target.id).map_err(write_err)?;
    // Следы записи в журнале обращений агентов уходят вместе с ней; сама строка обращения остаётся (ADR-0009).
    crate::agent_log::delete_log_messages(tx, &target.id).map_err(write_err)?;
    tx.execute("DELETE FROM message WHERE id = ?1", [&target.id])
        .map_err(write_err)?;
    // После удаления строки `message`: повреждённый индекс перестраивается из оставшихся записей, и удалённая в него не вернётся.
    crate::search::remove(tx, &target.id).map_err(write_err)?;
    Ok(())
}

impl Store {
    /// Записи из корзины, самые недавно удалённые первыми: чат, дата, сколько осталось до автоудаления.
    pub fn list_trash(&self, now: &Now) -> Result<Vec<TrashItem>, StoreError> {
        let now_ms = now.ms;
        self.read(|conn| {
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {VIEW_COLUMNS}, m.deleted_at, c.title, c.icon, c.id = ?1 \
                     FROM message m LEFT JOIN chat c ON c.id = m.chat_id \
                     WHERE m.deleted_at IS NOT NULL \
                     ORDER BY m.deleted_at DESC, m.id DESC LIMIT ?2"
                ))
                .map_err(read_err)?;
            let rows = statement
                .query_map(rusqlite::params![INBOX_ID, TRASH_LIMIT], |row| {
                    let deleted_at: i64 = row.get(VIEW_LEN)?;
                    Ok(TrashItem {
                        message: view_from_row_at(row, 0)?,
                        chat_title: row
                            .get::<_, Option<String>>(VIEW_LEN + 1)?
                            .unwrap_or_default(),
                        chat_icon: row.get(VIEW_LEN + 2)?,
                        chat_is_inbox: row.get::<_, Option<bool>>(VIEW_LEN + 3)?.unwrap_or(false),
                        deleted_at,
                        days_left: days_left(deleted_at, now_ms),
                    })
                })
                .map_err(read_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(read_err)
        })
    }

    /// «Удалить навсегда» у записи в корзине: то же, что автоудаление через 30 дней. Записи нет — `MessageNotFound`,
    /// запись не в корзине — `NotInTrash`: живую запись так не удалить. Группа `deleted` новее поддерживаемой — `ReadOnly`.
    pub fn purge_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        let (id, now_ms) = (id.to_string(), now.ms);
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let target = load_target(&tx, &id)?;
            if target.deleted_at.is_none() {
                return Err(StoreError::NotInTrash);
            }
            if !deleted_group_supported(&target.hlc_json) {
                return Err(StoreError::ReadOnly);
            }
            purge(&tx, &target, now_ms)?;
            tx.commit().map_err(write_err)
        })
    }

    /// Автоудаление: записи, пролежавшие в корзине 30 суток, удаляются физически (при старте и раз в сутки). Возвращает,
    /// сколько удалено. Повреждённая запись не останавливает остальные: она остаётся в корзине, а последняя ошибка
    /// возвращается после обработки остальных. Записи с группой `deleted` в неподдерживаемой `sv` пропускаются молча.
    pub fn purge_expired(&self, now: &Now) -> Result<usize, StoreError> {
        match self.purge_expired_report(now) {
            (purged, None) => Ok(purged),
            (_, Some(error)) => Err(error),
        }
    }

    /// То же, но результат частичный и ошибка не прерывает вызывающего: сколько удалено и последняя ошибка (если была).
    pub(crate) fn purge_expired_report(&self, now: &Now) -> (usize, Option<StoreError>) {
        let border = now.ms.saturating_sub(TRASH_DAYS * DAY_MS);
        let now_ms = now.ms;
        let mut purged = 0usize;
        let mut skip: Vec<String> = Vec::new();
        let mut last_error = None;
        loop {
            let skipped = skip.clone();
            let outcome = self.write(move |conn| {
                let mut tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(write_err)?;
                let ids: Vec<String> = {
                    let mut statement = tx
                        .prepare(
                            "SELECT id FROM message WHERE deleted_at IS NOT NULL AND deleted_at <= ?1 \
                             ORDER BY deleted_at, id LIMIT ?2",
                        )
                        .map_err(write_err)?;
                    let rows = statement
                        .query_map(
                            rusqlite::params![border, PURGE_BATCH + skipped.len() as i64],
                            |row| row.get::<_, String>(0),
                        )
                        .map_err(write_err)?;
                    rows.collect::<Result<Vec<_>, _>>().map_err(write_err)?
                };
                let mut done = 0usize;
                let mut bad = Vec::new();
                let mut error = None;
                for id in ids.into_iter().filter(|id| !skipped.contains(id)) {
                    // Точка сохранения на запись: повреждённая откатывается одна, уже удалённые остаются.
                    let step = tx.savepoint().map_err(write_err)?;
                    let result = load_target(&step, &id).and_then(|target| {
                        if deleted_group_supported(&target.hlc_json) {
                            purge(&step, &target, now_ms).map(|()| true)
                        } else {
                            Ok(false)
                        }
                    });
                    match result {
                        Ok(true) => {
                            step.commit().map_err(write_err)?;
                            done += 1;
                        }
                        Ok(false) => {
                            drop(step);
                            bad.push(id);
                        }
                        Err(e) => {
                            drop(step);
                            bad.push(id);
                            error = Some(e);
                        }
                    }
                }
                tx.commit().map_err(write_err)?;
                Ok((done, bad, error))
            });
            let (done, bad, error) = match outcome {
                Ok(found) => found,
                Err(error) => {
                    last_error = Some(error);
                    break;
                }
            };
            purged += done;
            // Пропущенные записи (неподдерживаемая группа, повреждённый JSON) не мешают здоровым за ними: выборка в
            // следующем проходе длиннее на число пропущенных, цикл кончается только когда новых записей не осталось.
            let seen_new = done + bad.len();
            skip.extend(bad);
            if error.is_some() {
                last_error = error;
            }
            if seen_new == 0 {
                break;
            }
        }
        (purged, last_error)
    }
}
