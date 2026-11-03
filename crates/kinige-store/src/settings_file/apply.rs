//! Applying a parsed file to the database (ADR-0010): one writer transaction for everything except the shortcuts (the
//! system accepts them and the Tauri layer registers them). The same transaction holds the "before / after" comparison of
//! the agent access, the `mcp_*` keys, the file hash and the access log row.

use super::parse::{ChatRef, Field, MAX_ISSUES, Parsed};
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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Hash of the last synchronised file (`app_setting`, not written to the file).
pub(crate) const KEY_FILE_HASH: &str = "settings_file_hash";
/// Signature of the access log row when the access is changed from the file: by `tool`, not by `client` (an MCP client names itself).
pub const LOG_CLIENT: &str = "settings.json";
pub const LOG_TOOL: &str = "settings_file";
/// Inbox title in the file: in the database it is empty and the interface translates it by language; the file does not depend on the language.
pub(crate) const INBOX_FILE_TITLE: &str = "Inbox";

/// A chat to match against an `agent_access.chats` element.
#[derive(Debug, Clone)]
pub(crate) struct ChatInfo {
    pub id: String,
    pub title: String,
    pub ai_excluded: bool,
}

/// Live chats (the archive is included). Inbox is titled `Inbox`.
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

/// Elements of the chats array -> ids that can be enabled. An unsuitable element gives an error for that element and is
/// not enabled; errors only narrow the access. `id` wins over the title: a nonexistent `id` is an error, with no fallback to the title.
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
        let issue = match found {
            Ok(chat) if chat.ai_excluded => Some(KeyIssueKind::ChatExcluded),
            Ok(chat) => {
                if !ids.contains(&chat.id) {
                    ids.push(chat.id.clone());
                }
                None
            }
            Err(kind) => Some(kind),
        };
        // A cap, as in the parser: an array of thousands of bad elements does not bloat the window.
        if let Some(kind) = issue
            && issues.len() < MAX_ISSUES
        {
            issues.push(KeyIssue { key, kind });
        }
    }
    (ids, issues)
}

/// The copy folder from the file: an absolute, existing folder outside the data directory.
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

/// What has to be applied.
pub(crate) struct ApplyInput {
    pub parsed: Parsed,
    /// The file is safe: the `agent_access.*` and `mirror.folder` keys are applied.
    pub trusted: bool,
    pub data_dir: PathBuf,
    /// SHA-256 of the bytes read, hex.
    pub hash: String,
    pub at_ms: i64,
}

/// What changed and what still has to be done after the commit.
#[derive(Debug, Default)]
pub(crate) struct ApplyOutcome {
    pub issues: Vec<KeyIssue>,
    pub appearance: bool,
    pub mirror: bool,
    pub access: bool,
    /// Shortcuts whose value in the file differs from the current one: the system (the Tauri layer) accepts and saves them.
    pub shortcuts: Vec<(ShortcutKind, String)>,
}

impl Store {
    /// Applies the file in one transaction. A write error means nothing is applied (the hash is not moved, the file is applied again).
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

    // Appearance.
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

    // Shortcuts: the system accepts them, so only the list of differences is built here.
    for kind in ShortcutKind::ALL {
        let Some(field) = parsed.shortcuts.get(shortcut_slot(kind)) else {
            continue;
        };
        let wanted = match field {
            Field::Absent => continue,
            Field::Default => kind.default_accelerator().to_string(),
            Field::Value(accelerator) => accelerator.clone(),
        };
        if stored_accelerator(conn, kind).map_err(write_err)? != wanted {
            out.shortcuts.push((kind, wanted));
        }
    }

    // The copy and the agent access come only from a safe file.
    let wants_mirror = !matches!(parsed.mirror_folder, Field::Absent);
    let wants_agent = !matches!(
        (&parsed.agent_enabled, &parsed.agent_mode, &parsed.chats),
        (Field::Absent, Field::Absent, Field::Absent)
    );
    if input.trusted {
        apply_mirror(conn, input, default_mirror, &mut out)?;
        // Windows has no MCP server (slice 10a) and no owner and mode checks of the file (ADR-0010, Windows note), so the
        // file must not store AI-agent access there: it would switch access on, with write access, at the upgrade that
        // brings MCP to Windows, without the warning. The key is reported and nothing is written.
        if cfg!(unix) {
            apply_agent(conn, input, &mut out)?;
        } else if wants_agent {
            out.issues.push(KeyIssue {
                key: "agent_access".to_string(),
                kind: KeyIssueKind::Unsupported,
            });
        }
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
    let mut before = agent_state(conn);
    // The agent does not see chats that are gone or excluded from AI: the "before / after" comparison ignores them,
    // otherwise an extra log row would appear without any change of access.
    let visible: HashSet<String> = load_chats(conn)?
        .into_iter()
        .filter(|chat| !chat.ai_excluded)
        .map(|chat| chat.id)
        .collect();
    before.chats.retain(|id| visible.contains(id));
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
            let room = MAX_ISSUES.saturating_sub(out.issues.len());
            out.issues.extend(issues.into_iter().take(room));
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
    // No chat titles and no paths: only the flag, the mode and the number of chats.
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

/// The agent access as it is. A corrupted value reads as "off, no chats".
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
