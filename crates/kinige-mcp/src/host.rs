//! Приложение Kinige рядом с мостом: запущено ли оно и как его запустить в фоне (ADR-0009). `open` вызывается только когда
//! процесса нет: запущенному приложению он пришёл бы как Reopen и показал бы главное окно.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Имя главного исполняемого файла в `Kinige.app/Contents/MacOS/`.
const MAIN_BINARY: &str = "kinige";

/// Подмена проверки процесса в тестах моста: `present` или `absent`.
const TEST_PROCESS: &str = "KINIGE_MCP_TEST_PROCESS";
/// Вместо `open` в тестах мост дописывает строку в этот файл.
const TEST_OPEN_LOG: &str = "KINIGE_MCP_TEST_OPEN_LOG";

/// Где находится приложение, из бандла которого запущен мост.
pub struct Host {
    /// `Kinige.app`, если мост лежит в `Contents/MacOS/` бандла; в сборке `dev` бандла нет.
    bundle: Option<PathBuf>,
}

impl Host {
    pub fn locate() -> Host {
        let exe = env::current_exe().and_then(std::fs::canonicalize);
        let bundle = exe.ok().and_then(|exe| bundle_of(&exe));
        Host { bundle }
    }

    /// Процесс приложения из своего бандла есть. Без бандла (dev) и без подмены в тестах считается, что процесса нет, но
    /// запускать нечего: `launch` ничего не делает.
    pub fn process_running(&self) -> bool {
        if let Ok(forced) = env::var(TEST_PROCESS) {
            return forced == "present";
        }
        let Some(bundle) = &self.bundle else {
            return false;
        };
        // Пути сравниваются после `canonicalize`: `current_exe` на macOS отдаёт путь как запущено (с `..`, симлинками, `/tmp`
        // вместо `/private/tmp`), а `ps` — путь, как запустила система. Ошибка здесь запустила бы `open` поверх работающего
        // приложения, а это Reopen и окно на экране.
        let Ok(main) = std::fs::canonicalize(bundle.join("Contents/MacOS").join(MAIN_BINARY))
        else {
            return false;
        };
        let Ok(output) = Command::new("ps").args(["-axo", "comm="]).output() else {
            return false;
        };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim_end)
            .filter(|line| line.ends_with(MAIN_BINARY))
            .any(|line| std::fs::canonicalize(line).is_ok_and(|path| path == main))
    }

    /// Запускает приложение в фоне: без вывода окна на передний план и без окна вовсе (`--background`).
    pub fn launch(&self) {
        if let Ok(path) = env::var(TEST_OPEN_LOG) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(file, "open");
            }
            return;
        }
        let Some(bundle) = &self.bundle else {
            return;
        };
        let _ = Command::new("open")
            .arg("-g")
            .arg("-a")
            .arg(bundle)
            .args(["--args", "--background"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// `.../Kinige.app/Contents/MacOS/kinige-mcp` → `.../Kinige.app`.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let named_like_a_bundle = bundle.extension().is_some_and(|ext| ext == "app");
    let in_place = macos.file_name()? == "MacOS" && contents.file_name()? == "Contents";
    (named_like_a_bundle && in_place).then(|| bundle.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_only_for_a_real_bundle_layout() {
        let inside = Path::new("/Applications/Kinige.app/Contents/MacOS/kinige-mcp");
        assert_eq!(
            bundle_of(inside),
            Some(PathBuf::from("/Applications/Kinige.app"))
        );
        for outside in [
            "/tmp/target/debug/kinige-mcp",
            "/Applications/Kinige/Contents/MacOS/kinige-mcp",
            "kinige-mcp",
            "/",
        ] {
            assert_eq!(bundle_of(Path::new(outside)), None, "{outside}");
        }
    }
}
