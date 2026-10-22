//! Окно быстрого захвата (ТЗ §5.1): показ и скрытие, команды окна, хоткей буфера. Правила дописывания и хранения — в
//! `kinige-domain` и `kinige-store`; здесь только окно и платформа.

use kinige_domain::{ApiError, CaptureOutcome, CaptureState, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, State};

pub const CAPTURE_LABEL: &str = "capture";
pub const MAIN_LABEL: &str = "main";
/// Событие главному окну: записи изменились (захват, дописывание, буфер). Слушает только `main`.
pub const ENTRIES_CHANGED: &str = "entries-changed";

/// Доля высоты экрана над окном захвата: окно чуть выше середины, как у быстрых панелей.
const TOP_FRACTION: f64 = 0.28;
/// Сколько ждать, пока окно захвата само сохранит черновик и спрячется, прежде чем спрятать его принудительно.
const HIDE_GRACE: Duration = Duration::from_millis(600);
/// Сколько после показа потеря фокуса не считается уходом пользователя.
const BLUR_GRACE: Duration = Duration::from_millis(250);
/// Повторы `set_focus` после показа, если окно не получило фокус, и пауза перед каждым.
const FOCUS_RETRIES: usize = 3;
const FOCUS_RETRY_DELAY: Duration = Duration::from_millis(150);

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    eprintln!("kinige: ошибка хранилища: {}", error.category());
    error.code().into()
}

/// Состояние окна захвата.
#[derive(Default)]
pub struct CaptureShell {
    /// Приложение было активным, когда окно захвата показали: тогда при скрытии прячется только оно, иначе приложение
    /// скрывается целиком и фокус возвращается прежней программе.
    app_was_active: AtomicBool,
    /// Текст буфера и ошибка при сбое записи: окно открывается с ними один раз.
    pending: Mutex<Option<Pending>>,
    /// Растёт при каждом показе: отложенное принудительное скрытие не должно спрятать окно, показанное заново.
    shown: AtomicU64,
    /// Когда окно показано в последний раз: потеря фокуса в первые мгновения после показа — не уход пользователя.
    shown_at: Mutex<Option<Instant>>,
}

struct Pending {
    text: String,
    error: ErrorCode,
}

impl CaptureShell {
    fn pending(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Показывает главное окно (меню трея, значок в Dock, второй запуск).
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Сообщает главному окну, что записи изменились.
pub fn notify_entries_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit_to(MAIN_LABEL, ENTRIES_CHANGED, ());
}

fn app_is_active<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false))
}

/// Ставит окно на экран с курсором: по центру по горизонтали, выше середины по вертикали.
fn place_on_cursor_screen<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        let _ = window.center();
        return;
    };
    let origin = monitor.position();
    let area = monitor.size();
    let x = f64::from(origin.x) + (f64::from(area.width) - f64::from(size.width)) / 2.0;
    let y = f64::from(origin.y) + (f64::from(area.height) - f64::from(size.height)) * TOP_FRACTION;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Показывает окно захвата поверх всех окон на экране с курсором. Окно создано скрытым при старте: показ мгновенный.
pub fn show_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if let Some(shell) = app.try_state::<CaptureShell>() {
        shell
            .app_was_active
            .store(app_is_active(app), Ordering::SeqCst);
        shell.shown.fetch_add(1, Ordering::SeqCst);
        *shell
            .shown_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }
    place_on_cursor_screen(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
    // Окно создано скрытым и не сфокусированным: без этого вызова клавиатура остаётся у окна, а не у страницы (курсор в
    // поле виден, но символы не печатаются).
    let webview: &tauri::Webview<R> = window.as_ref();
    let _ = webview.set_focus();
    // Страница узнаёт о показе без IPC (у окна нет права слушать события): перечитывает состояние и ставит курсор в поле.
    let _ = window.eval("window.dispatchEvent(new Event('kinige-capture-shown'))");
    // Система может отдать фокус не сразу (macOS 14+ активирует приложения «кооперативно»): несколько повторов.
    std::thread::spawn(move || {
        for _ in 0..FOCUS_RETRIES {
            std::thread::sleep(FOCUS_RETRY_DELAY);
            if window.is_focused().unwrap_or(true) || !window.is_visible().unwrap_or(false) {
                return;
            }
            let _ = window.set_focus();
            let webview: &tauri::Webview<R> = window.as_ref();
            let _ = webview.set_focus();
        }
    });
}

/// Прячет окно захвата. Если до показа активной была другая программа — приложение скрывается целиком (macOS возвращает
/// фокус прежней программе); окно скрывается первым, чтобы система не вернула его при следующей активации.
pub fn hide_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    eprintln!("TRACE hide_capture visible={:?}", window.is_visible());
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let _ = window.hide();
    let was_active = app
        .try_state::<CaptureShell>()
        .is_none_or(|shell| shell.app_was_active.load(Ordering::SeqCst));
    #[cfg(target_os = "macos")]
    if !was_active {
        let _ = app.hide();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = was_active;
}

