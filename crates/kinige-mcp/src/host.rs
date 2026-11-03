//! The Kinige app next to the bridge: whether it is running and how to start it in the background (ADR-0009). `open` is called only when
//! there is no process: to a running app it would arrive as Reopen and show the main window.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The name of the main executable in `Kinige.app/Contents/MacOS/`.
const MAIN_BINARY: &str = "kinige";

/// Override of the process check in the bridge tests: `present` or `absent`.
const TEST_PROCESS: &str = "KINIGE_MCP_TEST_PROCESS";
/// Instead of `open`, in tests the bridge appends a line to this file.
const TEST_OPEN_LOG: &str = "KINIGE_MCP_TEST_OPEN_LOG";

/// Where the app is, from whose bundle the bridge was started.
pub struct Host {
    /// `Kinige.app` if the bridge sits in the bundle's `Contents/MacOS/`; a `dev` build has no bundle.
    bundle: Option<PathBuf>,
}

impl Host {
    pub fn locate() -> Host {
        let exe = env::current_exe().and_then(std::fs::canonicalize);
        let bundle = exe.ok().and_then(|exe| bundle_of(&exe));
        Host { bundle }
    }

    /// The app process from its own bundle exists. Without a bundle (dev) and without an override in tests, there is considered to be no process, but
    /// there is nothing to start: `launch` does nothing.
    pub fn process_running(&self) -> bool {
        if let Ok(forced) = env::var(TEST_PROCESS) {
            return forced == "present";
        }
        let Some(bundle) = &self.bundle else {
            return false;
        };
        // Paths are compared after `canonicalize`: `current_exe` on macOS returns the path as launched (with `..`, symlinks, `/tmp`
        // instead of `/private/tmp`), while `ps` returns the path as the system started it. A mistake here would run `open` on top of a running
        // app, which is a Reopen and a window on the screen.
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

    /// Starts the app in the background: without bringing a window to the front and without a window at all (`--background`).
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
