//! Быстрый захват (ТЗ §5.1, слайс 2): контекст окна, новая запись из окна и дописывание к последней записи Inbox. Только
//! Inbox: выбор чата в окне захвата — слайс 3.

use crate::draft::{CAPTURE_KEY, delete_draft};
use crate::hlc_state::next_hlc;
use crate::send::{insert_new_message, mark_dirty};
use crate::{Now, Store, StoreError};
use kinige_domain::append::{AppendCandidate, CurrentBody, can_append, joined_body, rewrite_body};
use kinige_domain::groups::{self, GroupError, INBOX_ID};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{CaptureAction, CaptureOutcome, MessageView};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior};

/// Сколько записей показывает окно захвата над полем (ТЗ §5.1: две-три).
pub const CONTEXT_LEN: usize = 3;

/// Что окну захвата нужно из базы при показе.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureView {
    pub today: String,
    /// До трёх последних неудалённых записей Inbox за сегодня; самая новая последней.
    pub context: Vec<MessageView>,
    /// Запись, к которой ⌘Enter допишет текст.
    pub append_target: Option<String>,
    pub draft: String,
}

fn read_err(error: rusqlite::Error) -> StoreError {
    StoreError::Read(error.to_string())
}

fn write_err(error: rusqlite::Error) -> StoreError {
    StoreError::Write(error.to_string())
}

fn group_err(error: GroupError) -> StoreError {
    match error {
        GroupError::TooLarge => StoreError::TooLarge,
        other => StoreError::Internal(other.to_string()),
    }
}

/// Последняя неудалённая запись Inbox со всем, что нужно правилу дописывания и перезаписи `body`.
struct LastMessage {
    view: MessageView,
    origin: String,
    hlc_json: String,
    groups_raw: String,
    server_known: bool,
}

impl LastMessage {
    fn candidate(&self) -> AppendCandidate<'_> {
        AppendCandidate {
            origin: &self.origin,
            created_at: self.view.created_at,
            journal_date: &self.view.journal_date,
            hlc_json: &self.hlc_json,
        }
    }
}

fn last_message(conn: &Connection) -> rusqlite::Result<Option<LastMessage>> {
    conn.query_row(
        "SELECT id, chat_id, body_md, journal_date, created_at, created_tz, origin, hlc_json, groups_raw, server_known \
         FROM message WHERE chat_id = ?1 AND deleted_at IS NULL \
         ORDER BY journal_date DESC, created_at DESC, id DESC LIMIT 1",
        [INBOX_ID],
        |row: &Row<'_>| {
            Ok(LastMessage {
                view: MessageView {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    body_md: row.get(2)?,
                    journal_date: row.get(3)?,
                    created_at: row.get(4)?,
                    created_tz: row.get(5)?,
                },
                origin: row.get(6)?,
                hlc_json: row.get(7)?,
                groups_raw: row.get(8)?,
                server_known: row.get::<_, i64>(9)? != 0,
            })
        },
    )
    .optional()
}

