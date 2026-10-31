//! Окно «Настройки» (слайс 4b, ADR-0010): отдельное окно Tauri, меню приложения (⌘,) и значка, события окнам, мост к
//! глобальным сочетаниям для файла настроек. Правила файла живут в `kinige-store`; здесь только окно и платформа.

use crate::capture::{MAIN_LABEL, show_main};
use crate::hotkeys::Hotkeys;
use kinige_domain::{ApiError, ErrorCode, SettingsFileState, ShortcutKind};
use kinige_store::{SettingsFile, SettingsFileOptions, ShortcutApplier, ShortcutRefusal, Store};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewUrl, WebviewWindowBuilder};

pub const SETTINGS_LABEL: &str = "settings";
/// Событие окнам `main` и `settings`: настройки изменились (окном, другим окном или файлом). Без данных: окна читают сами.
pub const SETTINGS_CHANGED: &str = "settings-changed";
/// Событие главному окну: открыть запись в ленте (из журнала обращений в окне настроек).
pub const OPEN_MESSAGE: &str = "open-message";

/// Размеры окна: по умолчанию и наименьший (логические px). Совпадают с токенами `layout.settings.window-*` (проверяет
/// `tests/config.rs`): страница рассчитана на ровно такие окна.
pub const WIDTH: f64 = 860.0;
pub const HEIGHT: f64 = 600.0;
pub const MIN_WIDTH: f64 = 640.0;
pub const MIN_HEIGHT: f64 = 440.0;
/// Идентификатор пункта «Настройки…» в меню приложения.
pub const ID_APP_SETTINGS: &str = "app-settings";
/// Не дать двум быстрым нажатиям создать два окна.
static OPEN_LOCK: Mutex<()> = Mutex::new(());

/// Пункт «Настройки…» меню приложения: включается, когда журнал открыт.
pub struct AppSettingsItem<R: Runtime>(pub tauri::menu::MenuItem<R>);

/// Меню приложения на macOS: стандартное (в нём «Правка» — без неё в WKWebView не работают ⌘C и ⌘V; меню по умолчанию
/// собирает `tauri::menu::Menu::default`, проверено по исходникам tauri 2.12.4) и в меню приложения пункт «Настройки…» с ⌘,.
/// Пока журнал не открыт, пункт недоступен (`set_available`).
#[cfg(target_os = "macos")]
pub fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem};
    let menu = Menu::default(app)?;
    let labels = crate::tray::labels(crate::commands::get_language());
    let item = MenuItem::with_id(
        app,
        ID_APP_SETTINGS,
        &labels.settings,
        false,
        Some("CmdOrCtrl+,"),
    )?;
    // Первое подменю — меню приложения: «О программе», разделитель, … Пункт ставится после разделителя.
    if let Some(MenuItemKind::Submenu(first)) = menu.items()?.into_iter().next() {
        first.insert(&item, 2)?;
        first.insert(&PredefinedMenuItem::separator(app)?, 3)?;
    }
    app.manage(AppSettingsItem(item));
    Ok(menu)
}

/// Показывает окно настроек (создаёт при первом открытии). Пока журнал не открыт, ничего не делает. Второй вызов поднимает
/// то же окно. Из фона ставит обычную политику активации, как `show_main`.
pub fn open<R: Runtime>(app: &AppHandle<R>) {
    if app.try_state::<Store>().is_none() {
        return;
    }
    let _guard = OPEN_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let title = crate::tray::window_title(crate::commands::get_language());
    let built = WebviewWindowBuilder::new(
        app,
        SETTINGS_LABEL,
        WebviewUrl::App("index.html#settings".into()),
    )
    .title(title)
    .inner_size(WIDTH, HEIGHT)
    .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
    .resizable(true)
    .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(error) => log::error!("окно настроек не создано: {error}"),
    }
}

/// Окно настроек закрыто (окно уничтожено): режим записи сочетания снимается, иначе хоткеи остались бы выключенными.
pub fn on_destroyed<R: Runtime>(app: &AppHandle<R>) {
    if let Some(hotkeys) = app.try_state::<Hotkeys>() {
        hotkeys.set_recording(false);
    }
}

/// Сообщает обоим окнам, что настройки изменились. Не блокирует: вызывается и из потока файла.
pub fn notify_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit_to(MAIN_LABEL, SETTINGS_CHANGED, ());
    let _ = app.emit_to(SETTINGS_LABEL, SETTINGS_CHANGED, ());
}

/// «Настройки…» доступны, когда журнал открыт (не в режиме восстановления и не при «база новее»).
pub fn set_available<R: Runtime>(app: &AppHandle<R>, available: bool) {
    if let Some(item) = app.try_state::<AppSettingsItem<R>>() {
        let _ = item.0.set_enabled(available);
    }
}

/// Хоткеи для файла настроек: те же шаги, что при смене сочетания в окне (проверка, регистрация, сохранение).
struct HotkeyBridge<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> ShortcutApplier for HotkeyBridge<R> {
    fn apply(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ShortcutRefusal> {
        let Some(hotkeys) = self.app.try_state::<Hotkeys>() else {
            return Err(ShortcutRefusal::Invalid);
        };
        hotkeys
            .change(store, kind, accelerator)
            .map_err(|code| match code {
                ErrorCode::ShortcutTaken => ShortcutRefusal::Taken,
                ErrorCode::ShortcutDuplicate => ShortcutRefusal::Duplicate,
                _ => ShortcutRefusal::Invalid,
            })
    }
}

/// Запускает поток файла настроек. Файл применяется синхронно, до регистрации сочетаний и до слушателя MCP.
pub fn start_file<R: Runtime>(
    app: &AppHandle<R>,
    store: &Store,
    data_dir: PathBuf,
) -> SettingsFile {
    let handle = app.clone();
    let mut options = SettingsFileOptions::new(Arc::new(move || notify_changed(&handle)));
    options.shortcuts = Some(Arc::new(HotkeyBridge { app: app.clone() }));
    SettingsFile::start(store.clone(), data_dir, options)
}

/// Открыть окно настроек из главного окна (профиль → «Настройки»).
#[tauri::command]
pub fn open_settings<R: Runtime>(app: AppHandle<R>) {
    open(&app);
}

/// Путь файла, его состояние и ошибки ключей. Команда не `async`, то есть идёт на главном потоке: она только читает
/// состояние под коротким замком и никогда не ждёт поток файла. Поток файла, наоборот, ждёт главный поток (регистрация
/// сочетаний в плагине идёт через `run_on_main_thread`), поэтому ни одна команда главного потока не должна ждать его —
/// иначе получилась бы взаимная блокировка (`open_settings`, `shortcut_recording` и эта команда не ждут).
#[tauri::command]
pub fn get_settings_file(file: State<'_, SettingsFile>) -> SettingsFileState {
    file.state()
}

/// Открыть запись из журнала обращений в главном окне. Запись проверяется здесь: удалена навсегда или в корзине —
/// `message_not_found`, окно покажет «Запись удалена». Содержимое записи окну настроек не отдаётся.
#[tauri::command(async)]
pub fn settings_open_entry<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> Result<(), ApiError> {
    let chat_id = store.live_message_chat(&id).map_err(|error| {
        log::warn!("запись для окна настроек не найдена: {}", error.category());
        ApiError::from(error.code())
    })?;
    show_main(&app);
    let _ = app.emit_to(
        MAIN_LABEL,
        OPEN_MESSAGE,
        serde_json::json!({"chatId": chat_id, "messageId": id}),
    );
    Ok(())
}
