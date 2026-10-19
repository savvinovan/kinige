//! Тонкий слой Tauri: окно и команды. Бизнес-логика живёт в `kinige-domain`.

use kinige_domain::AppInfo;
use tauri::webview::PageLoadEvent;

/// Сведения о приложении для `KinigeApi.appInfo()`.
#[tauri::command]
fn app_info() -> AppInfo {
    kinige_domain::app_info(env!("CARGO_PKG_VERSION"))
}

/// Конструктор приложения; общий для `run` и для тестов IPC (там с mock-рантаймом).
pub fn builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![app_info])
}

pub fn run() {
    // Замер холодного старта (`scripts/measure-cold-start.sh`): печатаем метку,
    // когда страница загружена. Без переменной окружения ничего не выводится.
    let timing = std::env::var_os("KINIGE_TIMING").is_some();

    builder(tauri::Builder::default())
        .on_page_load(move |_webview, payload| {
            if timing && payload.event() == PageLoadEvent::Finished {
                eprintln!("kinige-timing: page-loaded");
            }
        })
        .run(tauri::generate_context!())
        .expect("не удалось запустить Kinige");
}
