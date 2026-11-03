//! The last safety net of "never crash" (KINIG-28): a panic that `catch_unwind` around commands, writer tasks
//! and MCP tools did not catch still lands in `kinige.log`. Without the hook, in an .app started from Finder it would be invisible.
//!
//! Only the panic location (file:line) and the thread name go to the log. The panic message text is not written: it can contain pieces
//! of user data (`unwrap()` on an `Err`, a string slice, an index). The file path is trimmed: inside dependencies it is
//! absolute and contains the user name (`/Users/<name>/.cargo/registry/...`).

use std::panic::{self, PanicHookInfo};

/// How many trailing parts of an absolute path to keep in the log.
const KEPT_PATH_PARTS: usize = 3;

/// Installs the panic hook. The previous hook (output to stderr for the developer) is called after writing to the log.
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
        || "unknown location".to_string(),
        |location| format!("{}:{}", short_path(location.file()), location.line()),
    );
    format!(
        "panic in thread \"{}\" at {place}",
        thread.name().unwrap_or("unnamed")
    )
}

/// A path without the home directory and without the registry hash: a relative path stays as is, for registry dependencies it is
/// `<crate-version>/src/...`, for other absolute ones the last parts.
fn short_path(file: &str) -> String {
    const REGISTRY: &str = "/registry/src/";
    if let Some(at) = file.find(REGISTRY) {
        // After `registry/src/` comes the index directory (`index.crates.io-<hash>`): it tells nothing.
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
