//! Типы, которыми обмениваются Rust и фронтенд (`KinigeApi`). TS-типы генерируются в `web/src/api/generated/`
//! (`cargo test`, ADR-0002). Тексты ошибок — во фронтенде: Rust возвращает только коды.

use crate::backup::BackupStatus;
use crate::lang::Language;
use crate::shortcuts::ShortcutStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Чат в сайдбаре. У Inbox хранимое имя пустое: интерфейс показывает переведённое.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatSummary.ts")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_inbox: bool,
    /// Записи чата, кроме корзины.
    #[ts(type = "number")]
    pub count: i64,
    /// Когда закреплён (мс); `null` — не закреплён. Закреплённые идут в разделе «Закреплённые» в порядке закрепления.
    #[ts(type = "number | null")]
    pub pinned_at: Option<i64>,
    /// Когда отправлен в архив (мс); `null` — не в архиве.
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
}

/// Чаты сайдбара в порядке показа и размер корзины.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatList.ts")]
pub struct ChatList {
    /// Сначала закреплённые (по времени закрепления), затем остальные и архив (по `sort_key`, при равных — по `id`).
    pub chats: Vec<ChatSummary>,
    /// Записей в корзине.
    #[ts(type = "number")]
    pub trash_count: i64,
}

/// Запись в ленте.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessageView.ts")]
pub struct MessageView {
    pub id: String,
    pub chat_id: String,
    pub body_md: String,
    /// `YYYY-MM-DD`, граница дня 04:00 (ТЗ §4.1).
    pub journal_date: String,
    /// UTC, мс.
    #[ts(type = "number")]
    pub created_at: i64,
    /// IANA-пояс устройства в момент создания.
    pub created_tz: String,
    /// Запись — задача (флажок у плашки).
    pub is_task: bool,
    /// Когда задача отмечена выполненной (мс); `null` — не выполнена.
    #[ts(type = "number | null")]
    pub task_done_at: Option<i64>,
    /// Когда добавлена в избранное (мс); `null` — не в избранном.
    #[ts(type = "number | null")]
    pub favorite_at: Option<i64>,
    /// Теги записи в нижнем регистре, без `#` (разбор — `kinige_domain::tags`).
    pub tags: Vec<String>,
    /// Одна из групп записи новее поддерживаемой версии: запись только для чтения, действия недоступны (ADR-0001 §4).
    pub read_only: bool,
    /// Кто создал запись (`core.origin`): `user`, `import_telegram`, `agent`. Лента помечает записи агента.
    pub origin: String,
}

/// Какой флаг записи меняется командой `set_message_flag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "MessageFlag.ts")]
pub enum MessageFlag {
    /// Запись — задача (флажок). Снятие обнуляет «выполнено».
    Task,
    /// Задача выполнена; только у задачи.
    TaskDone,
    /// Избранное.
    Favorite,
}

/// Запись в корзине (ТЗ §2.1): чат, дата, сколько осталось до автоудаления.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TrashItem.ts")]
pub struct TrashItem {
    pub message: MessageView,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
    /// Когда удалена (мс).
    #[ts(type = "number")]
    pub deleted_at: i64,
    /// Сколько целых дней осталось до автоудаления (неполный день считается днём).
    #[ts(type = "number")]
    pub days_left: i64,
}

/// Позиция в ленте для постраничной загрузки (keyset): `(journal_date, created_at, id)` последней загруженной записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FeedCursor.ts")]
pub struct FeedCursor {
    pub journal_date: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub id: String,
}

/// Страница ленты: записи по возрастанию `(journal_date, created_at, id)`, самая новая — последней.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessagePage.ts")]
pub struct MessagePage {
    pub messages: Vec<MessageView>,
    /// Позиция для следующей (более старой) страницы; `null` — больше записей нет.
    pub older: Option<FeedCursor>,
    /// Позиция для следующей (более новой) страницы: последняя запись этой; `null` — новее ничего нет (слайс 4).
    pub newer: Option<FeedCursor>,
    /// Сегодняшний `journal_date` (с границей дня 04:00): от него считаются метки «Сегодня» и «Вчера».
    pub today: String,
}

/// Фильтры поиска (слайс 4, §9.1): все необязательные и соединяются через «и».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "SearchFilters.ts")]
pub struct SearchFilters {
    /// Только записи этого чата; `null` — везде.
    pub chat_id: Option<String>,
    /// Только избранное.
    pub favorite: bool,
    /// Только задачи.
    pub tasks: bool,
    /// С этого `journal_date` включительно (`YYYY-MM-DD`).
    pub from: Option<String>,
    /// По этот `journal_date` включительно.
    pub to: Option<String>,
}

