//! Команды главного окна слайса 3: чаты, действия с записью, корзина, настройки сайдбара и пикера. Бизнес-правил здесь нет
//! (они в `kinige-domain` и `kinige-store`); после изменений обновляется меню значка. Окну захвата эти команды не выданы.

use kinige_domain::{ApiError, ChatSummary, MessageFlag, MessageView, TrashItem};
use kinige_store::{Now, Store, StoreError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    eprintln!("kinige: ошибка хранилища: {}", error.category());
    error.code().into()
}

/// «+» у раздела «Чаты»: новый чат в конце раздела; без эмодзи — 💬.
#[tauri::command(async)]
pub fn create_chat<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    title: String,
    icon: Option<String>,
) -> ApiResult<ChatSummary> {
    let chat = store
        .create_chat(&title, icon.as_deref(), &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

#[tauri::command(async)]
pub fn rename_chat<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    title: String,
) -> ApiResult<ChatSummary> {
    let chat = store
        .rename_chat(&id, &title, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

#[tauri::command(async)]
pub fn set_chat_icon<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    icon: String,
) -> ApiResult<ChatSummary> {
    let chat = store
        .set_chat_icon(&id, &icon, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

#[tauri::command(async)]
pub fn set_chat_pinned(
    store: State<'_, Store>,
    id: String,
    pinned: bool,
) -> ApiResult<ChatSummary> {
    store
        .set_chat_pinned(&id, pinned, &Now::system())
        .map_err(api)
}

#[tauri::command(async)]
pub fn set_chat_archived<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    archived: bool,
) -> ApiResult<ChatSummary> {
    let chat = store
        .set_chat_archived(&id, archived, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

/// «Удалить»: запись уходит в корзину. Без оптимистичного показа: ответ приходит после фиксации.
#[tauri::command(async)]
pub fn delete_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> ApiResult<()> {
    store.delete_message(&id, &Now::system()).map_err(api)?;
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn restore_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> ApiResult<()> {
    store.restore_message(&id, &Now::system()).map_err(api)?;
    crate::tray::refresh(&app);
    Ok(())
}

/// Правка текста в плашке: ответ — запись, как она теперь выглядит; старая версия сохранена ревизией.
#[tauri::command(async)]
pub fn edit_message(store: State<'_, Store>, id: String, body: String) -> ApiResult<MessageView> {
    store.edit_message(&id, &body, &Now::system()).map_err(api)
}

/// Щелчок по `- [ ]` внутри записи: пункт `index` (с нуля) получает состояние `checked`.
#[tauri::command(async)]
pub fn toggle_task_item(
    store: State<'_, Store>,
    id: String,
    index: usize,
    checked: bool,
) -> ApiResult<MessageView> {
    store
        .toggle_task_item(&id, index, checked, &Now::system())
        .map_err(api)
}

/// Задача, выполнено, избранное.
#[tauri::command(async)]
pub fn set_message_flag(
    store: State<'_, Store>,
    id: String,
    flag: MessageFlag,
    on: bool,
) -> ApiResult<MessageView> {
    store
        .set_message_flag(&id, flag, on, &Now::system())
        .map_err(api)
}

/// «Изменить дату»: `YYYY-MM-DD`; будущие даты разрешены.
#[tauri::command(async)]
pub fn set_message_date(
    store: State<'_, Store>,
    id: String,
    journal_date: String,
) -> ApiResult<MessageView> {
    store
        .set_message_date(&id, &journal_date, &Now::system())
        .map_err(api)
}

/// «Переместить в чат»: запись переезжает в другой чат (день и текст не меняются, ревизии нет).
#[tauri::command(async)]
pub fn move_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    chat_id: String,
) -> ApiResult<MessageView> {
    let view = store
        .move_message(&id, &chat_id, &Now::system())
        .map_err(api)?;
    // «Последние чаты» в меню значка зависят от последней записи чата.
    crate::tray::refresh(&app);
    Ok(view)
}

#[tauri::command(async)]
pub fn list_trash(store: State<'_, Store>) -> ApiResult<Vec<TrashItem>> {
    store.list_trash(&Now::system()).map_err(api)
}

/// «Удалить навсегда» у записи в корзине (фронтенд спрашивает подтверждение).
#[tauri::command(async)]
pub fn purge_message(store: State<'_, Store>, id: String) -> ApiResult<()> {
    store.purge_message(&id, &Now::system()).map_err(api)
}

#[tauri::command(async)]
pub fn set_sidebar_collapsed(store: State<'_, Store>, collapsed: bool) -> ApiResult<()> {
    store.set_sidebar_collapsed(collapsed).map_err(api)
}

/// Выбранный в пикере эмодзи — первым в «недавних»; ответ — список целиком.
#[tauri::command(async)]
pub fn add_recent_emoji(store: State<'_, Store>, emoji: String) -> ApiResult<Vec<String>> {
    store.add_recent_emoji(&emoji).map_err(api)
}
