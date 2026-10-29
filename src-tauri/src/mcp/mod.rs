//! Сервер MCP внутри приложения (ADR-0009, слайс 4a): `kinige-mcp` — мост stdio <-> Unix-сокет, база принадлежит одному
//! писателю приложения. Слушатель стартует ПОСЛЕДНИМ в `start_services`, после `app.manage(store)`; сбой сокета журнал не
//! останавливает: ошибка видна в окне «Доступ ИИ-агентов». Сокет удаляется в `RunEvent::Exit`.
//!
//! Слушатель живёт под присмотром (`supervise`): первый `bind` не удался — привязка повторяется раз в интервал проверки;
//! слушатель упал с паникой (профиль release с размоткой стека) — ошибка в состоянии и перезапуск.

mod connect;
mod server;
mod socket;
mod tools;

pub use connect::socket_info;
pub use socket::{FRAME_LIMIT, Limited, SocketError, bind, bind_with, own_uid, socket_path};

use rmcp::serve_server;
use server::{ConnectionInfo, McpServer, disconnected};
use socket::socket_present;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

/// Как часто слушатель проверяет, что файл сокета на месте (macOS может вычищать старые файлы в `$TMPDIR`), а после
/// неудачного `bind` — повторяет привязку.
const RECHECK: Duration = Duration::from_secs(10 * 60);

/// Первая строка соединения после обрыва, которую шлёт мост (см. `kinige-mcp`): сеанс клиента тот же, строка журнала —
/// «переподключение», а не «подключение».
const RECONNECT_MARKER: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"kinige/reconnect\"}\n";

/// Состояние сервера MCP в приложении: путь сокета и почему он не слушает.
pub struct McpState {
    path: PathBuf,
    error: Mutex<Option<SocketError>>,
    recheck: Duration,
    /// Только для тестов: слушатель паникует при ближайшем соединении.
    panic_next: AtomicBool,
}

impl McpState {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn error(&self) -> Option<SocketError> {
        match self.error.lock() {
            Ok(error) => *error,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    fn set_error(&self, error: Option<SocketError>) {
        match self.error.lock() {
            Ok(mut slot) => *slot = error,
            Err(poisoned) => *poisoned.into_inner() = error,
        }
    }

    /// Только для тестов: следующее принятое соединение роняет слушатель паникой (присмотр его перезапустит).
    #[doc(hidden)]
    pub fn inject_listener_panic_for_test(&self) {
        self.panic_next.store(true, Ordering::SeqCst);
    }
}

/// Обёртка для `app.manage`: общий доступ к состоянию из задач и команд.
pub struct McpHandle(pub Arc<McpState>);

/// Запускает слушатель. Всегда возвращается: ошибка сокета записывается в состояние (`McpState::error`).
/// Вызывается из `start_services` последним, на главном потоке.
pub fn start<R: Runtime>(app: &AppHandle<R>, data_dir: &Path) {
    start_with_recheck(app, data_dir, RECHECK);
}

/// То же с заданным интервалом проверки (тесты укорачивают его).
pub fn start_with_recheck<R: Runtime>(app: &AppHandle<R>, data_dir: &Path, recheck: Duration) {
    if app.try_state::<McpHandle>().is_some() {
        return;
    }
    let path = socket_path(data_dir, &std::env::temp_dir());
    let state = Arc::new(McpState {
        path: path.clone(),
        error: Mutex::new(None),
        recheck,
        panic_next: AtomicBool::new(false),
    });
    app.manage(McpHandle(state.clone()));
    let Ok(uid) = tauri::async_runtime::block_on(async { own_uid() }) else {
        state.set_error(Some(SocketError::BindFailed));
        return;
    };
    // `UnixListener` tokio привязывается к реактору: `bind` вызывается внутри среды выполнения.
    let first = tauri::async_runtime::block_on(async { bind(&path, uid) });
    match &first {
        Ok(_) => log::info!("mcp: сокет поднят"),
        Err(error) => {
            log::warn!("mcp: сокет не поднят: {}", error.code());
            state.set_error(Some(*error));
        }
    }
    // Присмотр живёт и после неудачного `bind`: условие (папка, второй экземпляр) может исчезнуть.
    tauri::async_runtime::spawn(supervise(app.clone(), state, first.ok(), uid));
}

/// Удаляет файл сокета при выходе из приложения.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(handle) = app.try_state::<McpHandle>()
        && handle.0.error().is_none()
    {
        let _ = std::fs::remove_file(handle.0.path());
    }
}

/// Присмотр: пока слушателя нет — раз в интервал пробует привязаться; слушатель упал с паникой — ошибка в состоянии и
/// перезапуск. Сам не возвращается.
async fn supervise<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<McpState>,
    mut listener: Option<UnixListener>,
    uid: u32,
) {
    loop {
        let Some(live) = listener.take() else {
            tokio::time::sleep(state.recheck).await;
            match bind(state.path(), uid) {
                Ok(fresh) => {
                    listener = Some(fresh);
                    state.set_error(None);
                    log::info!("mcp: сокет поднят");
                }
                Err(error) => state.set_error(Some(error)),
            }
            continue;
        };
        let task = tauri::async_runtime::spawn(run_listener(app.clone(), state.clone(), live, uid));
        // `run_listener` не возвращается сам: конец — только паника (или остановка среды выполнения).
        if task.await.is_err() {
            log::error!("mcp: слушатель упал с паникой, перезапуск");
            state.set_error(Some(SocketError::BindFailed));
            // Прежний слушатель уничтожен вместе с задачей; файл сокета остался, `bind` его заменит.
            listener = bind(state.path(), uid).ok();
            if listener.is_some() {
                state.set_error(None);
            }
        } else {
            return;
        }
    }
}

