//! Значок в menu bar (ТЗ §5.1): меню «Новая заметка» / «Inbox» / «Выйти» и короткое мигание значка при успешном
//! сохранении буфера (решение пользователя 22.10.2026). Строки меню — из тех же `web/src/locales/{en,ru}.json`, что и
//! интерфейс: один источник переводов.

use kinige_domain::Language;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{RecentChat, Store};
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
const ID_SETTINGS: &str = "tray-settings";
const ID_QUIT: &str = "tray-quit";
/// Пункт чата: `tray-chat:<id чата>`.
const ID_CHAT_PREFIX: &str = "tray-chat:";
const TRAY_ID: &str = "kinige";

/// Длительность одной вспышки и паузы между двумя вспышками.
const FLASH_ON: Duration = Duration::from_millis(160);
const FLASH_OFF: Duration = Duration::from_millis(110);

/// Подписи пунктов меню трея.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub new_note: String,
    pub inbox: String,
    pub settings: String,
    pub quit: String,
    pub tooltip: String,
}

/// Подписи на языке интерфейса из файлов переводов (`tray.*`). Нет ключа — подпись по умолчанию (английская), а не падение:
/// файлы вшиты в бинарник, и полноту ключей проверяет тест.
pub fn labels(language: Language) -> Labels {
    let source = translations(language);
    Labels {
        new_note: translation(source, ["tray", "newNote"], "New note"),
        inbox: translation(source, ["tray", "inbox"], "Inbox"),
        settings: translation(source, ["tray", "settings"], "Settings…"),
        quit: translation(source, ["tray", "quit"], "Quit"),
        tooltip: translation(source, ["tray", "tooltip"], "Kinige"),
    }
}

/// Заголовок окна «Настройки» на языке интерфейса (`settings.title` в файлах переводов).
pub fn window_title(language: Language) -> String {
    translation(translations(language), ["settings", "title"], "Settings")
}

fn translations(language: Language) -> &'static str {
    match language {
        Language::En => EN,
        Language::Ru => RU,
    }
}

/// Строка перевода по пути `[раздел, ключ]`; нет файла, раздела или строки — `fallback`.
fn translation(source: &str, path: [&str; 2], fallback: &str) -> String {
    let json: Value = serde_json::from_str(source).unwrap_or(Value::Null);
    json.get(path[0])
        .and_then(|section| section.get(path[1]))
        .and_then(Value::as_str)
        .map_or_else(
            || {
                log::error!("в переводах нет строки интерфейса, взята подпись по умолчанию");
                fallback.to_string()
            },
            str::to_string,
        )
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
    /// Подписи меню на языке интерфейса: нужны, чтобы пересобрать меню после изменений.
    labels: Labels,
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        ID_NEW_NOTE => crate::capture::show_capture(app),
        ID_INBOX => crate::capture::open_chat(app, INBOX_ID),
        ID_SETTINGS => crate::settings_window::open(app),
        ID_QUIT => app.exit(0),
        other => {
            if let Some(chat_id) = other.strip_prefix(ID_CHAT_PREFIX) {
                crate::capture::open_chat(app, chat_id);
            }
        }
    }
}

/// Сколько последних чатов (кроме Inbox) показывает меню значка.
pub const RECENT_CHATS: usize = 5;
/// Наибольшая длина названия чата в пункте меню (знаков); длиннее обрезается многоточием.
const MENU_TITLE_CHARS: usize = 40;

/// Пункт меню для чата: эмодзи и название, длинное название обрезано.
pub fn chat_label(chat: &RecentChat) -> String {
    let icon = chat.icon.as_deref().unwrap_or("💬");
    let title: String = if chat.title.chars().count() > MENU_TITLE_CHARS {
        let head: String = chat.title.chars().take(MENU_TITLE_CHARS - 1).collect();
        format!("{head}…")
    } else {
        chat.title.clone()
    };
    format!("{icon} {title}")
}

