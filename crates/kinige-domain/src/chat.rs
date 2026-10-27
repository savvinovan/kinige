//! Чаты (слайс 3): новый чат со всеми шестью группами, перезапись `meta` (имя, эмодзи) и `state` (закреплён, архив),
//! ключ порядка `order.sort_key` и разбор колонок из групп (ADR-0001 §3.2, приложение §A.0). Без IO.

use crate::groups::{ChatRow, GroupError, INBOX_ID, SCHEMA_VERSION, assemble};
use crate::rewrite::{
    Groups, Rewritten, corrupt, flag, group_object, opt_int, opt_str, req_str, rewrite_group,
};
use serde_json::{Value, json};
use std::fmt;

/// Эмодзи чата по умолчанию (слайс 3: «без эмодзи — 💬»).
pub const DEFAULT_ICON: &str = "💬";
/// Длина имени чата в знаках (слайс 3: от 1 до 128).
pub const MAX_TITLE_CHARS: usize = 128;
/// Предел записи эмодзи в байтах: один знак с вариантами и ZWJ-последовательностью (семья, флаг, оттенок кожи) помещается.
pub const MAX_ICON_BYTES: usize = 32;

/// Почему введённые имя или эмодзи не приняты.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatInputError {
    TitleEmpty,
    TitleTooLong,
    IconInvalid,
}

impl fmt::Display for ChatInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ChatInputError::TitleEmpty => "имя чата пустое",
            ChatInputError::TitleTooLong => "имя чата длиннее 128 знаков",
            ChatInputError::IconInvalid => "эмодзи чата не принято",
        })
    }
}

impl std::error::Error for ChatInputError {}

/// Имя чата: пробелы по краям убраны, от 1 до 128 знаков, без управляющих символов (в них перевод строки и табуляция).
pub fn normalize_title(raw: &str) -> Result<String, ChatInputError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ChatInputError::TitleEmpty);
    }
    if title.chars().any(char::is_control) {
        return Err(ChatInputError::TitleEmpty);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ChatInputError::TitleTooLong);
    }
    Ok(title.to_string())
}

/// Эмодзи чата: непустая строка до [`MAX_ICON_BYTES`] без пробелов и управляющих символов; нет значения — 💬.
/// Что это именно эмодзи, домен не проверяет: пикер выдаёт только эмодзи, а чужой клиент мог записать что угодно, и
/// ADR-0001 §4 требует показывать записанное.
pub fn normalize_icon(raw: Option<&str>) -> Result<String, ChatInputError> {
    let Some(icon) = raw.map(str::trim).filter(|icon| !icon.is_empty()) else {
        return Ok(DEFAULT_ICON.to_string());
    };
    if icon.len() > MAX_ICON_BYTES || icon.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(ChatInputError::IconInvalid);
    }
    Ok(icon.to_string())
}

const KEY_DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const KEY_WIDTH: usize = 6;

/// Ключ порядка для нового чата в конце списка (ТЗ §4.2: дробный индекс). Ключ чата — `a` и шесть цифр системы по
/// основанию 36, поэтому строки сравниваются так же, как числа; Inbox имеет `a0` и стоит первым. Чужой ключ другого формата
/// (другой клиент) получает приставку: строка, у которой прежний максимум — начало, больше его и всех меньших. Одинаковые
/// ключи на двух устройствах допустимы: порядок решает `id` (приложение §A.0).
pub fn key_after(max: Option<&str>) -> String {
    let Some(max) = max else {
        return format!("a{}", "0".repeat(KEY_WIDTH - 1) + "1");
    };
    let digits = max.strip_prefix('a').map(str::as_bytes);
    let own = digits.is_some_and(|d| {
        d.len() == KEY_WIDTH && d.iter().all(|b| KEY_DIGITS.contains(b)) && d != [b'z'; KEY_WIDTH]
    });
    if !own {
        // Inbox (`a0`) и чужие форматы: берём стартовое значение или дописываем к максимуму.
        return if max == "a0" || max < "a000001" {
            key_after(None)
        } else {
            format!("{max}1")
        };
    }
    let mut next = digits.unwrap_or_default().to_vec();
    for slot in next.iter_mut().rev() {
        let at = KEY_DIGITS
            .iter()
            .position(|d| d == slot)
            .unwrap_or_default();
        if at + 1 < KEY_DIGITS.len() {
            *slot = KEY_DIGITS[at + 1];
            break;
        }
        *slot = KEY_DIGITS[0];
    }
    format!("a{}", String::from_utf8(next).expect("цифры ASCII"))
}

