//! Тонкий слой Tauri: окна и команды. Бизнес-логика живёт в `kinige-domain`, хранение — в `kinige-store`.

mod actions;
pub mod capture;
mod commands;
pub mod guard;
pub mod hotkeys;
#[cfg(target_os = "macos")]
mod panel;
pub mod recovery;
pub mod tray;

use capture::{CAPTURE_LABEL, CaptureShell, MAIN_LABEL};
use hotkeys::{Hotkeys, PluginRegistrar};
use kinige_domain::ShortcutKind;
use kinige_store::{Store, StoreError, StoreOptions};
use recovery::Recovery;
use std::path::PathBuf;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

/// Команды главного окна (capability `main`).
pub const MAIN_COMMANDS: &[&str] = &[
    "app_info",
    "get_language",
    "list_chats",
    "list_messages",
    "send_message",
    "get_settings",
    "set_text_size",
    "choose_mirror_folder",
    "open_external_url",
    "get_draft",
    "save_draft",
    "set_shortcut",
    "reset_shortcut",
    "shortcut_recording",
    "create_chat",
    "rename_chat",
    "set_chat_icon",
    "set_chat_pinned",
    "set_chat_archived",
    "delete_message",
    "restore_message",
    "edit_message",
    "toggle_task_item",
    "set_message_flag",
    "set_message_date",
    "list_trash",
    "purge_message",
    "set_sidebar_collapsed",
    "add_recent_emoji",
    "startup_state",
    "restore_backup",
    "restart_for_recovery",
];

/// Команды окна захвата (capability `capture`): только Inbox, без `chat_id` (§12.3).
pub const CAPTURE_COMMANDS: &[&str] = &[
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
    "capture_resize",
];

/// Все команды приложения. Список совпадает с `AppManifest::commands` в `build.rs`: сначала команды главного окна,
/// затем окна захвата (это проверяют тесты).
pub const COMMANDS: &[&str] = &[
    "app_info",
    "get_language",
    "list_chats",
    "list_messages",
    "send_message",
    "get_settings",
    "set_text_size",
    "choose_mirror_folder",
    "open_external_url",
    "get_draft",
    "save_draft",
    "set_shortcut",
    "reset_shortcut",
    "shortcut_recording",
    "create_chat",
    "rename_chat",
    "set_chat_icon",
    "set_chat_pinned",
    "set_chat_archived",
    "delete_message",
    "restore_message",
    "edit_message",
    "toggle_task_item",
    "set_message_flag",
    "set_message_date",
    "list_trash",
    "purge_message",
    "set_sidebar_collapsed",
    "add_recent_emoji",
    "startup_state",
    "restore_backup",
    "restart_for_recovery",
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
    "capture_resize",
];

/// Конструктор приложения; общий для `run` и для тестов IPC (там с mock-рантаймом и своим `Store`).
/// Плагины opener и dialog подключены только ради Rust-API: JS-разрешений на них у окна нет (capability их не выдаёт).
/// Глобальные сочетания, трей и single-instance подключает только `run`: в тестах им нет места.
pub fn builder<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(guard::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_language,
            commands::list_chats,
            commands::list_messages,
            commands::send_message,
            commands::get_settings,
            commands::set_text_size,
            commands::choose_mirror_folder,
            commands::open_external_url,
            commands::get_draft,
            commands::save_draft,
            commands::set_shortcut,
            commands::reset_shortcut,
            commands::shortcut_recording,
            actions::create_chat,
            actions::rename_chat,
            actions::set_chat_icon,
            actions::set_chat_pinned,
            actions::set_chat_archived,
            actions::delete_message,
            actions::restore_message,
            actions::edit_message,
            actions::toggle_task_item,
            actions::set_message_flag,
            actions::set_message_date,
            actions::list_trash,
            actions::purge_message,
            actions::set_sidebar_collapsed,
            actions::add_recent_emoji,
            recovery::startup_state,
            recovery::restore_backup,
            recovery::restart_for_recovery,
            capture::capture_state,
            capture::capture_send,
            capture::capture_append,
            capture::capture_save_draft,
            capture::capture_hide,
            capture::capture_resize,
        ])
}

/// Где лежат данные. `KINIGE_DATA_DIR` и `KINIGE_MIRROR_DIR` — для проверки на чистой машине и для автотестов; обычный
/// запуск их не задаёт: база — в каталоге данных приложения, копия — в `~/Documents/Kinige`.
fn db_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = match std::env::var_os("KINIGE_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => app.path().app_data_dir()?,
    };
    Ok(data_dir.join("kinige.db"))
}

/// Папка Markdown-копии по умолчанию: `~/Documents/Kinige` (или `KINIGE_MIRROR_DIR`).
fn default_mirror_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    match std::env::var_os("KINIGE_MIRROR_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => app
            .path()
            .document_dir()
            .ok()
            .map(|documents| documents.join("Kinige")),
    }
}

/// Открывает хранилище. Повреждение, найденное проверкой целостности в фоне, ставит отметку и сообщает главному окну
/// (`recovery::corruption_hook`).
fn open_store<R: Runtime>(app: &AppHandle<R>, db_path: PathBuf) -> Result<Store, StoreError> {
    let mut options = StoreOptions::new(db_path.clone(), default_mirror_dir(app));
    options.on_corruption = Some(recovery::corruption_hook(app.clone(), db_path));
    Store::open(options)
}

