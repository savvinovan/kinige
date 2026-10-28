//! Тонкий слой Tauri: окна и команды. Бизнес-логика живёт в `kinige-domain`, хранение — в `kinige-store`.

mod actions;
mod agent_access;
pub mod capture;
mod commands;
pub mod guard;
pub mod hotkeys;
pub mod import;
pub mod logging;
pub mod mcp;
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
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

/// Аргумент запуска мостом MCP: окно не показывается, приложение живёт в menu bar.
pub const BACKGROUND_FLAG: &str = "--background";

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
    "move_message",
    "list_trash",
    "purge_message",
    "set_sidebar_collapsed",
    "add_recent_emoji",
    "startup_state",
    "restore_backup",
    "restart_for_recovery",
    "import_telegram_open",
    "import_telegram_run",
    "import_telegram_stop",
    "import_telegram_close",
    "search_messages",
    "get_agent_access",
    "set_agent_enabled",
    "set_agent_mode",
    "set_agent_chats",
    "list_agent_log",
    "list_agent_log_entries",
];

/// Команды окна захвата (capability `capture`): только Inbox, без `chat_id` (§12.3).
pub const CAPTURE_COMMANDS: &[&str] = &[
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
    "capture_resize",
    "capture_add_recent_emoji",
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
    "move_message",
    "list_trash",
    "purge_message",
    "set_sidebar_collapsed",
    "add_recent_emoji",
    "startup_state",
    "restore_backup",
    "restart_for_recovery",
    "import_telegram_open",
    "import_telegram_run",
    "import_telegram_stop",
    "import_telegram_close",
    "search_messages",
    "get_agent_access",
    "set_agent_enabled",
    "set_agent_mode",
    "set_agent_chats",
    "list_agent_log",
    "list_agent_log_entries",
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
    "capture_resize",
    "capture_add_recent_emoji",
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
            actions::move_message,
            actions::list_trash,
            actions::purge_message,
            actions::set_sidebar_collapsed,
            actions::add_recent_emoji,
            recovery::startup_state,
            recovery::restore_backup,
            recovery::restart_for_recovery,
            import::import_telegram_open,
            import::import_telegram_run,
            import::import_telegram_stop,
            import::import_telegram_close,
            import::search_messages,
            agent_access::get_agent_access,
            agent_access::set_agent_enabled,
            agent_access::set_agent_mode,
            agent_access::set_agent_chats,
            agent_access::list_agent_log,
            agent_access::list_agent_log_entries,
            capture::capture_state,
            capture::capture_send,
            capture::capture_append,
            capture::capture_save_draft,
            capture::capture_hide,
            capture::capture_resize,
            capture::capture_add_recent_emoji,
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

/// Открывает хранилище. Повреждение, найденное проверкой целостности в фоне или любой командой с любого потока (она
/// прочитала битую страницу), ставит отметку, флаг процесса и сообщает главному окну
/// (`recovery::corruption_hook`).
fn open_store<R: Runtime>(
    app: &AppHandle<R>,
    db_path: PathBuf,
    corruption: Arc<AtomicBool>,
) -> Result<Store, StoreError> {
    let mut options = StoreOptions::new(db_path.clone(), default_mirror_dir(app));
    options.on_corruption = Some(recovery::corruption_hook(app.clone(), db_path, corruption));
    Store::open(options)
}

/// Всё, что работает поверх открытого хранилища: сочетания, окно захвата как панель, значок в menu bar. Вызывается при
/// запуске и, если база была повреждена, сразу после восстановления из копии.
///
/// Хранилище (и всё остальное, что делает «журнал открыт») отдаётся приложению ПОСЛЕДНИМ, когда всё, что может упасть
/// (панель, значок), уже удалось: иначе при сбое восстановление считалось бы неудавшимся, а хранилище уже стояло бы в
/// состоянии приложения, и повторное «Восстановить» подменяло бы файл базы под открытым писателем.
fn start_services<R: Runtime>(
    app: &AppHandle<R>,
    store: Store,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    panel::convert(
        app,
        CAPTURE_LABEL,
        capture::on_capture_key,
        capture::on_capture_blur,
        capture::request_hide,
    )?;
    tray::setup(app, commands::get_language())?;
    let hotkeys = Hotkeys::new(Box::new(PluginRegistrar { app: app.clone() }));
    // Сочетание, которое система не приняла, остаётся «не назначено» в настройках; остальное работает.
    hotkeys.register_saved(&store);
    app.manage(store);
    app.manage(hotkeys);
    app.manage(CaptureShell::default());
    app.manage(import::ImportState::default());
    // Значок строился без хранилища (его ещё не было в состоянии приложения): последние чаты — теперь.
    tray::refresh(app);
    // Сервер MCP — последним, после `app.manage(store)`: сбой сокета журнал не останавливает (ошибка видна в настройках).
    if let Ok(db_path) = db_path(app)
        && let Some(data_dir) = db_path.parent()
    {
        mcp::start(app, data_dir);
    }
    Ok(())
}

/// После восстановления из копии: база открывается, сервисы запускаются на главном потоке (значок и панель — AppKit).
pub(crate) fn resume_after_restore<R: Runtime>(app: &AppHandle<R>) -> Result<(), StoreError> {
    let db_path = db_path(app).map_err(|e| StoreError::Internal(e.to_string()))?;
    let corruption = app
        .try_state::<Recovery>()
        .map(|recovery| recovery.corruption_flag())
        .unwrap_or_default();
    let store = open_store(app, db_path, corruption)?;
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
    let configured = builder(
        tauri::Builder::default()
            // Должен идти первым: второй запуск показывает главное окно существующего экземпляра и завершается.
            .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                // Мост MCP запускает приложение с `--background`: уже запущенному окно показывать не нужно.
                if !args.iter().any(|arg| arg == BACKGROUND_FLAG) {
                    capture::show_main(app);
                }
            }))
            // После single-instance: второй экземпляр завершается, не открыв файл журнала.
            .plugin(logging::plugin())
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
            log::info!("запуск Kinige {}", env!("CARGO_PKG_VERSION"));
            let handle = app.handle().clone();
            let db_path = db_path(&handle)?;
            // Журнал не открывается, и окно показывает копии (`recovery`), если: восстановление прошлого запуска не
            // закончено; отметка фоновой проверки подтвердилась повторной синхронной проверкой (не подтвердилась —
            // отметка снимается и база открывается как обычно); файла базы нет, а копии есть; база не открывается как
            // повреждённая. База новее приложения (§10.4) — отдельное сообщение. Любая другая ошибка открытия — не
            // повод предлагать копии: приложение не запускается и говорит об этом.
            let mirror = default_mirror_dir(&handle);
            let mut too_new = false;
            // Флаг «повреждение найдено» нужен хуку проверки, которая стартует вместе с хранилищем, раньше `Recovery`.
            let corruption = Arc::new(AtomicBool::new(false));
            let store =
                if kinige_store::startup_check(&db_path) == kinige_store::StartupCheck::Recovery {
                    None
                } else {
                    match open_store(&handle, db_path.clone(), corruption.clone()) {
                        Ok(store) => Some(store),
                        Err(StoreError::Corrupt(_)) => {
                            log::error!("база повреждена, предлагаем копии");
                            None
                        }
                        Err(StoreError::TooNew(found)) => {
                            log::warn!("база создана более новой версией (схема {found})");
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
            app.manage(
                recovery
                    .with_mirror(mirror)
                    .with_corruption_flag(corruption),
            );
            if let Some(store) = store {
                start_services(&handle, store)?;
            }
            // Главное окно создаётся скрытым (`visible: false`), чтобы запуск в фоне не мигал окном: показываем его здесь,
            // если приложение запущено не мостом MCP (`--background`, значок в menu bar без окна).
            if !std::env::args().any(|arg| arg == BACKGROUND_FLAG) {
                capture::show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(on_window_event)
        .on_page_load(|webview, payload| {
            // Замер холодного старта (`scripts/measure-cold-start.sh`): метка, когда страница главного окна загружена.
            // Без `KINIGE_TIMING=1` уровень цели `kinige-timing` выключен, и ничего не пишется.
            if webview.label() == MAIN_LABEL && payload.event() == PageLoadEvent::Finished {
                log::info!(target: logging::TIMING_TARGET, "page-loaded");
            }
        })
        .build(tauri::generate_context!())
        .expect("не удалось собрать Kinige");

    app.run(|app, event| {
        // Файл сокета MCP не должен пережить приложение.
        if let tauri::RunEvent::Exit = event {
            mcp::shutdown(app);
        }
        // Щелчок по значку в Dock, когда окон на экране нет, показывает главное окно (событие есть только на macOS).
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            capture::show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
