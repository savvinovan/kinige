//! Черновики (ТЗ §5, §5.1): локальная несинхронизируемая таблица `draft(key, body, updated_at)`. Ключи — окно захвата
//! (`capture`) и чат главного окна (`chat:<id>`). Черновик очищается в той же транзакции, что отправка. В Markdown-копию
//! черновики не попадают.

use crate::error::{read_err, write_err};
use crate::{Now, Store, StoreError};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};

/// Ключ черновика окна захвата.
pub(crate) const CAPTURE_KEY: &str = "capture";
/// Предел черновика в байтах: черновик не должен раздувать базу; запись длиннее предела всё равно не отправить.
pub const MAX_DRAFT_BYTES: usize = 1024 * 1024;

pub(crate) fn chat_key(chat_id: &str) -> String {
    format!("chat:{chat_id}")
}

pub(crate) fn delete_draft(tx: &Transaction<'_>, key: &str) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM draft WHERE key = ?1", [key])?;
    Ok(())
}

impl Store {
    /// Черновик чата; пустая строка, если его нет.
    pub fn get_chat_draft(&self, chat_id: &str) -> Result<String, StoreError> {
        self.read_draft(&chat_key(chat_id))
    }

    /// Сохраняет черновик чата; пустой текст удаляет его. Чата нет — `ChatNotFound`.
    pub fn set_chat_draft(&self, chat_id: &str, body: &str, now: &Now) -> Result<(), StoreError> {
        let chat_id = chat_id.to_string();
        self.write_draft(Some(chat_id.clone()), chat_key(&chat_id), body, now)
    }

    /// Черновик окна захвата.
    pub fn get_capture_draft(&self) -> Result<String, StoreError> {
        self.read_draft(CAPTURE_KEY)
    }

    pub fn set_capture_draft(&self, body: &str, now: &Now) -> Result<(), StoreError> {
        self.write_draft(None, CAPTURE_KEY.to_string(), body, now)
    }

    /// Сбой хоткея буфера: текст буфера идёт в черновик окна захвата под прежним непустым черновиком
    /// (`kinige_domain::draft::joined_draft`), ничего не затирается. Чтение и запись — в одной транзакции писателя.
    /// Возвращает текст поля целиком и признак «записано в базу»: если результат длиннее [`MAX_DRAFT_BYTES`], база
    /// остаётся прежней, а текст всё равно возвращается (окно покажет его из памяти).
    pub fn merge_clipboard_into_capture_draft(
        &self,
        clipboard: &str,
        now: &Now,
    ) -> Result<(String, bool), StoreError> {
        let clipboard = clipboard.to_string();
        let now_ms = now.ms;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let existing: String = tx
                .query_row(
                    "SELECT body FROM draft WHERE key = ?1",
                    [CAPTURE_KEY],
                    |row| row.get(0),
                )
                .optional()
                .map_err(write_err)?
                .unwrap_or_default();
            let joined = kinige_domain::draft::joined_draft(&existing, &clipboard);
            if joined.len() > MAX_DRAFT_BYTES {
                return Ok((joined, false));
            }
            tx.execute(
                "INSERT INTO draft(key, body, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(key) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at",
                rusqlite::params![CAPTURE_KEY, joined, now_ms],
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok((joined, true))
        })
    }

    fn read_draft(&self, key: &str) -> Result<String, StoreError> {
        self.read(|conn| {
            Ok(conn
                .query_row("SELECT body FROM draft WHERE key = ?1", [key], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(read_err)?
                .unwrap_or_default())
        })
    }

    fn write_draft(
        &self,
        require_chat: Option<String>,
        key: String,
        body: &str,
        now: &Now,
    ) -> Result<(), StoreError> {
        if body.len() > MAX_DRAFT_BYTES {
            return Err(StoreError::TooLarge);
        }
        let body = body.to_string();
        let now_ms = now.ms;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            if let Some(chat_id) = require_chat {
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
                        [&chat_id],
                        |row| row.get(0),
                    )
                    .map_err(write_err)?;
                if !exists {
                    return Err(StoreError::ChatNotFound);
                }
            }
            if body.is_empty() {
                delete_draft(&tx, &key).map_err(write_err)?;
            } else {
                tx.execute(
                    "INSERT INTO draft(key, body, updated_at) VALUES (?1, ?2, ?3) \
                     ON CONFLICT(key) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at",
                    rusqlite::params![key, body, now_ms],
                )
                .map_err(write_err)?;
            }
            tx.commit().map_err(write_err)
        })
    }
}