impl Store {
    /// Состояние окна захвата при показе: контекст за сегодня, цель дописывания, черновик.
    pub fn capture_view(&self, now: &Now) -> Result<CaptureView, StoreError> {
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        let now_ms = now.ms;
        self.read(|conn| {
            let mut statement = conn
                .prepare(
                    "SELECT id, chat_id, body_md, journal_date, created_at, created_tz FROM message \
                     WHERE chat_id = ?1 AND deleted_at IS NULL AND journal_date = ?2 \
                     ORDER BY created_at DESC, id DESC LIMIT ?3",
                )
                .map_err(read_err)?;
            let mut context = statement
                .query_map(
                    rusqlite::params![INBOX_ID, today, CONTEXT_LEN as i64],
                    |row| {
                        Ok(MessageView {
                            id: row.get(0)?,
                            chat_id: row.get(1)?,
                            body_md: row.get(2)?,
                            journal_date: row.get(3)?,
                            created_at: row.get(4)?,
                            created_tz: row.get(5)?,
                        })
                    },
                )
                .map_err(read_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(read_err)?;
            context.reverse();
            let append_target = last_message(conn)
                .map_err(read_err)?
                // Повреждённая запись окну не мешает: подсказки нет, а ⌘Enter сам сообщит об ошибке.
                .filter(|last| can_append(&last.candidate(), now_ms, &today).unwrap_or(false))
                .map(|last| last.view.id);
            let draft = conn
                .query_row("SELECT body FROM draft WHERE key = ?1", [CAPTURE_KEY], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(read_err)?
                .unwrap_or_default();
            Ok(CaptureView {
                today,
                context,
                append_target,
                draft,
            })
        })
    }

    /// Enter в окне захвата: новая запись в Inbox тем же путём, что `send_message` (`origin = 'user'`); черновик окна
    /// очищается в той же транзакции.
    pub fn capture_send(&self, body: &str, now: &Now) -> Result<CaptureOutcome, StoreError> {
        let body = groups::normalize_body(body)?;
        let now = now.clone();
        let outcome = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let message = insert_new_message(&tx, INBOX_ID, &body, &now)?;
            delete_draft(&tx, CAPTURE_KEY).map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(CaptureOutcome {
                action: CaptureAction::Created,
                message,
            })
        })?;
        self.wake_mirror();
        Ok(outcome)
    }

    /// ⌘Enter в окне захвата: правило решает Rust в той же транзакции (`kinige_domain::append::can_append`). Есть цель —
    /// текст дописывается новым абзацем: первая перезапись группы `body` (ADR-0001 §3.3, §3.4); нет цели — новая запись.
    /// Ответ сообщает, что сделано. Если после склейки группа длиннее предела — `TooLarge`, ничего не записано.
    pub fn capture_append(&self, body: &str, now: &Now) -> Result<CaptureOutcome, StoreError> {
        let addition = groups::normalize_body(body)?;
        let now = now.clone();
        let outcome = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
            let target = match last_message(&tx).map_err(write_err)? {
                Some(last) if can_append(&last.candidate(), now.ms, &today).map_err(group_err)? => {
                    Some(last)
                }
                _ => None,
            };
            let outcome = match target {
                Some(last) => {
                    let combined = joined_body(&last.view.body_md, &addition);
                    let hlc = next_hlc(&tx, now.ms)?;
                    let rewritten = rewrite_body(
                        &CurrentBody {
                            groups_raw: &last.groups_raw,
                            hlc_json: &last.hlc_json,
                            server_known: last.server_known,
                        },
                        &combined,
                        now.ms,
                        &hlc,
                    )
                    .map_err(group_err)?;
                    let changed = tx
                        .execute(
                            "UPDATE message SET body_md = ?1, edited_at = ?2, body_base = ?3, body_supersedes = ?4, \
                             hlc_json = ?5, schema_version = ?6, groups_raw = ?7 WHERE id = ?8",
                            rusqlite::params![
                                rewritten.body_md,
                                rewritten.edited_at,
                                rewritten.body_base,
                                rewritten.body_supersedes,
                                rewritten.hlc_json,
                                rewritten.schema_version,
                                rewritten.groups_raw,
                                last.view.id
                            ],
                        )
                        .map_err(write_err)?;
                    if changed != 1 {
                        return Err(StoreError::Internal("запись для дописывания исчезла".into()));
                    }
                    mark_dirty(&tx, INBOX_ID, &last.view.journal_date).map_err(write_err)?;
                    CaptureOutcome {
                        action: CaptureAction::Appended,
                        message: MessageView {
                            body_md: rewritten.body_md,
                            ..last.view
                        },
                    }
                }
                None => CaptureOutcome {
                    action: CaptureAction::Created,
                    message: insert_new_message(&tx, INBOX_ID, &addition, &now)?,
                },
            };
            delete_draft(&tx, CAPTURE_KEY).map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(outcome)
        })?;
        self.wake_mirror();
        Ok(outcome)
    }
}