/// Просит страницу окна сохранить черновик и спрятаться (`capture_hide`); если страница не ответила — прячет принудительно.
pub fn request_hide<R: Runtime>(app: &AppHandle<R>) {
    eprintln!("TRACE request_hide");
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    let _ = window.eval("window.dispatchEvent(new Event('kinige-capture-hide-request'))");
    let app = app.clone();
    let shown = app
        .try_state::<CaptureShell>()
        .map(|shell| shell.shown.load(Ordering::SeqCst));
    std::thread::spawn(move || {
        std::thread::sleep(HIDE_GRACE);
        let same_showing = app
            .try_state::<CaptureShell>()
            .map(|shell| shell.shown.load(Ordering::SeqCst))
            == shown;
        if same_showing {
            hide_capture(&app);
        }
    });
}

/// Окно захвата потеряло фокус (щелчок в другое окно, другая программа): прячется с сохранением черновика (§5.1). Потеря
/// фокуса в первые мгновения после показа игнорируется: система может отдать фокус не сразу.
pub fn on_capture_blur<R: Runtime>(app: &AppHandle<R>) {
    let recently_shown = app.try_state::<CaptureShell>().is_some_and(|shell| {
        shell
            .shown_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < BLUR_GRACE)
    });
    if !recently_shown {
        request_hide(app);
    }
}

/// Хоткей захвата: окно закрыто — показать, открыто — спрятать с сохранением черновика.
pub fn toggle_capture<R: Runtime>(app: &AppHandle<R>) {
    let visible = app
        .get_webview_window(CAPTURE_LABEL)
        .is_some_and(|window| window.is_visible().unwrap_or(false));
    if visible {
        request_hide(app);
    } else {
        show_capture(app);
    }
}

/// Хоткей буфера (ТЗ §5.1): текст из буфера обмена — новая запись в Inbox, без окна. Не текст или пусто — ничего. Сбой записи
/// — окно захвата с текстом буфера в поле и ошибкой. Успех — мигание значка в menu bar. Работает на отдельном потоке: чтение
/// буфера и запись в базу не должны задерживать цикл событий.
pub fn save_clipboard<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        let text = match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            Ok(text) => text,
            // Не текст (картинка, файлы — слайс 7), пустой буфер или буфер занят другой программой: ничего не пишем.
            Err(_) => return,
        };
        let store = app.state::<Store>();
        match store.save_clipboard(&text, &Now::system()) {
            Ok(_) => {
                notify_entries_changed(&app);
                crate::tray::flash(&app);
            }
            Err(StoreError::Empty) => {}
            Err(error) => {
                eprintln!("kinige: сохранение буфера не удалось: {}", error.category());
                open_with_failed_clipboard(&app, text, error.code());
            }
        }
    });
}

/// Сбой записи буфера: окно захвата открывается с текстом буфера в поле и ошибкой (ТЗ §1.2 п. 9). Текст кладётся и в черновик
/// (если база пишет), и в память: черновик не должен потеряться, если окно закроют, не исправив ошибку.
fn open_with_failed_clipboard<R: Runtime>(app: &AppHandle<R>, text: String, error: ErrorCode) {
    if let Some(shell) = app.try_state::<CaptureShell>() {
        let _ = app
            .state::<Store>()
            .set_capture_draft(&text, &Now::system());
        *shell.pending() = Some(Pending { text, error });
    }
    show_capture(app);
}

/// Состояние окна при показе: язык, размер текста, контекст за сегодня, цель дописывания, черновик и, если окно открыто из-за
/// сбоя буфера, текст буфера с ошибкой (один раз).
#[tauri::command(async)]
pub fn capture_state(
    store: State<'_, Store>,
    shell: State<'_, CaptureShell>,
) -> ApiResult<CaptureState> {
    let view = store.capture_view(&Now::system()).map_err(api)?;
    let settings = store.get_settings().map_err(api)?;
    let pending = shell.pending().take();
    let (draft, error) = match pending {
        Some(Pending { text, error }) => (text, Some(error)),
        None => (view.draft, None),
    };
    Ok(CaptureState {
        language: crate::commands::get_language(),
        text_size: settings.text_size,
        today: view.today,
        context: view.context,
        append_target: view.append_target,
        draft,
        error,
    })
}

/// Enter: новая запись в Inbox; главное окно получает событие.
#[tauri::command(async)]
pub fn capture_send<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    body: String,
) -> ApiResult<CaptureOutcome> {
    let outcome = store.capture_send(&body, &Now::system()).map_err(api)?;
    notify_entries_changed(&app);
    Ok(outcome)
}

/// ⌘Enter: дописать к последней записи или, если цели нет, новая запись; ответ сообщает, что сделано.
#[tauri::command(async)]
pub fn capture_append<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    body: String,
) -> ApiResult<CaptureOutcome> {
    let outcome = store.capture_append(&body, &Now::system()).map_err(api)?;
    notify_entries_changed(&app);
    Ok(outcome)
}

#[tauri::command(async)]
pub fn capture_save_draft(store: State<'_, Store>, body: String) -> ApiResult<()> {
    store.set_capture_draft(&body, &Now::system()).map_err(api)
}

/// Спрятать окно захвата (после отправки, Esc, потери фокуса).
#[tauri::command]
pub fn capture_hide<R: Runtime>(app: AppHandle<R>) {
    eprintln!("TRACE capture_hide called");
    hide_capture(&app);
}