/// Меню значка: «Новая заметка», под ней Inbox и последние чаты, «Выйти».
fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    labels: &Labels,
    chats: &[RecentChat],
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        ID_NEW_NOTE,
        &labels.new_note,
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        ID_INBOX,
        &labels.inbox,
        true,
        None::<&str>,
    )?)?;
    for chat in chats {
        let id = format!("{ID_CHAT_PREFIX}{}", chat.id);
        menu.append(&MenuItem::with_id(
            app,
            id,
            chat_label(chat),
            true,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    // При запуске в фоне окна нет и ⌘, недоступно: «Настройки…» открывают отсюда. Пока журнал не открыт — недоступно.
    menu.append(&MenuItem::with_id(
        app,
        ID_SETTINGS,
        &labels.settings,
        app.try_state::<Store>().is_some(),
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        ID_QUIT,
        &labels.quit,
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

/// Создаёт значок и меню с последними чатами (если хранилище уже открыто).
pub fn setup<R: Runtime>(app: &AppHandle<R>, language: Language) -> tauri::Result<()> {
    let labels = labels(language);
    let chats = recent_chats(app);
    let menu = build_menu(app, &labels, &chats)?;
    let icons = icons()?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icons.normal.clone())
        .icon_as_template(true)
        .tooltip(&labels.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .build(app)?;
    app.manage(TrayFlash {
        tray,
        icons,
        labels,
    });
    Ok(())
}

fn recent_chats<R: Runtime>(app: &AppHandle<R>) -> Vec<RecentChat> {
    app.try_state::<Store>()
        .and_then(|store| store.recent_chats(RECENT_CHATS).ok())
        .unwrap_or_default()
}

/// Пересобирает меню значка по последним чатам: вызывается после изменений (запись, удаление, чат). Без значка
/// (тесты IPC, режим восстановления) ничего не делает. Меню меняется на главном потоке.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    if app.try_state::<TrayFlash<R>>().is_none() {
        return;
    }
    let chats = recent_chats(app);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(state) = handle.try_state::<TrayFlash<R>>() else {
            return;
        };
        if let Ok(menu) = build_menu(&handle, &state.labels, &chats) {
            let _ = state.tray.set_menu(Some(menu));
        }
    });
}

/// Два коротких мигания значка: «буфер сохранён» без звука и без окна. Работает на отдельном потоке.
pub fn flash<R: Runtime>(app: &AppHandle<R>) {
    crate::capture::trace("flash");
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("kinige-tray-flash".into())
        .spawn(move || {
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
    if spawned.is_err() {
        log::warn!("поток мигания значка не запущен");
    }
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
            &en.settings,
            &en.quit,
            &en.tooltip,
            &ru.new_note,
            &ru.inbox,
            &ru.settings,
            &ru.quit,
            &ru.tooltip,
        ] {
            assert!(!text.trim().is_empty());
        }
        assert_ne!(en.new_note, ru.new_note);
        assert_ne!(en.quit, ru.quit);
    }

    #[test]
    fn both_translation_files_have_every_string_the_tray_needs() {
        // Запасная подпись скрыла бы пропуск ключа, поэтому полноту проверяем по самим файлам.
        for source in [EN, RU] {
            let json: Value = serde_json::from_str(source).expect("файл переводов — JSON");
            for (section, key) in [
                ("tray", "newNote"),
                ("tray", "inbox"),
                ("tray", "settings"),
                ("tray", "quit"),
                ("tray", "tooltip"),
                ("settings", "title"),
            ] {
                assert!(
                    json[section][key].as_str().is_some_and(|s| !s.is_empty()),
                    "нет {section}.{key}"
                );
            }
        }
    }

    #[test]
    fn a_broken_or_incomplete_translation_file_gives_the_fallback_not_a_panic() {
        let path = ["tray", "quit"];
        for source in [
            "",
            "не json",
            "null",
            "[]",
            "{}",
            r#"{"tray":null}"#,
            r#"{"tray":{"quit":5}}"#,
            r#"{"tray":"quit"}"#,
        ] {
            assert_eq!(translation(source, path, "Quit"), "Quit", "{source:?}");
        }
        assert_eq!(
            translation(r#"{"tray":{"quit":"Выйти"}}"#, path, "Quit"),
            "Выйти"
        );
    }

    #[test]
    fn the_settings_window_title_comes_from_the_translation_files() {
        assert_ne!(window_title(Language::En), window_title(Language::Ru));
        assert!(!window_title(Language::En).trim().is_empty());
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