/// Исходные значения нового чата; имя и эмодзи уже нормализованы.
pub struct NewChat<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub icon: &'a str,
    pub sort_key: &'a str,
    pub created_at: i64,
}

/// Новый чат сразу со всеми шестью группами (`core`, `meta`, `order`, `state`, `privacy`, `deleted`) под одним HLC, `sv = 1`.
pub fn new_chat(input: &NewChat<'_>, hlc: &str) -> Result<ChatRow, GroupError> {
    let groups: [(&str, Value); 6] = [
        (
            "core",
            json!({"created_at": input.created_at, "is_inbox": false}),
        ),
        (
            "meta",
            json!({"color": null, "icon": input.icon, "title": input.title}),
        ),
        ("order", json!({"sort_key": input.sort_key})),
        ("state", json!({"archived_at": null, "pinned_at": null})),
        ("privacy", json!({"ai_excluded": false})),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(ChatRow {
        id: input.id.to_string(),
        title: input.title.to_string(),
        icon: Some(input.icon.to_string()),
        color: None,
        sort_key: input.sort_key.to_string(),
        is_inbox: false,
        ai_excluded: false,
        pinned_at: None,
        archived_at: None,
        created_at: input.created_at,
        updated_at: input.created_at,
        deleted_at: None,
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// Колонки таблицы `chat`, которые строятся из групп.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatColumns {
    pub created_at: i64,
    pub is_inbox: bool,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_key: String,
    pub pinned_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub ai_excluded: bool,
    pub deleted_at: Option<i64>,
}

/// Колонки чата по его `groups_raw`.
pub fn chat_columns(groups_raw: &str) -> Result<ChatColumns, GroupError> {
    let core = group_object(groups_raw, "core")?;
    let meta = group_object(groups_raw, "meta")?;
    let order = group_object(groups_raw, "order")?;
    let state = group_object(groups_raw, "state")?;
    let privacy = group_object(groups_raw, "privacy")?;
    let deleted = group_object(groups_raw, "deleted")?;
    Ok(ChatColumns {
        created_at: opt_int(&core, "created_at")?.ok_or_else(|| corrupt("нет created_at"))?,
        is_inbox: flag(&core, "is_inbox")?,
        title: opt_str(&meta, "title")?.unwrap_or_default(),
        icon: opt_str(&meta, "icon")?,
        color: opt_str(&meta, "color")?,
        sort_key: req_str(&order, "sort_key")?,
        pinned_at: opt_int(&state, "pinned_at")?,
        archived_at: opt_int(&state, "archived_at")?,
        ai_excluded: flag(&privacy, "ai_excluded")?,
        deleted_at: opt_int(&deleted, "deleted_at")?,
    })
}

/// Перезаписывает `meta`: имя и/или эмодзи (`None` — поле остаётся); `color` и неизвестные поля — дословно.
/// Inbox переименовать нельзя: его имя в базе пустое, интерфейс показывает переведённое.
pub fn set_meta(
    current: &Groups<'_>,
    title: Option<&str>,
    icon: Option<&str>,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "meta", new_hlc, None, |meta, _| {
        if let Some(title) = title {
            meta.insert("title".into(), json!(title));
        }
        if let Some(icon) = icon {
            meta.insert("icon".into(), json!(icon));
        }
        Ok(())
    })
}

/// Изменение группы `state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateChange {
    /// Закрепить (`Some(время)`) или открепить.
    Pinned(Option<i64>),
    /// В архив (`Some(время)`) или из архива. Inbox архивировать нельзя.
    Archived(Option<i64>),
}

/// Перезаписывает `state`: закрепление или архив; второе поле и неизвестные поля — дословно.
pub fn set_state(
    current: &Groups<'_>,
    change: StateChange,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "state", new_hlc, None, |state, _| {
        match change {
            StateChange::Pinned(at) => state.insert("pinned_at".into(), json!(at)),
            StateChange::Archived(at) => state.insert("archived_at".into(), json!(at)),
        };
        Ok(())
    })
}

/// Это Inbox (по id, а не по флагу, ADR-0001 §3.6).
pub fn is_inbox(chat_id: &str) -> bool {
    chat_id == INBOX_ID
}
