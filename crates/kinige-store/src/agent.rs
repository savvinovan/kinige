//! Доступ ИИ-агентов через MCP (ADR-0009, слайс 4a): настройки доступа и единая точка входа для вызова инструмента.
//!
//! Агент ничего не получает мимо [`Store::agent_call`]: выключатель и режим проверяются при каждом вызове, видимое множество
//! чатов (`VISIBLE`) входит в каждый оператор чтения и в транзакцию записи, каждый вызов оставляет ровно одну строку
//! журнала (`agent_log`), а текст записей в журнал не попадает. Клиентам и мосту не доверяем.

use crate::agent_log::{insert_log, log_connection, log_only};
use crate::agent_read::{ReadCtx, ReadFail};
use crate::agent_write::run_write_job;
use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::{Now, Store, StoreError};
use kinige_domain::AgentMode;
use kinige_domain::agent::{AgentTool, ToolArgs, args_json, clean_client, parse_args};
use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Map, Value};

pub(crate) const KEY_ENABLED: &str = "mcp_enabled";
pub(crate) const KEY_MODE: &str = "mcp_mode";
pub(crate) const KEY_CHATS: &str = "mcp_chats";

/// Чат (`c`) виден агенту ⇔ он отмечен в `mcp_chats`, не исключён из AI и не удалён. Архив виден. Предикат стоит в каждом
/// операторе чтения агента и внутри транзакции записи: между операторами одного вызова множество может измениться, каждый
/// оператор проверен сам. Ключа нет → `json_each(NULL)` пуст → видимых чатов нет. Id, которых нет в базе, ни с чем не
/// совпадают.
pub(crate) const VISIBLE: &str = "c.deleted_at IS NULL AND c.ai_excluded = 0 AND c.id IN \
     (SELECT j.value FROM json_each((SELECT value FROM app_setting WHERE key = 'mcp_chats')) j)";

/// Настройки доступа агента (`app_setting`, не синхронизируются).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSettings {
    pub enabled: bool,
    pub mode: AgentMode,
    pub chats: Vec<String>,
}

/// Настройки из базы. Повреждённый `mcp_chats` (не массив строк) — ошибка без данных; ключа нет — пустое множество.
pub(crate) fn load_settings(conn: &Connection) -> Result<AgentSettings, StoreError> {
    let enabled = get_setting(conn, KEY_ENABLED).map_err(read_err)?;
    let mode = get_setting(conn, KEY_MODE).map_err(read_err)?;
    let chats = match get_setting(conn, KEY_CHATS).map_err(read_err)? {
        None => Vec::new(),
        Some(text) => serde_json::from_str::<Vec<String>>(&text)
            .map_err(|_| StoreError::Internal("значение mcp_chats повреждено".into()))?,
    };
    Ok(AgentSettings {
        enabled: enabled.as_deref() == Some("1"),
        mode: mode
            .as_deref()
            .map_or(AgentMode::Read, AgentMode::from_setting),
        chats,
    })
}

/// Вызов инструмента от агента. `client` — слова клиента (`clientInfo.name`), в журнал идёт очищенным.
pub struct AgentRequest<'a> {
    pub client: &'a str,
    pub tool: &'a str,
    pub arguments: Option<&'a Map<String, Value>>,
    /// Название Inbox на языке интерфейса (в базе хранится пустое).
    pub inbox_title: &'a str,
}

/// Ответ агенту: текст с JSON и признак «данные изменились» (слой Tauri тихо обновляет ленту и трей).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReply {
    pub text: String,
    pub changed: bool,
}

/// Отказ агенту. Закрытый чат, `ai_excluded`, удалённый, несуществующий и запись в корзине дают один и тот же `NotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    Disabled,
    ReadOnly,
    NotFound,
    NoAgentEntry,
    NotATask,
    Invalid(String),
    /// База повреждена: данных нет.
    Unavailable,
    /// Остальное; подробности только в категории лога.
    Internal,
}

impl AgentError {
    /// Код ошибки для агента.
    pub fn code(&self) -> &'static str {
        match self {
            AgentError::Disabled => "mcp_disabled",
            AgentError::ReadOnly => "mcp_read_only",
            AgentError::NotFound => "not_found",
            AgentError::NoAgentEntry => "no_agent_entry",
            AgentError::NotATask => "not_a_task",
            AgentError::Invalid(_) => "invalid_argument",
            AgentError::Unavailable => "journal_unavailable",
            AgentError::Internal => "internal_error",
        }
    }

    /// Исход для журнала обращений.
    pub(crate) fn outcome(&self) -> &'static str {
        match self {
            AgentError::Disabled => "disabled",
            AgentError::ReadOnly => "read_only",
            AgentError::NotFound => "not_found",
            AgentError::NoAgentEntry => "no_agent_entry",
            AgentError::NotATask => "not_a_task",
            AgentError::Invalid(_) => "invalid_argument",
            AgentError::Unavailable | AgentError::Internal => "error",
        }
    }
}

impl From<StoreError> for AgentError {
    fn from(error: StoreError) -> Self {
        log::warn!("агент: {}", error.category());
        match error {
            StoreError::Corrupt(_) => AgentError::Unavailable,
            _ => AgentError::Internal,
        }
    }
}

