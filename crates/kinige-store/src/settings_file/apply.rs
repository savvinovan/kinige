//! Применение разобранного файла к базе (ADR-0010): одна транзакция писателя на всё, кроме сочетаний (их принимает система
//! и регистрирует слой Tauri). В той же транзакции: сравнение «до / после» доступа агента, ключи `mcp_*`, хэш файла и
//! строка журнала обращений.

use super::parse::{ChatRef, Field, Parsed};
use crate::agent::{KEY_CHATS, KEY_ENABLED, KEY_MODE, load_settings};
use crate::agent_log::insert_log;
use crate::agent_settings::put;
use crate::error::write_err;
use crate::send::mark_all_dirty;
use crate::settings::{
    KEY_MIRROR_DIR, KEY_TEXT_SIZE, effective_mirror_dir, get_setting, stored_accelerator,
};
use crate::{AgentSettings, Store, StoreError};
use kinige_domain::chat::normalize_title;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{AgentMode, KeyIssue, KeyIssueKind, ShortcutKind, TextSize};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Хэш последнего синхронизированного файла (`app_setting`, в файл не идёт).
pub(crate) const KEY_FILE_HASH: &str = "settings_file_hash";
/// Подпись строки журнала обращений при изменении доступа из файла: по `tool`, а не по `client` (клиент MCP называет себя сам).
pub const LOG_CLIENT: &str = "settings.json";
pub const LOG_TOOL: &str = "settings_file";
/// Название Inbox в файле: в базе оно пустое, интерфейс переводит его по языку, файл не зависит от языка.
pub(crate) const INBOX_FILE_TITLE: &str = "Inbox";

/// Чат для сопоставления с элементом `agent_access.chats`.
#[derive(Debug, Clone)]
pub(crate) struct ChatInfo {
    pub id: String,
    pub title: String,
    pub ai_excluded: bool,
}

/// Живые чаты (архив входит). У Inbox название `Inbox`.
pub(crate) fn load_chats(conn: &Connection) -> Result<Vec<ChatInfo>, StoreError> {
    let mut statement = conn
        .prepare("SELECT id, title, ai_excluded FROM chat WHERE deleted_at IS NULL")
        .map_err(write_err)?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            Ok(ChatInfo {
                title: if id == INBOX_ID {
                    INBOX_FILE_TITLE.to_string()
                } else {
                    normalize_title(&title).unwrap_or(title)
                },
                id,
                ai_excluded: row.get::<_, i64>(2)? != 0,
            })
        })
        .map_err(write_err)?;
    rows.collect::<Result<_, _>>().map_err(write_err)
}

/// Элементы массива чатов → id, которые можно включить. Неподходящий элемент даёт ошибку этого элемента и не включается;
/// ошибки только сужают доступ. `id` главнее названия: несуществующий `id` — ошибка, без отката на название.
pub(crate) fn resolve_chats(chats: &[ChatInfo], refs: &[ChatRef]) -> (Vec<String>, Vec<KeyIssue>) {
    let by_id: HashMap<&str, &ChatInfo> =
        chats.iter().map(|chat| (chat.id.as_str(), chat)).collect();
    let mut by_title: HashMap<&str, Vec<&ChatInfo>> = HashMap::new();
    for chat in chats {
        by_title.entry(chat.title.as_str()).or_default().push(chat);
    }
    let mut ids: Vec<String> = Vec::new();
    let mut issues = Vec::new();
    for reference in refs {
        let key = format!("agent_access.chats[{}]", reference.index);
        let found: Result<&ChatInfo, KeyIssueKind> = match (&reference.id, &reference.title) {
            (Some(id), _) => by_id
                .get(id.as_str())
                .copied()
                .ok_or(KeyIssueKind::ChatNotFound),
            (None, Some(title)) => match normalize_title(title) {
                Err(_) => Err(KeyIssueKind::Value),
                Ok(title) => match by_title.get(title.as_str()).map(Vec::as_slice) {
                    None | Some([]) => Err(KeyIssueKind::ChatNotFound),
                    Some([chat]) => Ok(*chat),
                    Some(_) => Err(KeyIssueKind::ChatAmbiguous),
                },
            },
            (None, None) => Err(KeyIssueKind::Value),
        };
        match found {
            Ok(chat) if chat.ai_excluded => issues.push(KeyIssue {
                key,
                kind: KeyIssueKind::ChatExcluded,
            }),
            Ok(chat) => {
                if !ids.contains(&chat.id) {
                    ids.push(chat.id.clone());
                }
            }
            Err(kind) => issues.push(KeyIssue { key, kind }),
        }
    }
    (ids, issues)
}

