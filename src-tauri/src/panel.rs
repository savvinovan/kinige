//! Окно захвата как неактивирующая панель NSPanel (macOS, ADR-0006).
//!
//! Обычное окно Tauri на macOS 14+ получает ввод ненадёжно, когда активна чужая программа: система «кооперативно» отказывает
//! в активации, и окно то получает фокус, то теряет его, а нажатия клавиш уходят не в страницу. Панель с флагом
//! `NonactivatingPanel` становится ключевым окном, не активируя приложение (так устроены Spotlight, Raycast, Alfred): прежняя
//! программа остаётся активной и после скрытия панели сама получает фокус обратно, главное окно не всплывает.
//! Библиотека — `tauri-nspanel` 2.1.1 (требует флаг `macOSPrivateApi`).

#![allow(clippy::unused_unit)] // макрос `panel_event!` пишет `-> ()`

use tauri::{AppHandle, Manager, Runtime};
use tauri_nspanel::{
    CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel,
};

tauri_panel! {
    panel!(CapturePanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })

    panel_event!(CapturePanelEvents {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
        window_should_close(window: &NSWindow) -> Bool
    })
}

/// Превращает созданное при старте скрытое окно захвата в панель: без активации, поверх остальных, на всех рабочих столах и
/// над полноэкранными программами. Потеря ключевого статуса — спрятаться (`on_resign`), закрытие окна (⌘W) — то же самое,
/// но панель не уничтожается.
pub fn convert<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    on_become: impl Fn(&AppHandle<R>) + 'static,
    on_resign: impl Fn(&AppHandle<R>) + 'static,
    on_close: impl Fn(&AppHandle<R>) + 'static,
) -> tauri::Result<()> {
    let window = app
        .get_webview_window(label)
        .ok_or(tauri::Error::WindowNotFound)?;
    let panel = window.to_panel::<CapturePanel<R>>()?;
    panel
        .add_style_mask(StyleMask::empty().nonactivating_panel().value())
        .map_err(|error| tauri::Error::Anyhow(error.into()))?;
    panel.set_level(PanelLevel::Floating.value());
    panel.set_hides_on_deactivate(false);
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );

    let handler = CapturePanelEvents::new();
    let resign_app = app.clone();
    let become_app = app.clone();
    handler.window_did_become_key(move |_| on_become(&become_app));
    handler.window_did_resign_key(move |_| on_resign(&resign_app));
    let close_app = app.clone();
    handler.window_should_close(move |_| {
        // Закрытие окна не уничтожает панель: окно просто прячется (через страницу, чтобы сохранить черновик).
        on_close(&close_app);
        tauri_nspanel::objc2::runtime::Bool::NO
    });
    panel.set_event_handler(Some(handler.as_ref()));
    Ok(())
}

/// Панель окна, если плагин подключён и окно уже превращено в панель (в тестах с mock-рантаймом плагина нет).
fn panel_of<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<tauri_nspanel::PanelHandle<R>> {
    app.try_state::<tauri_nspanel::WebviewPanelManager<R>>()?;
    app.get_webview_panel(label).ok()
}

/// Показывает панель и делает её ключевой, не активируя приложение.
pub fn show<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    panel_of(app, label).is_some_and(|panel| {
        panel.show_and_make_key();
        true
    })
}

/// Возвращает панели ключевой статус (система сняла его в первые мгновения после показа).
pub fn make_key<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(panel) = panel_of(app, label) {
        panel.make_key_window();
    }
}

/// Прячет панель.
pub fn hide<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    panel_of(app, label).is_some_and(|panel| {
        panel.hide();
        true
    })
}
