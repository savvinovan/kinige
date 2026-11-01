//! Запуск приложения и страховка вокруг него (KINIG-28, ADR-0011): аргументы процесса, ошибка запуска без паники, обёртка
//! обработчика команд Tauri, которая превращает панику в ответ окну.

use kinige_domain::ErrorCode;
use kinige_store::StoreError;
use std::ffi::OsString;
use std::panic::{self, AssertUnwindSafe};
use tauri::Runtime;
use tauri::ipc::{Invoke, InvokeResolver};

/// Аргумент запуска мостом MCP: окно не показывается, приложение живёт в menu bar.
pub const BACKGROUND_FLAG: &str = "--background";

/// Код выхода, если приложение не смогло запуститься (ошибка запуска или сборки). Паники при этом нет.
const EXIT_STARTUP_FAILED: i32 = 1;

/// Запущено мостом MCP (`--background`)? Тогда нет ни окна, ни значка в Dock: приложение живёт в menu bar (политика
/// активации Accessory); первое же показанное окно возвращает обычную политику (`capture::show_main`).
pub fn is_background_launch<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|arg| arg == BACKGROUND_FLAG)
}

/// Показывать ли главное окно при запуске. Обычный запуск показывает всегда. Запуск мостом MCP (`--background`) живёт в
/// menu bar без окна, но только если журнал открыт: при повреждённой или более новой базе (`store_open = false`) в menu bar
/// сидел бы невидимый процесс, а экран восстановления (`recovery`) — единственное место, где человек видит, что случилось.
pub fn should_show_main_on_launch(background: bool, store_open: bool) -> bool {
    !background || !store_open
}

/// Аргументы процесса строками. `std::env::args()` паникует на аргументе не в UTF-8 (его может подсунуть любая программа,
/// запустившая приложение): такой аргумент приходит с заменой нечитаемых байтов, флаги от этого не меняются.
pub fn launch_args() -> Vec<String> {
    lossy_args(std::env::args_os())
}

/// Ошибка запуска не паника: причина (только категория) уходит в журнал, процесс завершается кодом. Tauri на ошибке
/// `setup` паникует сам, поэтому ошибка перехватывается здесь, до него.
pub(crate) fn fail_fast(
    result: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = result {
        log::error!(
            "запуск не удался: {}",
            startup_error_category(error.as_ref())
        );
        exit_after_failure();
    }
    Ok(())
}

pub(crate) fn exit_after_failure() -> ! {
    log::logger().flush();
    std::process::exit(EXIT_STARTUP_FAILED)
}

/// Страховка вокруг разбора вызова: паника в синхронной части обработчика (разбор аргументов, синхронная команда на главном
/// потоке, где размотка стека через обратный вызов ОС оборвала бы процесс) логируется, а окно получает ошибку `internal`
/// вместо вечно ждущего промиса. Асинхронная часть команд идёт в задаче tokio: её паника убивает только эту задачу
/// (процесс жив, но ответа на вызов нет — клон ответчика сюда не дотягивается), а чтение, запись и агентские вызовы
/// хранилища ловят панику сами и отвечают ошибкой (`kinige-store`).
pub fn guarded<R: Runtime>(
    handler: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let resolver = invoke.resolver.clone();
        panic::catch_unwind(AssertUnwindSafe(|| handler(invoke))).unwrap_or_else(|_| {
            log::error!("паника в обработчике команды, окну отправлена ошибка");
            reject_internal(resolver);
            true
        })
    }
}

/// Отклоняет вызов ошибкой `internal`. Если ответ ушёл до паники, tauri при втором ответе паникует сам: это ловится здесь.
fn reject_internal<R: Runtime>(resolver: InvokeResolver<R>) {
    let rejected = panic::catch_unwind(AssertUnwindSafe(|| {
        resolver.reject(kinige_domain::ApiError::from(ErrorCode::Internal));
    }));
    if rejected.is_err() {
        log::warn!("вызов уже получил ответ до паники");
    }
}

fn lossy_args<I: IntoIterator<Item = OsString>>(args: I) -> Vec<String> {
    args.into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// Категория ошибки запуска для журнала: тексты ошибок ОС и SQLite содержат пути и куски данных.
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
        // Мост MCP запустил приложение, база повреждена или новее приложения: без окна процесс невидим.
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
        // `std::env::args()` паниковал бы на втором аргументе.
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
