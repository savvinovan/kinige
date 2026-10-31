//! Типы файла настроек `settings.json` для окна «Файл настроек» (слайс 4b, ADR-0010): путь, что не так с файлом, ошибки
//! ключей и когда раздел менялся через файл. Без IO. TS-типы генерируются в `web/src/api/generated/` (ADR-0002).

use serde::Serialize;
use ts_rs::TS;

/// Состояние файла для окна.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SettingsFileState.ts")]
pub struct SettingsFileState {
    /// Абсолютный путь файла: показывается и копируется («Копировать путь»).
    pub path: String,
    /// Файл не разобран или не прочитан; ни одна настройка из него не применена.
    pub problem: Option<FileProblem>,
    /// Почему файл небезопасен: ключи `agent_access.*` и `mirror.folder` из него не применяются.
    pub unsafe_reason: Option<UnsafeReason>,
    /// Ошибки отдельных ключей: остальные ключи применены.
    pub issues: Vec<KeyIssue>,
    /// Неизвестные ключи: предупреждение, в файле они сохраняются.
    pub unknown_keys: Vec<String>,
    /// Сколько неизвестных ключей не поместилось в список («и ещё N»).
    pub unknown_more: u32,
    /// Когда раздел менялся через файл.
    pub changed: FileChanges,
}

/// Файл целиком не годится.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FileProblem.ts")]
pub struct FileProblem {
    pub kind: FileProblemKind,
    /// Строка и столбец (с единицы) для `parse`.
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "FileProblemKind.ts")]
pub enum FileProblemKind {
    /// Не JSON.
    Parse,
    /// JSON, но не объект.
    NotObject,
    /// Не UTF-8.
    NotUtf8,
    /// Начинается с BOM.
    Bom,
    /// Больше 1 МиБ.
    TooLarge,
    /// Не удалось прочитать или записать (права, диск).
    Io,
}

/// Почему файл небезопасен (ADR-0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "UnsafeReason.ts")]
pub enum UnsafeReason {
    /// Файл — символическая ссылка.
    Symlink,
    /// Не обычный файл (канал, устройство).
    NotRegular,
    /// Владелец не текущий пользователь.
    Owner,
    /// Файл доступен на запись группе или всем.
    Permissions,
    /// Каталог данных доступен на запись группе или всем (или это ссылка).
    Directory,
}

/// Ошибка одного ключа (или элемента массива): `key` — путь вида `agent_access.chats[2]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "KeyIssue.ts")]
pub struct KeyIssue {
    pub key: String,
    pub kind: KeyIssueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "KeyIssueKind.ts")]
pub enum KeyIssueKind {
    /// Неверный тип значения.
    Type,
    /// Недопустимое значение.
    Value,
    /// Сочетание записано неверно или зарезервировано.
    ShortcutInvalid,
    /// Сочетание уже назначено другому действию.
    ShortcutDuplicate,
    /// Система не приняла сочетание.
    ShortcutTaken,
    FolderNotAbsolute,
    FolderMissing,
    /// Папка внутри каталога данных.
    FolderInsideData,
    /// Файл небезопасен: ключ не применён.
    UnsafeIgnored,
    ChatNotFound,
    ChatAmbiguous,
    /// Чат исключён из AI.
    ChatExcluded,
}

/// Когда раздел менялся через файл (UTC, мс); `null` — не менялся в этом запуске.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FileChanges.ts")]
pub struct FileChanges {
    #[ts(type = "number | null")]
    pub appearance: Option<i64>,
    #[ts(type = "number | null")]
    pub shortcuts: Option<i64>,
    #[ts(type = "number | null")]
    pub mirror: Option<i64>,
    #[ts(type = "number | null")]
    pub access: Option<i64>,
}
