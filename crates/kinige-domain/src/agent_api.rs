//! Типы окна «Доступ ИИ-агентов» (слайс 4a, ADR-0009): настройки доступа, состояние сокета, журнал обращений.
//! TS-типы генерируются в `web/src/api/generated/` (ADR-0002).

use crate::MessageView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Режим доступа агента: один переключатель на все видимые чаты (решение 3, 28.10.2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "AgentMode.ts")]
pub enum AgentMode {
    /// Только чтение: запись и отметка задач отказывают.
    Read,
    /// Чтение и запись в любой видимый чат.
    ReadWrite,
}

impl AgentMode {
    /// Значение в `app_setting`. Незнакомое значение — «только чтение»: безопаснее.
    pub fn setting(self) -> &'static str {
        match self {
            AgentMode::Read => "read",
            AgentMode::ReadWrite => "read_write",
        }
    }

    pub fn from_setting(text: &str) -> AgentMode {
        if text == "read_write" {
            AgentMode::ReadWrite
        } else {
            AgentMode::Read
        }
    }
}

/// Чат в списке «видно агенту».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentChatChoice.ts")]
pub struct AgentChatChoice {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_inbox: bool,
    #[ts(type = "number")]
    pub count: i64,
    /// Исключён из AI (`ai_excluded`): строка неактивна, агент его не видит никогда.
    pub ai_excluded: bool,
    /// Отмечен «видно агенту».
    pub visible: bool,
}

/// Состояние сокета MCP, фрагменты подключения и путь к мосту. Заполняет слой Tauri.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentSocket.ts")]
pub struct AgentSocket {
    /// Сокет слушает.
    pub listening: bool,
    /// Почему не слушает: `dir_unsafe`, `second_instance`, `bind_failed`, `bridge_missing`.
    pub error: Option<String>,
    /// Абсолютный путь моста (`kinige-mcp`) рядом с приложением.
    pub bridge_path: Option<String>,
    pub socket_path: Option<String>,
    /// Фрагмент `claude_desktop_config.json`.
    pub desktop_config: Option<String>,
    /// Команда `claude mcp add …`.
    pub code_command: Option<String>,
}

/// Окно «Доступ ИИ-агентов».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentAccess.ts")]
pub struct AgentAccess {
    pub enabled: bool,
    pub mode: AgentMode,
    pub chats: Vec<AgentChatChoice>,
    pub socket: AgentSocket,
}

/// Строка журнала обращений: факт обращения без содержимого записей.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogRow.ts")]
pub struct AgentLogRow {
    #[ts(type = "number")]
    pub id: i64,
    /// UTC, мс.
    #[ts(type = "number")]
    pub at: i64,
    /// Имя клиента с его слов: не проверено.
    pub client: String,
    /// Имя инструмента, `connect` или `disconnect`.
    pub tool: String,
    /// Поисковый запрос (до 128 знаков).
    pub query: Option<String>,
    /// `ok`, `disabled`, `read_only`, `not_found`, `no_agent_entry`, `not_a_task`, `invalid_argument`, `error`.
    pub outcome: String,
    /// Сколько записей затронуло обращение. Пишется вместе со строкой и не меняется после «Удалить навсегда» (раскрытие
    /// покажет меньше записей, окно скажет, сколько удалено).
    #[ts(type = "number")]
    pub count: i64,
}

/// Страница журнала, новые сверху.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogPage.ts")]
pub struct AgentLogPage {
    pub rows: Vec<AgentLogRow>,
    /// `id` последней строки страницы, если есть ещё более старые.
    #[ts(type = "number | null")]
    pub older: Option<i64>,
}

/// Запись, которую затронуло обращение: сама запись или `None`, если она удалена навсегда.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogEntry.ts")]
pub struct AgentLogEntry {
    pub message_id: String,
    pub message: Option<MessageView>,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
}
