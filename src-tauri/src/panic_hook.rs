//! Последняя страховка «никогда не падать» (KINIG-28): паника, которую не поймал `catch_unwind` вокруг команд, задач
//! писателя и инструментов MCP, всё равно попадает в `kinige.log`. Без хука в .app из Finder она была бы невидимой.
//!
//! В журнал идёт только место паники (файл:строка) и имя потока. Текст сообщения паники не пишется: в нём бывают куски
//! данных пользователя (`unwrap()` на `Err`, срез строки, индекс). Путь файла обрезается: внутри зависимостей он
//! абсолютный и содержит имя пользователя (`/Users/<имя>/.cargo/registry/...`).

use std::panic::{self, PanicHookInfo};

/// Сколько последних частей абсолютного пути оставлять в журнале.
const KEPT_PATH_PARTS: usize = 3;

/// Ставит хук паники. Прежний хук (вывод в stderr для разработчика) вызывается после записи в журнал.
pub fn install() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log::error!("{}", describe(info));
        previous(info);
    }));
}

fn describe(info: &PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    let place = info.location().map_or_else(
        || "место неизвестно".to_string(),
        |location| format!("{}:{}", short_path(location.file()), location.line()),
    );
    format!(
        "паника в потоке «{}» в {place}",
        thread.name().unwrap_or("без имени")
    )
}

/// Путь без домашнего каталога и без хэша реестра: относительный путь остаётся как есть, у зависимостей из реестра —
/// `<крейт-версия>/src/…`, у остальных абсолютных — последние части.
fn short_path(file: &str) -> String {
    const REGISTRY: &str = "/registry/src/";
    if let Some(at) = file.find(REGISTRY) {
        // После `registry/src/` идёт каталог индекса (`index.crates.io-<хэш>`): он ничего не говорит.
        let rest = file.get(at + REGISTRY.len()..).unwrap_or_default();
        return rest
            .split_once('/')
            .map_or(rest, |(_, tail)| tail)
            .to_string();
    }
    let absolute = file.starts_with('/') || file.contains(":\\");
    if !absolute {
        return file.to_string();
    }
    let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let from = parts.len().saturating_sub(KEPT_PATH_PARTS);
    parts.get(from..).unwrap_or_default().join("/")
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn a_relative_path_of_our_own_crate_stays_as_is() {
        assert_eq!(short_path("src-tauri/src/lib.rs"), "src-tauri/src/lib.rs");
    }

    #[test]
    fn a_registry_path_loses_the_home_directory_and_the_index_hash() {
        assert_eq!(
            short_path(
                "/Users/someone/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-2.12.4/src/app.rs"
            ),
            "tauri-2.12.4/src/app.rs"
        );
    }

    #[test]
    fn any_other_absolute_path_keeps_only_its_last_parts() {
        assert_eq!(
            short_path("/Users/someone/work/project/src/main.rs"),
            "project/src/main.rs"
        );
        assert_eq!(
            short_path("C:\\Users\\someone\\project\\src\\main.rs"),
            "project/src/main.rs"
        );
        assert_eq!(
            short_path("/rustc/abc123/library/std/src/x.rs"),
            "std/src/x.rs"
        );
        assert_eq!(short_path("/x.rs"), "x.rs");
        assert_eq!(short_path(""), "");
    }
}
