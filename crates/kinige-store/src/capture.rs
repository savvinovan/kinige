//! Быстрый захват (ТЗ §5.1, слайс 2): контекст окна, новая запись из окна и дописывание к последней записи Inbox. Только
//! Inbox: выбор чата в окне захвата не входит ни в один слайс (§1.2 п. 1, §12.3).

use crate::draft::{CAPTURE_KEY, delete_draft};
use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use crate::send::{insert_new_message, mark_dirty};
use crate::view::{VIEW_COLUMNS, view_from_row, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::append::{AppendCandidate, CurrentBody, can_append, joined_body, rewrite_body};
use kinige_domain::groups::{self, GroupError, INBOX_ID};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{CaptureAction, CaptureOutcome, MessageView};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

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

/// Ошибки домена при дописывании: слишком длинный текст — `TooLarge`, остальное (в том числе группа новее
/// поддерживаемой, которую правило цели уже отсеяло) — внутренняя ошибка.
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
        &format!(
            "SELECT {VIEW_COLUMNS}, m.origin, m.hlc_json, m.groups_raw, m.server_known \
             FROM message m WHERE m.chat_id = ?1 AND m.deleted_at IS NULL \
             ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT 1"
        ),
        [INBOX_ID],
        |row| {
            Ok(LastMessage {
                view: view_from_row_at(row, 0)?,
                origin: row.get(10)?,
                hlc_json: row.get(11)?,
                groups_raw: row.get(12)?,
                server_known: row.get::<_, i64>(13)? != 0,
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
                .prepare(&format!(
                    "SELECT {VIEW_COLUMNS} FROM message m \
                     WHERE m.chat_id = ?1 AND m.deleted_at IS NULL AND m.journal_date = ?2 \
                     ORDER BY m.created_at DESC, m.id DESC LIMIT ?3"
                ))
                .map_err(read_err)?;
            let mut context = statement
                .query_map(
                    rusqlite::params![INBOX_ID, today, CONTEXT_LEN as i64],
                    view_from_row,
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
                .query_row(
                    "SELECT body FROM draft WHERE key = ?1",
                    [CAPTURE_KEY],
                    |r| r.get::<_, String>(0),
                )
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

    /// Хоткей буфера: текст из буфера обмена — новая запись в Inbox. Черновики не трогаются: сохранение буфера не отправка
    /// набранного текста.
    pub fn save_clipboard(&self, text: &str, now: &Now) -> Result<MessageView, StoreError> {
        let body = groups::normalize_body(text)?;
        let now = now.clone();
        let message = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let message = insert_new_message(&tx, INBOX_ID, &body, &now)?;
            tx.commit().map_err(write_err)?;
            Ok(message)
        })?;
        self.wake_mirror();
        Ok(message)
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
                            // Outbox появится в слайсе 11: тогда подтверждённость — и колонка, и строка outbox (ADR §3.4).
                            body_confirmed: last.server_known,
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
                    crate::tags::replace_tags(&tx, &last.view.id, &rewritten.body_md)
                        .map_err(write_err)?;
                    mark_dirty(&tx, INBOX_ID, &last.view.journal_date).map_err(write_err)?;
                    CaptureOutcome {
                        action: CaptureAction::Appended,
                        message: MessageView {
                            tags: kinige_domain::tags::parse_tags(&rewritten.body_md),
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
