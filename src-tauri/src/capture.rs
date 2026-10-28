//! Окно быстрого захвата (ТЗ §5.1): показ и скрытие, команды окна, хоткей буфера. Правила дописывания и хранения — в
//! `kinige-domain` и `kinige-store`; здесь только окно и платформа.

use kinige_domain::{ApiError, CaptureOutcome, CaptureState, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, State};

pub const CAPTURE_LABEL: &str = "capture";
pub const MAIN_LABEL: &str = "main";
/// Событие главному окну: записи изменились (захват, дописывание, буфер). Слушает только `main`.
pub const ENTRIES_CHANGED: &str = "entries-changed";

/// Нижний край панели захвата стоит выше низа рабочей области монитора (без Dock и menu bar) на эту долю её высоты. Панель
/// внизу экрана, как поле чата: она не закрывает то, о чём человек хочет записать, а поле ввода всегда на одном месте. Нижний
/// край закреплён при любой высоте окна: контекст и многострочный текст растут вверх.
const BOTTOM_FRACTION: f64 = 0.12;
/// Размеры окна захвата в логических px. Совпадают с токенами `layout.capture.*` (проверяет `tests/config.rs`): ширина
/// окна = ширина панели + два поля под тень; высоту подгоняет страница командой `capture_resize` в этих пределах.
pub const WINDOW_WIDTH: f64 = 584.0;
pub const SHADOW_MARGIN: f64 = 32.0;
pub const MIN_HEIGHT: f64 = 100.0;
pub const MAX_HEIGHT: f64 = 640.0;
/// Сколько ждать, пока окно захвата само сохранит черновик и спрячется, прежде чем спрятать его принудительно.
const HIDE_GRACE: Duration = Duration::from_millis(600);

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    log::error!("ошибка хранилища: {}", error.category());
    error.code().into()
}

/// Состояние окна захвата.
#[derive(Default)]
pub struct CaptureShell {
    /// Текст буфера и ошибка при сбое записи: окно открывается с ними один раз.
    pending: Mutex<Option<Pending>>,
    /// Растёт при каждом показе: отложенное принудительное скрытие не должно спрятать окно, показанное заново.
    shown: AtomicU64,
    /// Якорь окна на экране при последнем показе: `(x левого края, y нижнего края окна)` в физических px. Нижний край
    /// остаётся на месте при любой высоте: и при показе, и при `capture_resize` (размер на главном потоке ставится
    /// отложенно, поэтому положение считается от якоря, а не от прочитанного размера окна).
    anchor: Mutex<Option<(i32, i32)>>,
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

/// Сообщает главному окну, что записи изменились, и обновляет «последние чаты» в меню значка.
pub fn notify_entries_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit_to(MAIN_LABEL, ENTRIES_CHANGED, ());
    crate::tray::refresh(app);
}

/// Событие главному окну: открыть чат (из меню значка). Слушает только `main`; в нагрузке — id чата.
pub const OPEN_CHAT: &str = "open-chat";

/// Показывает главное окно сразу с этим чатом (меню значка, слайс 3).
pub fn open_chat<R: Runtime>(app: &AppHandle<R>, chat_id: &str) {
    show_main(app);
    let _ = app.emit_to(MAIN_LABEL, OPEN_CHAT, chat_id.to_string());
}

/// Высота окна из просьбы страницы: в пределах `MIN_HEIGHT … MAX_HEIGHT`, целое число px.
pub fn clamp_height(height: f64) -> f64 {
    if height.is_finite() {
        height.round().clamp(MIN_HEIGHT, MAX_HEIGHT)
    } else {
        MIN_HEIGHT
    }
}

/// Якорь окна для рабочей области монитора `(x, y, ширина, высота)` (физические px): `(x левого края, y нижнего края окна)`.
/// По центру по горизонтали (`window_width` — ширина окна); нижний край панели выше низа рабочей области на `BOTTOM_FRACTION`
/// её высоты, а окно ниже панели на поле под тень `shadow_margin`.
pub fn anchor_for(work: (i32, i32, u32, u32), window_width: u32, shadow_margin: f64) -> (i32, i32) {
    let (work_x, work_y, work_width, work_height) = work;
    let x = f64::from(work_x) + (f64::from(work_width) - f64::from(window_width)) / 2.0;
    let panel_bottom = f64::from(work_y) + f64::from(work_height) * (1.0 - BOTTOM_FRACTION);
    (
        x.round() as i32,
        (panel_bottom + shadow_margin).round() as i32,
    )
}

/// Верх окна (физические px) при высоте `height`, если нижний край окна стоит на `bottom`: нижний край закреплён.
pub fn top_for_bottom(bottom: i32, height: u32) -> i32 {
    bottom.saturating_sub(i32::try_from(height).unwrap_or(i32::MAX))
}

/// Ставит окно на экран с курсором внизу рабочей области (без Dock и menu bar) по центру и запоминает якорь. Высота окна
/// влияет только на верх окна: нижний край панели всегда на одном месте.
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
    let work = monitor.work_area();
    let scale = window.scale_factor().unwrap_or(1.0);
    let (x, bottom) = anchor_for(
        (
            work.position.x,
            work.position.y,
            work.size.width,
            work.size.height,
        ),
        size.width,
        SHADOW_MARGIN * scale,
    );
    if let Some(shell) = app.try_state::<CaptureShell>() {
        *shell
            .anchor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((x, bottom));
    }
    trace(&format!(
        "place x={x} bottom={bottom} height={}",
        size.height
    ));
    let _ = window.set_position(PhysicalPosition::new(
        x,
        top_for_bottom(bottom, size.height),
    ));
}