impl Store {
    /// Единая точка входа вызова инструмента. Порядок: инструмент известен → доступ включён → режим → аргументы → выполнение.
    /// Каждый путь оставляет ровно одну строку журнала; сбой записи журнала — ошибка агенту, данных он не получает.
    pub fn agent_call(
        &self,
        request: &AgentRequest<'_>,
        now: &Now,
    ) -> Result<AgentReply, AgentError> {
        let client = clean_client(request.client);
        let Some(tool) = AgentTool::from_name(request.tool) else {
            self.agent_refuse(
                &client,
                "unknown",
                &AgentError::Invalid("unknown tool".into()),
                now,
            )?;
            return Err(AgentError::Invalid("unknown tool".into()));
        };
        let settings = self.read(load_settings)?;
        if !settings.enabled {
            return self.agent_refuse(&client, tool.name(), &AgentError::Disabled, now);
        }
        if tool.writes() && settings.mode != AgentMode::ReadWrite {
            return self.agent_refuse(&client, tool.name(), &AgentError::ReadOnly, now);
        }
        let args = match parse_args(tool, request.arguments) {
            Ok(args) => args,
            Err(why) => {
                return self.agent_refuse(&client, tool.name(), &AgentError::Invalid(why.0), now);
            }
        };
        let call = Call {
            client,
            tool,
            args_json: args_json(&args),
            inbox_title: request.inbox_title.to_string(),
            now: now.clone(),
        };
        let result = if tool.writes() {
            self.agent_write(&call, args)
        } else {
            self.agent_read(&call, &args)
        };
        if let Err(AgentError::Internal | AgentError::Unavailable) = &result {
            // Сбой выполнения: данных нет, но обращение остаётся в журнале, если база ещё принимает записи.
            let _ = log_only(
                self,
                &call.client,
                tool.name(),
                &call.args_json,
                "error",
                now,
            );
        }
        result
    }

    /// Отказ без выполнения: одна строка журнала, затем ошибка агенту.
    fn agent_refuse(
        &self,
        client: &str,
        tool: &str,
        error: &AgentError,
        now: &Now,
    ) -> Result<AgentReply, AgentError> {
        log_only(self, client, tool, "{}", error.outcome(), now)?;
        Err(error.clone())
    }

    /// Чтение: выборка на соединении чтения, затем транзакция писателя со строкой журнала, затем ответ. Сбой журнала —
    /// ошибка, а не данные.
    fn agent_read(&self, call: &Call, args: &ToolArgs) -> Result<AgentReply, AgentError> {
        let ctx = ReadCtx::new(&call.inbox_title, &call.now);
        let read = match self.read_with_heal(&ctx, args) {
            Ok(read) => read,
            Err(ReadFail::NotFound) => {
                return self.agent_refuse(
                    &call.client,
                    call.tool.name(),
                    &AgentError::NotFound,
                    &call.now,
                );
            }
            Err(ReadFail::Store(error)) => return Err(error.into()),
        };
        let job = call.clone();
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let (tool, ms) = (job.tool.name(), job.now.ms);
            insert_log(&tx, ms, &job.client, tool, &job.args_json, "ok", &read.ids)
                .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(AgentReply {
                text: read.text,
                changed: false,
            })
        })
        .map_err(AgentError::from)
    }

    /// Запись: одна транзакция писателя — проверка доступа и режима, видимость чата, изменение данных и строка журнала.
    fn agent_write(&self, call: &Call, args: ToolArgs) -> Result<AgentReply, AgentError> {
        let call = call.clone();
        let reply = self.write(move |conn| run_write_job(conn, &call, &args))??;
        self.wake_mirror();
        Ok(reply)
    }

    /// Подключение клиента (`initialize`): одна строка журнала с очищенным именем клиента. `reconnect` — соединение открыто
    /// мостом после обрыва: строка `reconnect`, а прежний сеанс этого клиента, если он остался открытым (приложение
    /// перезапускалось), закрывается строкой «отключение» в той же транзакции. Немых путей нет: каждое соединение — строка.
    pub fn agent_connected(
        &self,
        client: &str,
        reconnect: bool,
        now: &Now,
    ) -> Result<(), StoreError> {
        log_connection(self, &clean_client(client), reconnect, now).map_err(|error| {
            log::warn!("агент: журнал подключения: {}", error.category());
            StoreError::Write("журнал подключения".into())
        })
    }

    /// Отключение клиента: одна строка журнала.
    pub fn agent_disconnected(&self, client: &str, now: &Now) -> Result<(), StoreError> {
        log_only(self, &clean_client(client), "disconnect", "{}", "ok", now).map_err(|error| {
            log::warn!("агент: журнал отключения: {}", error.code());
            StoreError::Write("журнал отключения".into())
        })
    }
}

/// Один вызов инструмента после разбора аргументов (всё владеемое: уходит на поток-писатель).
#[derive(Clone)]
pub(crate) struct Call {
    pub(crate) client: String,
    pub(crate) tool: AgentTool,
    pub(crate) args_json: String,
    pub(crate) inbox_title: String,
    pub(crate) now: Now,
}
