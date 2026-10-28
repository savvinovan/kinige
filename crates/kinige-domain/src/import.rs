//! Строка импортированной записи (слайс 4, задача 4.3). Те же пять групп под одним HLC, что у `groups::new_message`,
//! но `core` несёт происхождение: `origin = "import_telegram"`, внешний id `ext` и `reply_to_id`
//! (ADR-0001, приложение §C.3). Исходное время Telegram идёт в `created_at` и `journal_date`, а не в HLC: HLC — момент
//! импорта (ADR-0001 §10, «кроме импорта»).

use crate::groups::{GroupError, MessageRow, NewMessage, Provenance, build_message};
use crate::hlc::is_uuid_v4;
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;

/// `origin` импортированной записи из Telegram: неизменяемо, `origin = 'import_telegram'` ⇔ `ext.src = 'telegram'`.
pub const ORIGIN_IMPORT_TELEGRAM: &str = "import_telegram";
/// `ext.src` записей из Telegram.
pub const EXT_SRC_TELEGRAM: &str = "telegram";

/// Исходные значения импортированной записи; текст уже нормализован (`groups::normalize_body`).
pub struct NewImportedMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub body_md: &'a str,
    pub created_at: i64,
    pub created_tz: &'a str,
    pub journal_date: &'a str,
    /// Id чата Telegram, десятичное целое со знаком или без, до 20 знаков.
    pub ext_chat: &'a str,
    /// Id сообщения Telegram, в том же виде.
    pub ext_msg: &'a str,
    /// Id записи Kinige (UUID v4), на которую это сообщение отвечает, если она уже импортирована.
    pub reply_to_id: Option<&'a str>,
}

/// `^-?[0-9]{1,20}$`: id Telegram 64-битные, в `core` лежат строкой (JCS ограничен ±2⁵³).
fn is_external_id(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    (1..=20).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Строка импортированной записи со всеми пятью группами под одним `hlc`, `sv = 1`, `server_known = 0`.
pub fn new_imported_message(
    input: &NewImportedMessage<'_>,
    hlc: &str,
) -> Result<MessageRow, GroupError> {
    if !is_external_id(input.ext_chat) {
        return Err(GroupError::Invalid("ext.chat не число Telegram"));
    }
    if !is_external_id(input.ext_msg) {
        return Err(GroupError::Invalid("ext.msg не число Telegram"));
    }
    if input.reply_to_id.is_some_and(|id| !is_uuid_v4(id)) {
        return Err(GroupError::Invalid("reply_to_id не UUID v4"));
    }
    let provenance = Provenance {
        origin: ORIGIN_IMPORT_TELEGRAM,
        reply_to_id: input.reply_to_id,
        ext: Some(json!({
            "chat": input.ext_chat,
            "msg": input.ext_msg,
            "src": EXT_SRC_TELEGRAM,
        })),
    };
    let base = NewMessage {
        id: input.id,
        chat_id: input.chat_id,
        body_md: input.body_md,
        created_at: input.created_at,
        created_tz: input.created_tz,
        journal_date: input.journal_date,
    };
    build_message(&base, &provenance, hlc)
}

/// Вид чата Telegram, который импортируется (группы и каналы не импортируются).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ImportChatKind.ts")]
pub enum ImportChatKind {
    /// «Избранное»: записи идут в Inbox.
    Saved,
    /// Личный чат: свой чат Kinige.
    Personal,
}

/// Чат экспорта в окне выбора.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportChatPreview.ts")]
pub struct ImportChatPreview {
    /// Id чата Telegram (строка: 64-битное число).
    pub id: String,
    /// Имя из экспорта; у «Избранного» его нет.
    pub name: Option<String>,
    pub kind: ImportChatKind,
    /// Сколько записей получится: сообщения без служебных и без пустых.
    #[ts(type = "number")]
    pub total: i64,
    /// Из них уже импортировано (есть в `message_ext`).
    #[ts(type = "number")]
    pub existing: i64,
}

/// Что нашлось в экспорте: чаты для выбора и сколько чатов пропущено (группы, каналы, боты).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportPreview.ts")]
pub struct ImportPreview {
    pub chats: Vec<ImportChatPreview>,
    #[ts(type = "number")]
    pub skipped_chats: i64,
}

/// Прогресс импорта после каждой зафиксированной пачки: обработано сообщений из скольких.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportProgress.ts")]
pub struct ImportProgress {
    #[ts(type = "number")]
    pub done: i64,
    #[ts(type = "number")]
    pub total: i64,
}

/// Итог импорта.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportReport.ts")]
pub struct ImportReport {
    #[ts(type = "number")]
    pub imported: i64,
    /// Уже были импортированы раньше (в том числе лежат в корзине).
    #[ts(type = "number")]
    pub skipped_existing: i64,
    /// Служебные сообщения (вступил, закрепил…).
    #[ts(type = "number")]
    pub skipped_service: i64,
    /// Без текста и без вложения.
    #[ts(type = "number")]
    pub skipped_empty: i64,
    /// У скольких импортированных записей вместо вложения осталась заглушка.
    #[ts(type = "number")]
    pub media_placeholders: i64,
    /// Создано чатов Kinige (для личных чатов).
    #[ts(type = "number")]
    pub chats_created: i64,
    /// Файл копии базы, снятой перед импортом; `null` — писать было нечего и копия не снималась.
    pub backup_name: Option<String>,
    /// Импорт остановлен пользователем; всё, что записано до остановки, осталось.
    pub stopped: bool,
}

#[cfg(test)]
mod tests;
