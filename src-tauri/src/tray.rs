//! The menu bar icon (PRD §5.1): the menu "New note" / "Inbox" / "Quit" and a short icon blink on a successful
//! clipboard save (user decision of 22.10.2026). The menu strings come from the same `web/src/locales/{en,ru}.json` as the
//! interface: one source of translations.

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
/// A chat item: `tray-chat:<chat id>`.
const ID_CHAT_PREFIX: &str = "tray-chat:";
const TRAY_ID: &str = "kinige";

/// The duration of one flash and of the pause between two flashes.
const FLASH_ON: Duration = Duration::from_millis(160);
const FLASH_OFF: Duration = Duration::from_millis(110);

/// The labels of the tray menu items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub new_note: String,
    pub inbox: String,
    pub settings: String,
    pub quit: String,
    pub tooltip: String,
}

/// Labels in the interface language from the translation files (`tray.*`). A missing key gives the default (English) label, not a crash:
/// the files are embedded in the binary, and a test checks the keys are complete.
pub fn labels(language: Language) -> Labels {
    let json = parse_translations(translations(language));
    Labels {
        new_note: translation(&json, ["tray", "newNote"], "New note"),
        inbox: translation(&json, ["tray", "inbox"], "Inbox"),
        settings: translation(&json, ["tray", "settings"], "Settings…"),
        quit: translation(&json, ["tray", "quit"], "Quit"),
        tooltip: translation(&json, ["tray", "tooltip"], "Kinige"),
    }
}

/// The title of the "Settings" window in the interface language (`settings.title` in the translation files).
pub fn window_title(language: Language) -> String {
    let json = parse_translations(translations(language));
    translation(&json, ["settings", "title"], "Settings")
}

fn translations(language: Language) -> &'static str {
    match language {
        Language::En => EN,
        Language::Ru => RU,
    }
}

/// A translation file as JSON; if it does not parse, `Null` (all strings then come from the fallback labels).
fn parse_translations(source: &str) -> Value {
    serde_json::from_str(source).unwrap_or(Value::Null)
}

/// A translation string by the path `[section, key]`; no section or string gives `fallback`.
fn translation(json: &Value, path: [&str; 2], fallback: &str) -> String {
    json.get(path[0])
        .and_then(|section| section.get(path[1]))
        .and_then(Value::as_str)
        .map_or_else(
            || {
                log::error!(
                    "a UI string is missing in the translations, the default label is used"
                );
                fallback.to_string()
            },
            str::to_string,
        )
}

/// Icons: the regular one (template, macOS recolors it for the light and dark menu bar) and the "flash".
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

/// The tray icon in the app state: needed to blink it from any thread.
pub struct TrayFlash<R: Runtime> {
    tray: TrayIcon<R>,
    icons: Icons,
    /// Menu labels in the interface language: needed to rebuild the menu after changes.
    labels: Labels,
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        ID_NEW_NOTE => crate::capture::show_capture(app),
        ID_INBOX => crate::capture::open_chat(app, INBOX_ID),
        ID_SETTINGS => crate::settings_window::open_from_event(app),
        ID_QUIT => app.exit(0),
        other => {
            if let Some(chat_id) = other.strip_prefix(ID_CHAT_PREFIX) {
                crate::capture::open_chat(app, chat_id);
            }
        }
    }
}

/// How many recent chats (besides Inbox) the icon menu shows.
pub const RECENT_CHATS: usize = 5;
/// The maximum chat name length in a menu item (characters); a longer one is truncated with an ellipsis.
const MENU_TITLE_CHARS: usize = 40;

/// A menu item for a chat: the emoji and the name, a long name is truncated.
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

/// The icon menu: "New note", below it Inbox and the recent chats, "Quit".
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
    // On a background launch there is no window and Cmd+, is unavailable: "Settings..." is opened from here. While the journal is not open it is unavailable.
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

/// Creates the icon and the menu with recent chats (if the store is already open).
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

/// Rebuilds the icon menu from the recent chats: called after changes (an entry, a deletion, a chat). Without an icon
/// (IPC tests, recovery mode) it does nothing. The menu is changed on the main thread.
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

/// Two short icon blinks: "clipboard saved" without sound and without a window. Runs on a separate thread.
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
        log::warn!("icon blink thread did not start");
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
        // A fallback label would hide a missing key, so we check completeness against the files themselves.
        for source in [EN, RU] {
            let json: Value = serde_json::from_str(source).expect("the translation file is JSON");
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
                    "missing {section}.{key}"
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
            let json = parse_translations(source);
            assert_eq!(translation(&json, path, "Quit"), "Quit", "{source:?}");
        }
        assert_eq!(
            translation(
                &parse_translations(r#"{"tray":{"quit":"Выйти"}}"#),
                path,
                "Quit"
            ),
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
        let icons = icons().expect("the tray icons parse as PNG");
        assert!(icons.normal.width() > 0 && icons.normal.height() > 0);
        assert_eq!(
            (icons.normal.width(), icons.normal.height()),
            (icons.flash.width(), icons.flash.height()),
            "the flash has the same size"
        );
        assert_ne!(icons.normal.rgba(), icons.flash.rgba());
    }
}
