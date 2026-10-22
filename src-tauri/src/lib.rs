//! Тонкий слой Tauri: окна и команды. Бизнес-логика живёт в `kinige-domain`, хранение — в `kinige-store`.

pub mod capture;
mod commands;
pub mod guard;
pub mod hotkeys;
mod tray;

use capture::{CAPTURE_LABEL, CaptureShell, MAIN_LABEL};
use hotkeys::{Hotkeys, PluginRegistrar};
use kinige_domain::ShortcutKind;
use kinige_store::{Store, StoreOptions};
use std::path::PathBuf;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent, Runtime, WindowEvent};
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
];

/// Команды окна захвата (capability `capture`): только Inbox, без `chat_id` (§12.3).
pub const CAPTURE_COMMANDS: &[&str] = &[
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
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
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
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
            capture::capture_state,
            capture::capture_send,
            capture::capture_append,
            capture::capture_save_draft,
            capture::capture_hide,
        ])
}

/// Где лежат данные. `KINIGE_DATA_DIR` и `KINIGE_MIRROR_DIR` — для проверки на чистой машине и для автотестов; обычный
/// запуск их не задаёт: база — в каталоге данных приложения, копия — в `~/Documents/Kinige`.
fn open_store<R: Runtime>(app: &tauri::App<R>) -> Result<Store, Box<dyn std::error::Error>> {
    let data_dir = match std::env::var_os("KINIGE_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => app.path().app_data_dir()?,
    };
    let mirror_dir = match std::env::var_os("KINIGE_MIRROR_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => app
            .path()
            .document_dir()
            .ok()
            .map(|documents| documents.join("Kinige")),
    };
    Ok(Store::open(StoreOptions::new(
        data_dir.join("kinige.db"),
        mirror_dir,
    ))?)
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
            eprintln!("TRACE capture blur");
            capture::on_capture_blur(window.app_handle())
        }
        _ => {}
    }
}

pub fn run() {
    // Замер холодного старта (`scripts/measure-cold-start.sh`): печатаем метку, когда страница главного окна загружена.
    // Без переменной окружения ничего не выводится.
    let timing = std::env::var_os("KINIGE_TIMING").is_some();

    let app = builder(
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
    )
    .setup(|app| {
        let store = open_store(app)?;
        let hotkeys = Hotkeys::new(Box::new(PluginRegistrar {
            app: app.handle().clone(),
        }));
        // Сочетание, которое система не приняла, остаётся «не назначено» в настройках; остальное работает.
        hotkeys.register_saved(&store);
        app.manage(store);
        app.manage(hotkeys);
        app.manage(CaptureShell::default());
        tray::setup(app.handle(), commands::get_language())?;
        Ok(())
    })
    .on_window_event(on_window_event)
    .on_page_load(move |webview, payload| {
        if timing && webview.label() == MAIN_LABEL && payload.event() == PageLoadEvent::Finished {
            eprintln!("kinige-timing: page-loaded");
        }
    })
    .build(tauri::generate_context!())
    .expect("не удалось собрать Kinige");

    app.run(|app, event| {
        // Щелчок по значку в Dock, когда окон на экране нет, показывает главное окно.
        if let RunEvent::Reopen { .. } = event {
            capture::show_main(app);
        }
    });
}