/// Папка копии из файла: абсолютная, существующая папка вне каталога данных.
pub(crate) fn check_folder(path: &str, data_dir: &Path) -> Result<PathBuf, KeyIssueKind> {
    let folder = Path::new(path);
    if !folder.is_absolute() {
        return Err(KeyIssueKind::FolderNotAbsolute);
    }
    if !folder.is_dir() {
        return Err(KeyIssueKind::FolderMissing);
    }
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    if real(folder).starts_with(real(data_dir)) {
        return Err(KeyIssueKind::FolderInsideData);
    }
    Ok(folder.to_path_buf())
}

/// Что нужно применить.
pub(crate) struct ApplyInput {
    pub parsed: Parsed,
    /// Файл безопасен: ключи `agent_access.*` и `mirror.folder` применяются.
    pub trusted: bool,
    pub data_dir: PathBuf,
    /// SHA-256 прочитанных байтов, hex.
    pub hash: String,
    pub at_ms: i64,
}

/// Что изменилось и что ещё нужно сделать после фиксации.
#[derive(Debug, Default)]
pub(crate) struct ApplyOutcome {
    pub issues: Vec<KeyIssue>,
    pub appearance: bool,
    pub mirror: bool,
    pub access: bool,
    /// Сочетания, значение которых из файла отличается от действующего: принимает система (слой Tauri) и сохраняет.
    pub shortcuts: Vec<(ShortcutKind, String)>,
}

impl Store {
    /// Применяет файл одной транзакцией. Ошибка записи — ничего не применено (хэш не сдвинут, файл применится снова).
    pub(crate) fn apply_settings_file(
        &self,
        input: ApplyInput,
    ) -> Result<ApplyOutcome, StoreError> {
        let default_mirror = self.inner.default_mirror_dir.clone();
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let outcome = apply_in(&tx, &input, default_mirror.as_deref())?;
            put(&tx, KEY_FILE_HASH, &input.hash).map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(outcome)
        })
    }
}

fn apply_in(
    conn: &Transaction<'_>,
    input: &ApplyInput,
    default_mirror: Option<&Path>,
) -> Result<ApplyOutcome, StoreError> {
    let parsed = &input.parsed;
    let mut out = ApplyOutcome {
        issues: parsed.issues.clone(),
        ..ApplyOutcome::default()
    };

    // Оформление.
    let size = match &parsed.text_size {
        Field::Absent => None,
        Field::Default => Some(TextSize::Normal),
        Field::Value(size) => Some(*size),
    };
    if let Some(size) = size {
        let before = get_setting(conn, KEY_TEXT_SIZE)
            .map_err(write_err)?
            .map_or(TextSize::Normal, |text| TextSize::from_setting(&text));
        if before != size {
            put(conn, KEY_TEXT_SIZE, size.as_setting()).map_err(write_err)?;
            out.appearance = true;
        }
    }

    // Сочетания: принимает система, поэтому здесь только список отличий.
    for kind in ShortcutKind::ALL {
        let wanted = match &parsed.shortcuts[shortcut_slot(kind)] {
            Field::Absent => continue,
            Field::Default => kind.default_accelerator().to_string(),
            Field::Value(accelerator) => accelerator.clone(),
        };
        if stored_accelerator(conn, kind).map_err(write_err)? != wanted {
            out.shortcuts.push((kind, wanted));
        }
    }

    // Копия и доступ агента — только из безопасного файла.
    let wants_mirror = !matches!(parsed.mirror_folder, Field::Absent);
    let wants_agent = !matches!(
        (&parsed.agent_enabled, &parsed.agent_mode, &parsed.chats),
        (Field::Absent, Field::Absent, Field::Absent)
    );
    if input.trusted {
        apply_mirror(conn, input, default_mirror, &mut out)?;
        apply_agent(conn, input, &mut out)?;
    } else {
        if wants_mirror {
            out.issues.push(unsafe_ignored("mirror.folder"));
        }
        if wants_agent {
            out.issues.push(unsafe_ignored("agent_access"));
        }
    }
    Ok(out)
}

