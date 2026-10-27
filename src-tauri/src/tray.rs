//! Значок в menu bar (ТЗ §5.1): меню «Новая заметка» / «Inbox» / «Выйти» и короткое мигание значка при успешном
//! сохранении буфера (решение пользователя 22.10.2026). Строки меню — из тех же `web/src/locales/{en,ru}.json`, что и
//! интерфейс: один источник переводов.

use kinige_domain::Language;
use serde_json::Value;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Runtime};

const EN: &str = include_str!("../../web/src/locales/en.json");
const RU: &str = include_str!("../../web/src/locales/ru.json");

const ID_NEW_NOTE: &str = "tray-new-note";
const ID_INBOX: &str = "tray-inbox";
const ID_QUIT: &str = "tray-quit";
const TRAY_ID: &str = "kinige";

/// Длительность одной вспышки и паузы между двумя вспышками.
const FLASH_ON: Duration = Duration::from_millis(160);
const FLASH_OFF: Duration = Duration::from_millis(110);

/// Подписи пунктов меню трея.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub new_note: String,
    pub inbox: String,
    pub quit: String,
    pub tooltip: String,
}

/// Подписи на языке интерфейса из файлов переводов (`tray.*`). Нет ключа — это ошибка сборки, её ловит тест.
pub fn labels(language: Language) -> Labels {
    let source = match language {
        Language::En => EN,
        Language::Ru => RU,
    };
    let json: Value = serde_json::from_str(source).expect("файл переводов — корректный JSON");
    let text = |key: &str| -> String {
        json["tray"][key]
            .as_str()
            .unwrap_or_else(|| panic!("в переводах нет tray.{key}"))
            .to_string()
    };
    Labels {
        new_note: text("newNote"),
        inbox: text("inbox"),
        quit: text("quit"),
        tooltip: text("tooltip"),
    }
}

/// Значки: обычный (template, macOS перекрашивает под светлую и тёмную строку меню) и «вспышка».
struct Icons {
    normal: Image<'static>,
    flash: Image<'static>,
}

fn icons() -> tauri::Result<Icons> {
    Ok(Icons {
        normal: Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?,
        flash: Image::from_bytes(include_bytes!("../icons/tray-flash@2x.png"))?,
    })
}

/// Значок трея в состоянии приложения: нужен, чтобы мигать им из любого потока.
pub struct TrayFlash<R: Runtime> {
    tray: TrayIcon<R>,
    icons: Icons,
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        ID_NEW_NOTE => crate::capture::show_capture(app),
        ID_INBOX => crate::capture::show_main(app),
        ID_QUIT => app.exit(0),
        _ => {}
    }
}

/// Создаёт значок и меню. «Последние чаты» — Inbox, пока других чатов нет (слайс 3 добавит остальные).
pub fn setup<R: Runtime>(app: &AppHandle<R>, language: Language) -> tauri::Result<()> {
    let labels = labels(language);
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ID_NEW_NOTE, &labels.new_note, true, None::<&str>)?,
            &MenuItem::with_id(app, ID_INBOX, &labels.inbox, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_QUIT, &labels.quit, true, None::<&str>)?,
        ],
    )?;
    let icons = icons()?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icons.normal.clone())
        .icon_as_template(true)
        .tooltip(&labels.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .build(app)?;
    app.manage(TrayFlash { tray, icons });
    Ok(())
}

/// Два коротких мигания значка: «буфер сохранён» без звука и без окна. Работает на отдельном потоке.
pub fn flash<R: Runtime>(app: &AppHandle<R>) {
    crate::capture::trace("flash");
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(state) = app.try_state::<TrayFlash<R>>() else {
            return;
        };
        let set = |icon: &Image<'static>| {
            let _ = state
                .tray
                .set_icon_with_as_template(Some(icon.clone()), true);
        };
        for _ in 0..2 {
            set(&state.icons.flash);
            std::thread::sleep(FLASH_ON);
            set(&state.icons.normal);
            std::thread::sleep(FLASH_OFF);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_come_from_the_translation_files_in_both_languages() {
        let en = labels(Language::En);
        let ru = labels(Language::Ru);
        for text in [
            &en.new_note,
            &en.inbox,
            &en.quit,
            &en.tooltip,
            &ru.new_note,
            &ru.inbox,
            &ru.quit,
            &ru.tooltip,
        ] {
            assert!(!text.trim().is_empty());
        }
        assert_ne!(en.new_note, ru.new_note);
        assert_ne!(en.quit, ru.quit);
    }

    #[test]
    fn tray_icons_are_valid_pngs_and_differ() {
        let icons = icons().expect("иконки трея разбираются как PNG");
        assert!(icons.normal.width() > 0 && icons.normal.height() > 0);
        assert_eq!(
            (icons.normal.width(), icons.normal.height()),
            (icons.flash.width(), icons.flash.height()),
            "вспышка того же размера"
        );
        assert_ne!(icons.normal.rgba(), icons.flash.rgba());
    }
}