/// Найденная запись: сама запись, её чат и то, что показывает строка результата.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SearchHit.ts")]
pub struct SearchHit {
    pub message: MessageView,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
    /// Простой текст вокруг первого совпадения, до 200 знаков, с «…» по обрезанным краям.
    pub snippet: String,
    /// Слова записи, которые совпали с запросом, как они в ней написаны: фронтенд подсвечивает их целиком.
    pub terms: Vec<String>,
}

/// Страница результатов поиска: новые записи сверху (порядок ленты, самые новые первыми).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SearchPage.ts")]
pub struct SearchPage {
    /// Чаты, чьё название совпало с запросом; только на первой странице. У Inbox название переводит фронтенд.
    pub chats: Vec<ChatSummary>,
    pub hits: Vec<SearchHit>,
    /// Позиция для следующей страницы (последняя запись этой); `null` — больше нет.
    pub older: Option<FeedCursor>,
}

/// Размер текста (ADR-0004): «Обычный» или «Крупный» ×1,25.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "TextSize.ts")]
pub enum TextSize {
    Normal,
    Large,
}

impl TextSize {
    pub fn as_setting(self) -> &'static str {
        match self {
            TextSize::Normal => "normal",
            TextSize::Large => "large",
        }
    }

    /// Значение из `app_setting`; незнакомое — обычный размер.
    pub fn from_setting(value: &str) -> TextSize {
        if value == "large" {
            TextSize::Large
        } else {
            TextSize::Normal
        }
    }
}

/// Почему Markdown-копия сейчас не пишется.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "MirrorError.ts")]
pub enum MirrorError {
    /// Папку нельзя создать или открыть (нет доступа, диска, это файл).
    FolderUnavailable,
    /// Файл дня записать не удалось.
    WriteFailed,
}

/// Состояние Markdown-копии для окна «Настройки».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MirrorStatus.ts")]
pub struct MirrorStatus {
    /// Путь папки; `null`, если папка не выбрана и по умолчанию её нет — копия выключена.
    pub folder: Option<String>,
    /// Папка по умолчанию (`~/Documents/Kinige`), пользователь её не менял.
    pub is_default: bool,
    pub error: Option<MirrorError>,
    /// Дней, которые ждут записи в копию.
    #[ts(type = "number")]
    pub pending_days: i64,
}

/// Настройки (не синхронизируются, `app_setting`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "Settings.ts")]
pub struct Settings {
    pub text_size: TextSize,
    pub mirror: MirrorStatus,
    /// Глобальные сочетания клавиш (слайс 2): по одному на каждое действие.
    pub shortcuts: Vec<ShortcutStatus>,
    /// Сайдбар свёрнут до полосы эмодзи (слайс 3).
    pub sidebar_collapsed: bool,
    /// Недавние эмодзи пикера, самые свежие первыми (слайс 3).
    pub recent_emoji: Vec<String>,
    /// Копия файла базы (слайс 3).
    pub backup: BackupStatus,
}

/// Что сделал быстрый захват.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "CaptureAction.ts")]
pub enum CaptureAction {
    /// Создана новая запись.
    Created,
    /// Текст дописан к последней записи.
    Appended,
}

/// Ответ окну захвата: что сделано и запись, как она теперь выглядит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CaptureOutcome.ts")]
pub struct CaptureOutcome {
    pub action: CaptureAction,
    pub message: MessageView,
}

/// Всё, что нужно окну захвата при показе, одним ответом (§5.1, слайс 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CaptureState.ts")]
pub struct CaptureState {
    pub language: Language,
    pub text_size: TextSize,
    /// Сегодняшний `journal_date` (с границей дня 04:00).
    pub today: String,
    /// До трёх последних неудалённых записей Inbox за сегодня, по возрастанию: самая новая последней.
    pub context: Vec<MessageView>,
    /// Id записи, к которой ⌘Enter допишет текст; `null` — ⌘Enter создаст новую запись.
    pub append_target: Option<String>,
    /// Черновик окна захвата.
    pub draft: String,
    /// Ошибка, с которой окно открыто (сбой записи из буфера): показывается один раз.
    pub error: Option<ErrorCode>,
    /// «Недавние» эмодзи пикера (общий список с главным окном), свежие первыми.
    pub recent_emoji: Vec<String>,
}

