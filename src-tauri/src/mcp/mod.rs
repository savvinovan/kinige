//! Сервер MCP внутри приложения (ADR-0009, слайс 4a): `kinige-mcp` — мост stdio <-> Unix-сокет, база принадлежит одному
//! писателю приложения. Слушатель стартует ПОСЛЕДНИМ в `start_services`, после `app.manage(store)`; сбой сокета журнал не
//! останавливает: ошибка видна в окне «Доступ ИИ-агентов». Сокет удаляется в `RunEvent::Exit`.

mod connect;
mod server;
mod socket;
mod tools;

pub use connect::socket_info;
pub use socket::{FRAME_LIMIT, Limited, SocketError, bind, bind_with, own_uid, socket_path};

use rmcp::serve_server;
use server::{ConnectionInfo, McpServer, disconnected};
use socket::socket_present;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// Как часто слушатель проверяет, что файл сокета на месте (macOS может вычищать старые файлы в `$TMPDIR`).
const RECHECK: Duration = Duration::from_secs(10 * 60);
/// Укорочение проверки в тестах, мс.
const RECHECK_ENV: &str = "KINIGE_MCP_RECHECK_MS";

/// Первая строка соединения после обрыва, которую шлёт мост (см. `kinige-mcp`): «подключение» в журнал не пишется.
const RECONNECT_MARKER: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"kinige/reconnect\"}\n";

/// Состояние сервера MCP в приложении: путь сокета и почему он не слушает.
pub struct McpState {
    path: PathBuf,
    error: Mutex<Option<SocketError>>,
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
}

/// Запускает слушатель. Всегда возвращается: ошибка сокета записывается в состояние (`McpState::error`).
/// Вызывается из `start_services` последним, на главном потоке.
pub fn start<R: Runtime>(app: &AppHandle<R>, data_dir: &Path) {
    if app.try_state::<McpHandle>().is_some() {
        return;
    }
    let path = socket_path(data_dir, &std::env::temp_dir());
    let state = Arc::new(McpState {
        path: path.clone(),
        error: Mutex::new(None),
    });
    app.manage(McpHandle(state.clone()));
    let bound = tauri::async_runtime::block_on(async {
        let uid = own_uid().map_err(|_| SocketError::BindFailed)?;
        bind(&path, uid).map(|listener| (listener, uid))
    });
    match bound {
        Ok((listener, uid)) => {
            log::info!("mcp: сокет поднят");
            let app = app.clone();
            tauri::async_runtime::spawn(run_listener(app, state, listener, uid));
        }
        Err(error) => {
            log::warn!("mcp: сокет не поднят: {}", error.code());
            state.set_error(Some(error));
        }
    }
}

/// Обёртка для `app.manage`: общий доступ к состоянию из задач и команд.
pub struct McpHandle(pub Arc<McpState>);

/// Удаляет файл сокета при выходе из приложения.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(handle) = app.try_state::<McpHandle>()
        && handle.0.error().is_none()
    {
        let _ = std::fs::remove_file(handle.0.path());
    }
}

async fn run_listener<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<McpState>,
    mut listener: UnixListener,
    uid: u32,
) {
    let recheck = std::env::var(RECHECK_ENV)
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .map_or(RECHECK, Duration::from_millis);
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    tauri::async_runtime::spawn(serve_connection(app.clone(), stream, uid));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
            () = tokio::time::sleep(recheck) => {
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
    let (read, write) = stream.into_split();
    let mut reader = BufReader::new(read);
    // Мост после обрыва начинает соединение с маркера: сеанс клиента тот же, второе «подключение» в журнале не нужно.
    let reconnect = match reader.fill_buf().await {
        Ok(first) if first.starts_with(RECONNECT_MARKER) => {
            reader.consume(RECONNECT_MARKER.len());
            true
        }
        Ok(_) => false,
        Err(_) => return,
    };
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