/// Всё, что работает поверх открытого хранилища: сочетания, окно захвата как панель, значок в menu bar. Вызывается при
/// запуске и, если база была повреждена, сразу после восстановления из копии.
fn start_services<R: Runtime>(
    app: &AppHandle<R>,
    store: Store,
) -> Result<(), Box<dyn std::error::Error>> {
    let hotkeys = Hotkeys::new(Box::new(PluginRegistrar { app: app.clone() }));
    // Сочетание, которое система не приняла, остаётся «не назначено» в настройках; остальное работает.
    hotkeys.register_saved(&store);
    app.manage(store);
    app.manage(hotkeys);
    app.manage(CaptureShell::default());
    #[cfg(target_os = "macos")]
    panel::convert(
        app,
        CAPTURE_LABEL,
        capture::on_capture_key,
        capture::on_capture_blur,
        capture::request_hide,
    )?;
    tray::setup(app, commands::get_language())?;
    Ok(())
}

/// После восстановления из копии: база открывается, сервисы запускаются на главном потоке (значок и панель — AppKit).
pub(crate) fn resume_after_restore<R: Runtime>(app: &AppHandle<R>) -> Result<(), StoreError> {
    let db_path = db_path(app).map_err(|e| StoreError::Internal(e.to_string()))?;
    let store = open_store(app, db_path)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(start_services(&handle, store).map_err(|e| e.to_string()));
    })
    .map_err(|e| StoreError::Internal(e.to_string()))?;
    rx.recv()
        .map_err(|e| StoreError::Internal(e.to_string()))?
        .map_err(StoreError::Internal)
}

/// Нажатие глобального сочетания. В режиме записи нового сочетания («Настройки») нажатия игнорируются.
fn on_shortcut<R: Runtime>(app: &tauri::AppHandle<R>, pressed: &Shortcut) {
    let Some(hotkeys) = app.try_state::<Hotkeys>() else {
        return;
    };
    if hotkeys.is_recording() {
        return;
    }
    let kind = hotkeys.kind_for(|accelerator| {
        accelerator
            .parse::<Shortcut>()
            .is_ok_and(|shortcut| shortcut == *pressed)
    });
    match kind {
        Some(ShortcutKind::Capture) => capture::toggle_capture(app),
        Some(ShortcutKind::Clipboard) => capture::save_clipboard(app),
        None => {}
    }
}

/// Закрытие главного окна прячет его, приложение живёт в menu bar; закрытие окна захвата — то же, что Esc.
fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    match (window.label(), event) {
        (MAIN_LABEL, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            let _ = window.hide();
        }
        (CAPTURE_LABEL, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            capture::request_hide(window.app_handle());
        }
        // Щелчок в другое окно или другую программу: окно захвата прячется, черновик сохраняется.
        (CAPTURE_LABEL, WindowEvent::Focused(false)) => {
            capture::on_capture_blur(window.app_handle())
        }
        _ => {}
    }
}

pub fn run() {
    // Замер холодного старта (`scripts/measure-cold-start.sh`): печатаем метку, когда страница главного окна загружена.
    // Без переменной окружения ничего не выводится.
    let timing = std::env::var_os("KINIGE_TIMING").is_some();

    let configured = builder(
        tauri::Builder::default()
            // Должен идти первым: второй запуск показывает главное окно существующего экземпляра и завершается.
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                capture::show_main(app);
            }))
            .plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            on_shortcut(app, shortcut);
                        }
                    })
                    .build(),
            ),
    );
    // Окно захвата — панель NSPanel без активации приложения (ADR-0006).
    #[cfg(target_os = "macos")]
    let configured = configured.plugin(tauri_nspanel::init());
    let app = configured
        .setup(|app| {
            let handle = app.handle().clone();
            let db_path = db_path(&handle)?;
            // Журнал не открывается, и окно показывает копии (`recovery`), если: восстановление прошлого запуска не
            // закончено; отметка фоновой проверки подтвердилась повторной синхронной проверкой (не подтвердилась —
            // отметка снимается и база открывается как обычно); файла базы нет, а копии есть; база не открывается как
            // повреждённая. База новее приложения (§10.4) — отдельное сообщение. Любая другая ошибка открытия — не
            // повод предлагать копии: приложение не запускается и говорит об этом.
            let mirror = default_mirror_dir(&handle);
            let mut too_new = false;
            let store =
                if kinige_store::startup_check(&db_path) == kinige_store::StartupCheck::Recovery {
                    None
                } else {
                    match open_store(&handle, db_path.clone()) {
                        Ok(store) => Some(store),
                        Err(StoreError::Corrupt(_)) => {
                            eprintln!("kinige: база повреждена, предлагаем копии");
                            None
                        }
                        Err(StoreError::TooNew(found)) => {
                            eprintln!("kinige: база создана более новой версией (схема {found})");
                            too_new = true;
                            None
                        }
                        Err(error) => return Err(Box::new(error)),
                    }
                };
            let recovery = if too_new {
                Recovery::database_too_new(db_path)
            } else {
                Recovery::new(db_path, store.is_none())
            };
            app.manage(recovery.with_mirror(mirror));
            if let Some(store) = store {
                start_services(&handle, store)?;
            }
            Ok(())
        })
        .on_window_event(on_window_event)
        .on_page_load(move |webview, payload| {
            if timing && webview.label() == MAIN_LABEL && payload.event() == PageLoadEvent::Finished
            {
                eprintln!("kinige-timing: page-loaded");
            }
        })
        .build(tauri::generate_context!())
        .expect("не удалось собрать Kinige");

    app.run(|app, event| {
        // Щелчок по значку в Dock, когда окон на экране нет, показывает главное окно (событие есть только на macOS).
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            capture::show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
