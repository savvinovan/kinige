//! Окно быстрого захвата (ТЗ §5.1): показ и скрытие, команды окна, хоткей буфера. Правила дописывания и хранения — в
//! `kinige-domain` и `kinige-store`; здесь только окно и платформа.

use kinige_domain::{ApiError, CaptureOutcome, CaptureState, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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
/// Сколько панель должна продержаться ключевой, чтобы следующая потеря статуса считалась уходом пользователя, а не
/// действием системы: сразу после показа система, сообщая о смене активной программы, снимает статус через несколько
/// миллисекунд.
const STABLE_KEY: Duration = Duration::from_millis(300);
/// Сколько раз после показа возвращать панели ключевой статус, прежде чем сдаться (панель остаётся на экране, в неё можно щёлкнуть).
const MAX_REKEYS: u32 = 20;

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    eprintln!("kinige: ошибка хранилища: {}", error.category());
    error.code().into()
}

/// Состояние окна захвата.
#[derive(Default)]
pub struct CaptureShell {
    /// Текст буфера и ошибка при сбое записи: окно открывается с ними один раз.
    pending: Mutex<Option<Pending>>,
    /// Растёт при каждом показе: отложенное принудительное скрытие не должно спрятать окно, показанное заново.
    shown: AtomicU64,
    /// Когда панель в последний раз стала ключевой (`None` — с показа не становилась).
    key_since: Mutex<Option<Instant>>,
    /// Сколько раз с показа панели возвращали ключевой статус.
    rekeys: AtomicU32,
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

/// Журнал жизни окна захвата для проверки в живом окне: печатается только при `KINIGE_TRACE=1` (без текста записей).
pub fn trace(what: &str) {
    if std::env::var_os("KINIGE_TRACE").is_some() {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() % 100_000);
        eprintln!("kinige-trace {ms:05} {what}");
    }
}

/// Показывает окно захвата поверх всех окон на экране с курсором. Окно создано скрытым при старте: показ мгновенный.
/// На macOS это панель без активации приложения (`panel`): прежняя программа остаётся активной, окно получает ввод.
pub fn show_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if let Some(shell) = app.try_state::<CaptureShell>() {
        shell.shown.fetch_add(1, Ordering::SeqCst);
        shell.rekeys.store(0, Ordering::SeqCst);
        *shell
            .key_since
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    trace("show");
    place_on_cursor_screen(app, &window);
    #[cfg(target_os = "macos")]
    let as_panel = crate::panel::show(app, CAPTURE_LABEL);
    #[cfg(not(target_os = "macos"))]
    let as_panel = false;
    if !as_panel {
        let _ = window.show();
        let _ = window.set_focus();
        // Окно создано скрытым и не сфокусированным: без этого вызова клавиатура остаётся у окна, а не у страницы.
        let webview: &tauri::Webview<R> = window.as_ref();
        let _ = webview.set_focus();
    }
    // Страница узнаёт о показе без IPC (у окна нет права слушать события): перечитывает состояние и ставит курсор в поле.
    let _ = window.eval("window.dispatchEvent(new Event('kinige-capture-shown'))");
}

/// Прячет окно захвата. Приложение не активировалось (панель), поэтому прежняя программа остаётся активной и фокус
/// возвращается ей сам.
pub fn hide_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    trace("hide");
    #[cfg(target_os = "macos")]
    let as_panel = crate::panel::hide(app, CAPTURE_LABEL);
    #[cfg(not(target_os = "macos"))]
    let as_panel = false;
    if !as_panel {
        let _ = window.hide();
    }
}

/// Просит страницу окна сохранить черновик и спрятаться (`capture_hide`); если страница не ответила — прячет принудительно.
pub fn request_hide<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    // Спрятанное окно прятать не нужно (в том числе когда потеря ключевого статуса — следствие самого скрытия).
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    trace("request-hide");
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

/// Окно захвата стало ключевым.
pub fn on_capture_key<R: Runtime>(app: &AppHandle<R>) {
    trace("key");
    if let Some(shell) = app.try_state::<CaptureShell>() {
        *shell
            .key_since
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }
}

/// Окно захвата потеряло ключевой статус. Если до этого оно было ключевым достаточно долго — это уход пользователя (щелчок
/// в другое окно, другая программа): окно прячется с сохранением черновика (§5.1). Если оно не успело продержаться —
/// статус снимает система сразу после показа (смена активной программы): статус возвращается, несколько раз.
pub fn on_capture_blur<R: Runtime>(app: &AppHandle<R>) {
    trace("resign");
    let Some(shell) = app.try_state::<CaptureShell>() else {
        return;
    };
    let stable = shell
        .key_since
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some_and(|since| since.elapsed() >= STABLE_KEY);
    if stable {
        request_hide(app);
        return;
    }
    #[cfg(target_os = "macos")]
    if shell.rekeys.fetch_add(1, Ordering::SeqCst) < MAX_REKEYS {
        let app = app.clone();
        std::thread::spawn(move || {
            let main_app = app.clone();
            let _ =
                app.run_on_main_thread(move || crate::panel::make_key(&main_app, CAPTURE_LABEL));
        });
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
/// — окно захвата с текстом буфера и ошибкой. Успех — мигание значка в menu bar. Работает на отдельном потоке: чтение
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
    hide_capture(&app);
}