async fn run_listener<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<McpState>,
    mut listener: UnixListener,
    uid: u32,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    assert!(
                        !state.panic_next.swap(false, Ordering::SeqCst),
                        "проверка: паника слушателя"
                    );
                    tauri::async_runtime::spawn(serve_connection(app.clone(), stream, uid));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
            () = tokio::time::sleep(state.recheck) => {
                if socket_present(state.path()) {
                    continue;
                }
                // Файл сокета пропал (его вычистила система или удалили): те же проверки и тот же путь.
                match bind(state.path(), uid) {
                    Ok(fresh) => {
                        listener = fresh;
                        state.set_error(None);
                        log::info!("mcp: сокет перепривязан");
                    }
                    Err(error) => {
                        log::warn!("mcp: сокет не перепривязан: {}", error.code());
                        state.set_error(Some(error));
                    }
                }
            }
        }
    }
}

/// Читает начало соединения и решает, маркер ли это переподключения. Читает не больше длины маркера и только пока
/// прочитанное — его начало (маркер может прийти частями); всё лишнее возвращается серверу как начало потока.
async fn read_marker<R: AsyncReadExt + Unpin>(read: &mut R) -> std::io::Result<(bool, Vec<u8>)> {
    let mut head: Vec<u8> = Vec::with_capacity(RECONNECT_MARKER.len());
    let mut chunk = [0u8; 256];
    while head.len() < RECONNECT_MARKER.len()
        && RECONNECT_MARKER.starts_with(&head[..head.len().min(RECONNECT_MARKER.len())])
    {
        let n = read.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        head.extend_from_slice(&chunk[..n]);
    }
    if head.starts_with(RECONNECT_MARKER) {
        Ok((true, head.split_off(RECONNECT_MARKER.len())))
    } else {
        Ok((false, head))
    }
}

/// Соединение: чужой uid закрывается, предел кадра ставится на читающую половину, затем сервис `rmcp`. `own_uid` приходит
/// снаружи, чтобы тест мог подменить его и проверить закрытие соединения чужого пользователя.
pub async fn serve_connection<R: Runtime>(app: AppHandle<R>, stream: UnixStream, own_uid: u32) {
    match stream.peer_cred() {
        Ok(cred) if cred.uid() == own_uid => {}
        _ => {
            log::warn!("mcp: соединение чужого пользователя закрыто");
            return;
        }
    }
    let (mut read, write) = stream.into_split();
    // Мост после обрыва начинает соединение с маркера: сеанс клиента тот же, в журнале — «переподключение».
    let Ok((reconnect, rest)) = read_marker(&mut read).await else {
        return;
    };
    let reader = Cursor::new(rest).chain(read);
    let info = Arc::new(ConnectionInfo::default());
    let handler = McpServer {
        app: app.clone(),
        reconnect,
        info: info.clone(),
    };
    match serve_server(handler, (Limited::new(reader, FRAME_LIMIT), write)).await {
        Ok(running) => {
            let _ = running.waiting().await;
        }
        Err(_) => log::debug!("mcp: рукопожатие не состоялось"),
    }
    disconnected(&app, &info).await;
}