/// Журнал жизни окна захвата для проверки в живом окне: пишется в stderr только при `KINIGE_TRACE=1` (без текста записей).
pub fn trace(what: &str) {
    if log::log_enabled!(target: crate::logging::TRACE_TARGET, log::Level::Debug) {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() % 100_000);
        log::debug!(target: crate::logging::TRACE_TARGET, "{ms:05} {what}");
    }
}

/// Показывает окно захвата поверх всех окон на экране с курсором. Окно создано скрытым при старте: показ мгновенный.
/// На macOS это панель без активации приложения (`panel`): прежняя программа остаётся активной, окно получает ввод.
///
/// Любой вызов AppKit (панель, окно) — только на главном потоке: функция безопасна из любого потока, показ выполняется на главном.
pub fn show_capture<R: Runtime>(app: &AppHandle<R>) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || show_capture_on_main(&main_app));
}

fn show_capture_on_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if let Some(shell) = app.try_state::<CaptureShell>() {
        shell.shown.fetch_add(1, Ordering::SeqCst);
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
/// возвращается ей сам. Безопасна из любого потока (скрытие выполняется на главном).
pub fn hide_capture<R: Runtime>(app: &AppHandle<R>) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || hide_capture_on_main(&main_app));
}

fn hide_capture_on_main<R: Runtime>(app: &AppHandle<R>) {
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

/// Окно захвата стало ключевым (только метка в журнале `KINIGE_TRACE`).
pub fn on_capture_key<R: Runtime>(_app: &AppHandle<R>) {
    trace("key");
}

/// Окно захвата потеряло ключевой статус (щелчок в другое окно, другая программа): прячется с сохранением черновика
/// (§5.1). Если окно уже спрятано, `request_hide` ничего не делает.
pub fn on_capture_blur<R: Runtime>(app: &AppHandle<R>) {
    trace("resign");
    request_hide(app);
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
                log::error!("сохранение буфера не удалось: {}", error.category());
                open_with_failed_clipboard(&app, text, error.code());
            }
        }
    });
}

/// Сбой записи буфера: окно захвата открывается с текстом буфера в поле и ошибкой (ТЗ §1.2 п. 9). Прежний непустой черновик
/// окна не затирается: текст буфера идёт под ним (`joined_draft`), и в черновик, и в память — черновик не должен потеряться,
/// если окно закроют, не исправив ошибку.
fn open_with_failed_clipboard<R: Runtime>(app: &AppHandle<R>, text: String, error: ErrorCode) {
    if let Some(shell) = app.try_state::<CaptureShell>() {
        let store = app.state::<Store>();
        let joined = match store.merge_clipboard_into_capture_draft(&text, &Now::system()) {
            Ok((joined, _saved)) => joined,
            // База не пишет (из-за этого, возможно, и случился сбой): поле собирается из того, что читается.
            Err(_) => kinige_domain::draft::joined_draft(
                &store.get_capture_draft().unwrap_or_default(),
                &text,
            ),
        };
        *shell.pending() = Some(Pending {
            text: joined,
            error,
        });
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
        recent_emoji: settings.recent_emoji,
    })
}

/// Выбранный в пикере эмодзи запоминается в общем списке «недавних» (тот же, что у главного окна); ответ — новый список.
/// Команда не принимает ни чата, ни записи: меняет только этот список (слайс 3, решение приёмки 27.10.2026).
#[tauri::command(async)]
pub fn capture_add_recent_emoji(store: State<'_, Store>, emoji: String) -> ApiResult<Vec<String>> {
    store.add_recent_emoji(&emoji).map_err(api)
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

/// Подогнать высоту окна под содержимое страницы (логические px, вместе с полем под тень). Ширина фиксирована, а **нижний
/// край окна остаётся на месте** (окно растёт вверх, как чат: поле внизу, записи над ним), поэтому поле ввода всегда на одном
/// месте. Значение ограничивается пределами `MIN_HEIGHT … MAX_HEIGHT`; лишних просьб (та же высота) Rust не исполняет.
#[tauri::command]
pub fn capture_resize<R: Runtime>(app: AppHandle<R>, height: f64) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    let height = clamp_height(height);
    let scale = window.scale_factor().unwrap_or(1.0);
    let current = window
        .inner_size()
        .map(|size| f64::from(size.height) / scale);
    if current.is_ok_and(|current| (current - height).abs() < 0.5) {
        return;
    }
    let _ = window.set_size(tauri::LogicalSize::new(WINDOW_WIDTH, height));
    // `set_size` на macOS задаёт размер содержимого и не обещает, какой угол остаётся на месте: положение пересчитывается
    // явно от якоря показа (нижний край окна и x). Якоря нет (окно ещё не показывали) — позицию поставит показ.
    let anchor = app.try_state::<CaptureShell>().and_then(|shell| {
        *shell
            .anchor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    if let Some((x, bottom)) = anchor {
        let new_height = (height * scale).round() as u32;
        trace(&format!("resize height={height} bottom={bottom}"));
        let _ = window.set_position(PhysicalPosition::new(x, top_for_bottom(bottom, new_height)));
    }
}
