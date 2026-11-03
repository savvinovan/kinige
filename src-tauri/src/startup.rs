//! App startup and the safety net around it (KINIG-28, ADR-0011): process arguments, a startup error without a panic, a wrapper
//! around the Tauri command handler that turns a panic into a reply to the window.

use kinige_domain::ErrorCode;
use kinige_store::StoreError;
use std::ffi::OsString;
use std::panic::{self, AssertUnwindSafe};
use tauri::Runtime;
use tauri::ipc::{Invoke, InvokeResolver};

/// The launch argument used by the MCP bridge: no window is shown, the app lives in the menu bar.
pub const BACKGROUND_FLAG: &str = "--background";

/// The exit code if the app could not start (a startup or build error). There is no panic.
const EXIT_STARTUP_FAILED: i32 = 1;

/// Started by the MCP bridge (`--background`)? Then there is no window and no Dock icon: the app lives in the menu bar (the
/// Accessory activation policy); the first window shown restores the regular policy (`capture::show_main`).
pub fn is_background_launch<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|arg| arg == BACKGROUND_FLAG)
}

/// Whether to show the main window at launch. A regular launch always shows it. A launch by the MCP bridge (`--background`) lives in the
/// menu bar without a window, but only if the journal is open: with a corrupt or newer database (`store_open = false`) an invisible
/// process would sit in the menu bar, while the recovery screen (`recovery`) is the only place where a person sees what happened.
pub fn should_show_main_on_launch(background: bool, store_open: bool) -> bool {
    !background || !store_open
}

/// Process arguments as strings. `std::env::args()` panics on a non-UTF-8 argument (any program that started the app can slip one in):
/// such an argument arrives with unreadable bytes replaced, and the flags do not change because of it.
pub fn launch_args() -> Vec<String> {
    lossy_args(std::env::args_os())
}

/// A startup error is not a panic: the cause (only the category) goes to the log, the process exits with a code. Tauri panics on a
/// `setup` error by itself, so the error is intercepted here, before it.
pub(crate) fn fail_fast(
    result: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = result {
        log::error!("startup failed: {}", startup_error_category(error.as_ref()));
        exit_after_failure();
    }
    Ok(())
}

pub(crate) fn exit_after_failure() -> ! {
    log::logger().flush();
    std::process::exit(EXIT_STARTUP_FAILED)
}

/// A safety net around call parsing: a panic in the synchronous part of the handler (argument parsing, a synchronous command on the main
/// thread, where unwinding the stack through an OS callback would abort the process) is logged, and the window gets an `internal` error
/// instead of a forever-waiting promise. The asynchronous part of commands runs in a tokio task: its panic kills only that task
/// (the process is alive, but there is no reply to the call -- the responder clone does not reach here), while the store's read, write and agent calls
/// catch the panic themselves and reply with an error (`kinige-store`).
pub fn guarded<R: Runtime>(
    handler: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let resolver = invoke.resolver.clone();
        panic::catch_unwind(AssertUnwindSafe(|| handler(invoke))).unwrap_or_else(|_| {
            log::error!("panic in the command handler, an error was sent to the window");
            reject_internal(resolver);
            true
        })
    }
}

/// Rejects the call with an `internal` error. If the reply went out before the panic, tauri panics by itself on the second reply: that is caught here.
fn reject_internal<R: Runtime>(resolver: InvokeResolver<R>) {
    let rejected = panic::catch_unwind(AssertUnwindSafe(|| {
        resolver.reject(kinige_domain::ApiError::from(ErrorCode::Internal));
    }));
    if rejected.is_err() {
        log::warn!("the call had already been answered before the panic");
    }
}

fn lossy_args<I: IntoIterator<Item = OsString>>(args: I) -> Vec<String> {
    args.into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// The startup error category for the log: OS and SQLite error texts contain paths and pieces of data.
fn startup_error_category(error: &(dyn std::error::Error + 'static)) -> &'static str {
    match error.downcast_ref::<StoreError>() {
        Some(store) => store.category(),
        None => "startup",
    }
}

#[cfg(test)]
mod tests {
    use super::{is_background_launch, should_show_main_on_launch, startup_error_category};
    use kinige_store::StoreError;

    #[test]
    fn a_background_launch_with_a_broken_database_still_shows_the_recovery_window() {
        // The MCP bridge started the app, the database is corrupt or newer than the app: without a window the process is invisible.
        assert!(should_show_main_on_launch(true, false));
    }

    #[test]
    fn a_background_launch_with_an_open_store_stays_in_the_menu_bar() {
        assert!(!should_show_main_on_launch(true, true));
    }

    #[test]
    fn an_ordinary_launch_always_shows_the_window() {
        assert!(should_show_main_on_launch(false, true));
        assert!(should_show_main_on_launch(false, false));
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[cfg(unix)]
    #[test]
    fn an_argument_that_is_not_utf8_does_not_panic_and_flags_still_work() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        // `std::env::args()` would panic on the second argument.
        let args = super::lossy_args([
            OsString::from("kinige"),
            OsString::from_vec(vec![b'-', 0xff, 0xfe, b'x']),
            OsString::from("--background"),
        ]);
        assert!(is_background_launch(args));
    }

    #[test]
    fn a_startup_error_is_logged_by_category_never_by_its_text() {
        let store: Box<dyn std::error::Error> =
            Box::new(StoreError::Corrupt("/Users/someone/kinige.db".into()));
        assert_eq!(startup_error_category(store.as_ref()), "corrupt");
        let other: Box<dyn std::error::Error> = "/Users/someone/data: нет доступа".into();
        assert_eq!(startup_error_category(other.as_ref()), "startup");
    }

    #[test]
    fn only_the_exact_flag_marks_a_background_launch() {
        assert!(is_background_launch(args(&["kinige", "--background"])));
        assert!(!is_background_launch(args(&["kinige"])));
        assert!(!is_background_launch(args(&[
            "kinige",
            "--background=1",
            "background"
        ])));
    }
}
