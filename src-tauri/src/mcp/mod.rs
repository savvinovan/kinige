//! The MCP server inside the app (ADR-0009, slice 4a): `kinige-mcp` is a stdio <-> Unix socket bridge, the database belongs to the one
//! writer of the app. The listener starts LAST in `start_services`, after `app.manage(store)`; a socket failure does not
//! stop the journal: the error is visible in the "AI agent access" window. The socket is removed on `RunEvent::Exit`.
//!
//! The listener lives under supervision (`supervise`): if the first `bind` failed, the binding is retried once per check interval;
//! if the listener panicked (the release profile with stack unwinding), an error goes into the state and it is restarted.

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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

/// How often the listener checks that the socket file is in place (macOS may clean old files in `$TMPDIR`), and after
/// a failed `bind` retries the binding.
const RECHECK: Duration = Duration::from_secs(10 * 60);

/// The first line of a connection after a drop, sent by the bridge (see `kinige-mcp`): the client session is the same, the log row is
/// "reconnect", not "connect".
const RECONNECT_MARKER: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"kinige/reconnect\"}\n";

/// The MCP server state in the app: the socket path and why it is not listening.
pub struct McpState {
    path: PathBuf,
    error: Mutex<Option<SocketError>>,
    recheck: Duration,
    /// For tests only: the listener panics on the nearest connection.
    panic_next: AtomicBool,
    /// How many listeners have been bound and made ready (`bind` returned, including `chmod 0600`). Tests wait for it to
    /// grow instead of sleeping or polling the file system.
    binds: AtomicU64,
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

    /// For tests only: the next accepted connection brings the listener down with a panic (supervision will restart it).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn inject_listener_panic_for_test(&self) {
        self.panic_next.store(true, Ordering::SeqCst);
    }

    /// Test hook: the number of listeners bound so far. A new value means the socket file is final (0600) and the previous
    /// listener, if any, is already gone, so a client that connects now reaches the new one.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn binds(&self) -> u64 {
        self.binds.load(Ordering::SeqCst)
    }

    fn bound(&self) {
        self.binds.fetch_add(1, Ordering::SeqCst);
    }
}

/// A wrapper for `app.manage`: shared access to the state from tasks and commands.
pub struct McpHandle(pub Arc<McpState>);

/// Starts the listener. It always returns: a socket error is recorded in the state (`McpState::error`).
/// Called from `start_services` last, on the main thread.
pub fn start<R: Runtime>(app: &AppHandle<R>, data_dir: &Path) {
    start_with_recheck(app, data_dir, RECHECK);
}

/// The same with a given check interval (tests shorten it).
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
        binds: AtomicU64::new(0),
    });
    app.manage(McpHandle(state.clone()));
    let Ok(uid) = tauri::async_runtime::block_on(async { own_uid() }) else {
        state.set_error(Some(SocketError::BindFailed));
        return;
    };
    // The tokio `UnixListener` binds to the reactor: `bind` is called inside the runtime.
    let first = tauri::async_runtime::block_on(async { bind(&path, uid) });
    match &first {
        Ok(_) => {
            state.bound();
            log::info!("mcp: socket is up");
        }
        Err(error) => {
            log::warn!("mcp: socket is not up: {}", error.code());
            state.set_error(Some(*error));
        }
    }
    // Supervision lives on after a failed `bind` too: the condition (a folder, a second instance) may go away.
    tauri::async_runtime::spawn(supervise(app.clone(), state, first.ok(), uid));
}

/// Removes the socket file when the app exits.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(handle) = app.try_state::<McpHandle>()
        && handle.0.error().is_none()
    {
        let _ = std::fs::remove_file(handle.0.path());
    }
}

/// Supervision: while there is no listener, it tries to bind once per interval; if the listener panicked, an error goes into the state and it is
/// restarted. It never returns by itself.
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
                    state.bound();
                    log::info!("mcp: socket is up");
                }
                Err(error) => state.set_error(Some(error)),
            }
            continue;
        };
        let task = tauri::async_runtime::spawn(run_listener(app.clone(), state.clone(), live, uid));
        // `run_listener` does not return by itself: the end is only a panic (or the runtime stopping).
        if task.await.is_err() {
            log::error!("mcp: listener panicked, restarting");
            state.set_error(Some(SocketError::BindFailed));
            // The previous listener was destroyed with the task; the socket file remained, `bind` will replace it.
            listener = bind(state.path(), uid).ok();
            if listener.is_some() {
                state.set_error(None);
                state.bound();
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
                    // A hook for the safety net tests; it is absent in a release build (`debug_assertions` off).
                    assert!(
                        !(cfg!(debug_assertions) && state.panic_next.swap(false, Ordering::SeqCst)),
                        "check: listener panic"
                    );
                    tauri::async_runtime::spawn(serve_connection(app.clone(), stream, uid));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
            () = tokio::time::sleep(state.recheck) => {
                if socket_present(state.path()) {
                    continue;
                }
                // The socket file is gone (cleaned by the system or deleted): the same checks and the same path.
                match bind(state.path(), uid) {
                    Ok(fresh) => {
                        listener = fresh;
                        state.set_error(None);
                        state.bound();
                        log::info!("mcp: socket rebound");
                    }
                    Err(error) => {
                        log::warn!("mcp: socket not rebound: {}", error.code());
                        state.set_error(Some(error));
                    }
                }
            }
        }
    }
}

