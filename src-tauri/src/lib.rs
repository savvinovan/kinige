//! Тонкий слой Tauri: окно и команды. Бизнес-логика живёт в `kinige-domain`, хранение — в `kinige-store`.

mod commands;
pub mod guard;

use kinige_store::{Store, StoreOptions};
use std::path::PathBuf;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime};

/// Все команды приложения. Список совпадает с `AppManifest::commands` в `build.rs` и с capability главного окна.
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
];

/// Конструктор приложения; общий для `run` и для тестов IPC (там с mock-рантаймом и своим `Store`).
/// Плагины opener и dialog подключены только ради Rust-API: JS-разрешений на них у окна нет (capability их не выдаёт).
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

pub fn run() {
    // Замер холодного старта (`scripts/measure-cold-start.sh`): печатаем метку,
    // когда страница загружена. Без переменной окружения ничего не выводится.
    let timing = std::env::var_os("KINIGE_TIMING").is_some();

    builder(tauri::Builder::default())
        .setup(|app| {
            let store = open_store(app)?;
            app.manage(store);
            Ok(())
        })
        .on_page_load(move |_webview, payload| {
            if timing && payload.event() == PageLoadEvent::Finished {
                eprintln!("kinige-timing: page-loaded");
            }
        })
        .run(tauri::generate_context!())
        .expect("не удалось запустить Kinige");
}