fn unsafe_ignored(key: &str) -> KeyIssue {
    KeyIssue {
        key: key.to_string(),
        kind: KeyIssueKind::UnsafeIgnored,
    }
}

pub(crate) fn shortcut_slot(kind: ShortcutKind) -> usize {
    match kind {
        ShortcutKind::Capture => 0,
        ShortcutKind::Clipboard => 1,
    }
}

fn apply_mirror(
    conn: &Transaction<'_>,
    input: &ApplyInput,
    default_mirror: Option<&Path>,
    out: &mut ApplyOutcome,
) -> Result<(), StoreError> {
    let (current, _) = effective_mirror_dir(conn, default_mirror).map_err(write_err)?;
    let custom = get_setting(conn, KEY_MIRROR_DIR).map_err(write_err)?;
    match &input.parsed.mirror_folder {
        Field::Absent => {}
        Field::Default => {
            if custom.is_some() {
                conn.execute("DELETE FROM app_setting WHERE key = ?1", [KEY_MIRROR_DIR])
                    .map_err(write_err)?;
                mark_all_dirty(conn).map_err(write_err)?;
                out.mirror = true;
            }
        }
        Field::Value(path) => match check_folder(path, &input.data_dir) {
            Err(kind) => out.issues.push(KeyIssue {
                key: "mirror.folder".into(),
                kind,
            }),
            Ok(folder) => {
                if current.as_deref() != Some(folder.as_path()) {
                    put(conn, KEY_MIRROR_DIR, path).map_err(write_err)?;
                    mark_all_dirty(conn).map_err(write_err)?;
                    out.mirror = true;
                }
            }
        },
    }
    Ok(())
}

fn apply_agent(
    conn: &Transaction<'_>,
    input: &ApplyInput,
    out: &mut ApplyOutcome,
) -> Result<(), StoreError> {
    let parsed = &input.parsed;
    let before = agent_state(conn);
    let mut after = before.clone();
    match parsed.agent_enabled {
        Field::Absent => {}
        Field::Default => after.enabled = false,
        Field::Value(on) => after.enabled = on,
    }
    match parsed.agent_mode {
        Field::Absent => {}
        Field::Default => after.mode = AgentMode::Read,
        Field::Value(mode) => after.mode = mode,
    }
    match &parsed.chats {
        Field::Absent => {}
        Field::Default => after.chats = Vec::new(),
        Field::Value(refs) => {
            let (ids, issues) = resolve_chats(&load_chats(conn)?, refs);
            out.issues.extend(issues);
            after.chats = ids;
        }
    }
    if same_access(&before, &after) {
        return Ok(());
    }
    put(conn, KEY_ENABLED, if after.enabled { "1" } else { "0" }).map_err(write_err)?;
    put(conn, KEY_MODE, after.mode.setting()).map_err(write_err)?;
    let chats = serde_json::to_string(&after.chats)
        .map_err(|error| StoreError::Internal(error.to_string()))?;
    put(conn, KEY_CHATS, &chats).map_err(write_err)?;
    // Без названий чатов и путей: только флаг, режим и число чатов.
    let args = serde_json::json!({
        "enabled": after.enabled,
        "mode": after.mode.setting(),
        "chats": after.chats.len(),
    })
    .to_string();
    insert_log(conn, input.at_ms, LOG_CLIENT, LOG_TOOL, &args, "ok", &[]).map_err(write_err)?;
    out.access = true;
    Ok(())
}

/// Доступ агента как есть. Повреждённое значение читается как «выключено, чатов нет».
fn agent_state(conn: &Connection) -> AgentSettings {
    load_settings(conn).unwrap_or(AgentSettings {
        enabled: false,
        mode: AgentMode::Read,
        chats: Vec::new(),
    })
}

fn same_access(a: &AgentSettings, b: &AgentSettings) -> bool {
    let set = |settings: &AgentSettings| settings.chats.iter().cloned().collect::<BTreeSet<_>>();
    a.enabled == b.enabled && a.mode == b.mode && set(a) == set(b)
}