/// Код ошибки команды; текст — во фронтенде (`error.<код>` в файлах переводов).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ErrorCode.ts")]
pub enum ErrorCode {
    EmptyMessage,
    MessageTooLarge,
    ChatNotFound,
    StoreWriteFailed,
    StoreReadFailed,
    LinkNotAllowed,
    FolderPickFailed,
    /// Сочетание не принято: нет модификатора, зарезервировано системой или записано неверно.
    ShortcutInvalid,
    /// Сочетание занято другой программой или системой.
    ShortcutTaken,
    /// Такое же сочетание уже назначено другому действию Kinige.
    ShortcutDuplicate,
    /// Запись не найдена (удалена навсегда или id неверен).
    MessageNotFound,
    /// Запись в корзине: перенос в другой чат только для живых записей, сначала «Вернуть».
    MessageInTrash,
    /// Запись с группой в неподдерживаемой версии: только чтение (ADR-0001 §4).
    ReadOnly,
    /// Inbox нельзя переименовать, архивировать и удалить.
    InboxProtected,
    /// Действие только для записи в корзине (удалить навсегда).
    NotInTrash,
    /// Дата записи не существует.
    InvalidDate,
    /// Имя чата пустое.
    ChatTitleEmpty,
    /// Имя чата длиннее 128 знаков.
    ChatTitleTooLong,
    /// Эмодзи чата не принято.
    ChatIconInvalid,
    /// Пункта задачи с таким номером в тексте нет или он уже в нужном состоянии: окно показывало устаревший текст.
    TaskItemStale,
    /// База повреждена.
    DatabaseCorrupt,
    /// База создана более новой версией приложения (§10.4): откат не поддерживается, нужно обновить Kinige.
    DatabaseTooNew,
    /// Копия не найдена или не открывается.
    BackupNotFound,
    /// Папку Markdown-копии не удалось переименовать (занята другой программой, нет прав): восстановление не начато,
    /// ничего не тронуто; закрыть папку и повторить.
    MirrorFolderBusy,
    /// Выбранный файл не экспорт Telegram (не JSON или чужой JSON): ничего не записано (слайс 4).
    ImportNotTelegram,
    /// Выбранный файл не прочитался.
    ImportFileUnreadable,
    /// Файл больше предела 1 ГиБ.
    ImportFileTooLarge,
    /// Импорт уже идёт: одновременно один.
    ImportInProgress,
    /// Разобранный экспорт забыт (окно закрыто или выбран другой файл): выбрать файл заново.
    ImportSessionExpired,
    /// Копия базы перед импортом не снялась: ни одной записи не добавлено.
    ImportBackupFailed,
    Internal,
}

/// Ошибка команды, как её получает фронтенд: `{ "code": "store_write_failed" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "ApiError.ts")]
pub struct ApiError {
    pub code: ErrorCode,
}

impl From<ErrorCode> for ApiError {
    fn from(code: ErrorCode) -> Self {
        ApiError { code }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_as_code_object() {
        assert_eq!(
            serde_json::to_string(&ApiError::from(ErrorCode::StoreWriteFailed)).unwrap(),
            r#"{"code":"store_write_failed"}"#
        );
        assert_eq!(
            serde_json::to_string(&ApiError::from(ErrorCode::LinkNotAllowed)).unwrap(),
            r#"{"code":"link_not_allowed"}"#
        );
    }

    #[test]
    fn views_serialize_in_camel_case() {
        let view = MessageView {
            id: "i".into(),
            chat_id: "c".into(),
            body_md: "b".into(),
            journal_date: "2026-10-21".into(),
            created_at: 1,
            created_tz: "UTC".into(),
            is_task: true,
            task_done_at: None,
            favorite_at: Some(5),
            tags: vec!["идея".into()],
            read_only: false,
            origin: "agent".into(),
        };
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            r#"{"id":"i","chatId":"c","bodyMd":"b","journalDate":"2026-10-21","createdAt":1,"createdTz":"UTC","isTask":true,"taskDoneAt":null,"favoriteAt":5,"tags":["идея"],"readOnly":false,"origin":"agent"}"#
        );
    }

    #[test]
    fn text_size_round_trips_through_the_setting() {
        for size in [TextSize::Normal, TextSize::Large] {
            assert_eq!(TextSize::from_setting(size.as_setting()), size);
        }
        assert_eq!(TextSize::from_setting("huge"), TextSize::Normal);
    }
}