/// Reads the start of a connection and decides whether it is the reconnect marker. Reads no more than the marker length and only while
/// what was read is its beginning (the marker may arrive in parts); everything extra is returned to the server as the start of the stream.
async fn read_marker<R: AsyncReadExt + Unpin>(read: &mut R) -> std::io::Result<(bool, Vec<u8>)> {
    let mut head: Vec<u8> = Vec::with_capacity(RECONNECT_MARKER.len());
    let mut chunk = [0u8; 256];
    while head.len() < RECONNECT_MARKER.len() && RECONNECT_MARKER.starts_with(&head) {
        let n = read.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        head.extend_from_slice(chunk.get(..n).unwrap_or_default());
    }
    if head.starts_with(RECONNECT_MARKER) {
        Ok((true, head.split_off(RECONNECT_MARKER.len())))
    } else {
        Ok((false, head))
    }
}

/// A connection: a foreign uid is closed, the frame limit is put on the reading half, then the `rmcp` service. `own_uid` comes
/// from outside, so that a test can substitute it and check that a foreign user's connection is closed.
pub async fn serve_connection<R: Runtime>(app: AppHandle<R>, stream: UnixStream, own_uid: u32) {
    match stream.peer_cred() {
        Ok(cred) if cred.uid() == own_uid => {}
        _ => {
            log::warn!("mcp: a foreign user's connection was closed");
            return;
        }
    }
    let (mut read, write) = stream.into_split();
    // After a drop the bridge starts the connection with the marker: the client session is the same, the log shows "reconnect".
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
        Err(_) => log::debug!("mcp: the handshake did not happen"),
    }
    disconnected(&app, &info).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, ReadBuf};

    /// Hands out one piece per read, so the data arrives in parts by construction (a socket does not promise how it splits).
    struct Pieces<'a> {
        pieces: std::slice::Iter<'a, &'a [u8]>,
    }

    impl AsyncRead for Pieces<'_> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            if let Some(piece) = self.pieces.next() {
                buf.put_slice(piece);
            }
            Poll::Ready(Ok(()))
        }
    }

    async fn marker_of(pieces: &[&[u8]]) -> (bool, Vec<u8>) {
        let mut reader = Pieces {
            pieces: pieces.iter(),
        };
        read_marker(&mut reader).await.unwrap()
    }

    #[tokio::test]
    async fn the_reconnect_marker_arriving_in_pieces_is_recognised_and_the_rest_is_kept() {
        let (head, middle, tail) = (
            RECONNECT_MARKER.get(..11).unwrap(),
            RECONNECT_MARKER.get(11..30).unwrap(),
            RECONNECT_MARKER.get(30..).unwrap(),
        );
        let (reconnect, rest) = marker_of(&[head, middle, tail, b"{\"id\":1}\n"]).await;
        assert!(reconnect);
        assert!(rest.is_empty(), "{rest:?}");
        // What follows the marker in the same read goes back to the server as the start of the stream.
        let with_rest = [RECONNECT_MARKER, b"{\"id\":1}\n"].concat();
        let (reconnect, rest) =
            marker_of(&[with_rest.get(..20).unwrap(), with_rest.get(20..).unwrap()]).await;
        assert!(reconnect);
        assert_eq!(rest, b"{\"id\":1}\n");
    }

    #[tokio::test]
    async fn a_first_message_that_only_starts_like_the_marker_is_not_a_marker_and_loses_nothing() {
        let message = br#"{"jsonrpc":"2.0","method":"kinige/ping"}"#;
        let (reconnect, rest) =
            marker_of(&[message.get(..11).unwrap(), message.get(11..).unwrap()]).await;
        assert!(!reconnect);
        assert_eq!(rest, message);
    }

    #[tokio::test]
    async fn an_empty_or_garbage_start_is_not_a_marker() {
        assert_eq!(marker_of(&[]).await, (false, Vec::new()));
        let (reconnect, rest) = marker_of(&[b"\0\xff not json"]).await;
        assert!(!reconnect);
        assert_eq!(rest, b"\0\xff not json");
    }
}
