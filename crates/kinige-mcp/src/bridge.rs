//! The stdio <-> Kinige Unix socket bridge (ADR-0009). It does not open the database. It does not resend requests: on a drop an unanswered
//! request gets a JSON-RPC error (an in-flight `add_entry` may already have been written). After reconnecting it repeats
//! `initialize` and `notifications/initialized`; the reply to the repeated `initialize` does not go to the client.

use crate::frames::{FRAME_LIMIT, Frame, read_frame};
use crate::host::Host;
use crate::log::note;
use crate::scan::{Scan, is_object, scan, scan_prefix};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// The first line of a connection after a drop: the server does not write a second "connection" to the log for the same client session.
pub const RECONNECT_MARKER: &str = r#"{"jsonrpc":"2.0","method":"kinige/reconnect"}"#;
const CONNECT_RETRY: Duration = Duration::from_millis(100);

const LOST: &str = "Соединение с Kinige потеряно, проверьте журнал обращений (connection to Kinige lost; check the access log)";
const NOT_RUNNING: &str = "Откройте Kinige (Open Kinige: the app is not running)";
const UNAVAILABLE: &str =
    "journal_unavailable: Kinige is starting or recovering, try again in a moment";
/// JSON-RPC codes: parse failed; server error (a drop, no app).
const PARSE_ERROR: i32 = -32700;
const SERVER_ERROR: i32 = -32000;
const TOO_LONG: &str = "invalid_argument: the message is larger than 4 MiB";

pub struct Config {
    pub socket: PathBuf,
    /// How long to wait for the socket (10 s); tests shorten it.
    pub wait: Duration,
    pub frame_limit: usize,
}

impl Config {
    pub fn new(socket: PathBuf, wait: Duration) -> Config {
        Config {
            socket,
            wait,
            frame_limit: FRAME_LIMIT,
        }
    }
}

/// The current connection: the generation grows with each new one, so that a drop of the old one does not touch the new one.
struct Connection {
    generation: u64,
    writer: UnixStream,
}

#[derive(Default)]
struct State {
    connection: Option<Connection>,
    generation: u64,
    /// Unanswered client requests (messages with `method` and `id`): `id` verbatim -> the generation in which they were sent.
    pending: HashMap<String, u64>,
    initialize: Option<Vec<u8>>,
    initialized: Option<Vec<u8>>,
    /// The `id` of the repeated `initialize`, whose reply goes not to the client but to the bridge.
    rehandshake_id: Option<String>,
    rehandshake_done: bool,
}

pub struct Bridge {
    config: Config,
    host: Host,
    state: Mutex<State>,
    wake: Condvar,
    out: Mutex<Box<dyn Write + Send>>,
}

/// Why there is no connection.
enum Unreachable {
    NotRunning,
    Unavailable,
}

impl Bridge {
    pub fn new(config: Config, host: Host, out: Box<dyn Write + Send>) -> Arc<Bridge> {
        Arc::new(Bridge {
            config,
            host,
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            out: Mutex::new(out),
        })
    }

    /// Reads client messages until stdin ends. Returns on EOF.
    pub fn run(self: &Arc<Self>, mut input: impl BufRead) {
        let mut line = Vec::new();
        loop {
            match read_frame(&mut input, &mut line, self.config.frame_limit) {
                Ok(Frame::Line) => {
                    if !line.iter().all(u8::is_ascii_whitespace) {
                        self.client_line(&line);
                    }
                }
                Ok(Frame::TooLong) => {
                    // The `id` is looked up in the saved beginning of the line; none -- `null`, as JSON-RPC requires for an unparsed request.
                    let found = scan_prefix(&line);
                    let id = found.id.as_deref();
                    note("client_frame_too_long", id);
                    self.send_to_client(error_line(id.unwrap_or("null"), TOO_LONG));
                }
                Ok(Frame::Eof) | Err(_) => return,
            }
        }
    }

    fn client_line(self: &Arc<Self>, line: &[u8]) {
        // Not a JSON object (`{not json`): reply -32700 with `id: null` and nothing to the server; garbage does not start the app.
        if !is_object(line) {
            note("parse_error", None);
            self.send_to_client(error_with_code("null", PARSE_ERROR, "Parse error"));
            return;
        }
        let scanned = scan(line);
        let connected = self.connect();
        let id = scanned.id.as_deref();
        let Err(why) = connected else {
            self.remember_and_forward(line, &scanned);
            return;
        };
        // No connection: a request gets an error with the same `id`, a notification is dropped.
        note("unreachable", id);
        if scanned.is_request() {
            let text = match why {
                Unreachable::NotRunning => NOT_RUNNING,
                Unreachable::Unavailable => UNAVAILABLE,
            };
            self.send_to_client(error_line(id.unwrap_or("null"), text));
        }
    }

    /// Remembers the client handshake and the unanswered request, then writes the line to the server.
    fn remember_and_forward(self: &Arc<Self>, line: &[u8], scanned: &Scan) {
        let generation = {
            let mut state = self.lock();
            match scanned.method.as_deref() {
                Some("initialize") if scanned.id.is_some() => {
                    state.initialize = Some(line.to_vec())
                }
                Some("notifications/initialized") => state.initialized = Some(line.to_vec()),
                _ => {}
            }
            let Some(generation) = state.connection.as_ref().map(|c| c.generation) else {
                // The connection dropped between the check and the send: the request did not go out, but there is nothing for it to wait for.
                drop(state);
                if scanned.is_request() {
                    let id = scanned.id.as_deref().unwrap_or("null");
                    self.send_to_client(error_line(id, LOST));
                }
                return;
            };
            if let (true, Some(id)) = (scanned.is_request(), &scanned.id) {
                state.pending.insert(id.clone(), generation);
            }
            generation
        };
        if self.write_server(generation, line).is_err() {
            self.lost(generation);
        }
    }

    /// Establishes a connection if there is none. After a drop it repeats the client handshake.
    fn connect(self: &Arc<Self>) -> Result<(), Unreachable> {
        if self.lock().connection.is_some() {
            return Ok(());
        }
        let deadline = Instant::now() + self.config.wait;
        // First just connect: if the socket answers, the app is running, and there is no need to check the process (let alone to run
        // `open`, which would arrive at a running app as Reopen and show a window).
        let mut running: Option<bool> = None;
        let stream = loop {
            match UnixStream::connect(&self.config.socket) {
                Ok(stream) => break stream,
                Err(_) => {
                    // No answer: the process exists -- wait for the socket (the app is starting); no process -- start it once.
                    let running = *running.get_or_insert_with(|| {
                        let alive = self.host.process_running();
                        if !alive {
                            self.host.launch();
                        }
                        alive
                    });
                    if Instant::now() >= deadline {
                        return Err(if running {
                            Unreachable::Unavailable
                        } else {
                            Unreachable::NotRunning
                        });
                    }
                    thread::sleep(CONNECT_RETRY);
                }
            }
        };
        let (Ok(writer), Ok(reader)) = (stream.try_clone(), stream.try_clone()) else {
            return Err(Unreachable::Unavailable);
        };
        let (generation, replay) = {
            let mut state = self.lock();
            state.generation += 1;
            let generation = state.generation;
            let replay = state
                .initialize
                .clone()
                .zip(scan_id(state.initialize.as_deref()));
            state.rehandshake_id = replay.as_ref().map(|(_, id)| id.clone());
            state.rehandshake_done = false;
            state.connection = Some(Connection { generation, writer });
            (generation, replay)
        };
        let me = Arc::clone(self);
        let reading = thread::Builder::new()
            .name("kinige-mcp-reader".into())
            .spawn(move || me.read_server(generation, reader));
        if reading.is_err() {
            self.lost(generation);
            return Err(Unreachable::Unavailable);
        }
        let Some((initialize, _)) = replay else {
            return Ok(());
        };
        if self.replay_handshake(generation, &initialize).is_err() {
            self.lost(generation);
            return Err(Unreachable::Unavailable);
        }
        Ok(())
    }

    /// The reconnect marker, `initialize`, waiting for its reply, then `notifications/initialized`.
    fn replay_handshake(&self, generation: u64, initialize: &[u8]) -> io::Result<()> {
        self.write_server(generation, RECONNECT_MARKER.as_bytes())?;
        self.write_server(generation, initialize)?;
        let deadline = Instant::now() + self.config.wait;
        let mut state = self.lock();
        while !state.rehandshake_done {
            let left = deadline.saturating_duration_since(Instant::now());
            let alive = state
                .connection
                .as_ref()
                .is_some_and(|c| c.generation == generation);
            if left.is_zero() || !alive {
                return Err(io::ErrorKind::TimedOut.into());
            }
            state = match self.wake.wait_timeout(state, left) {
                Ok((state, _)) => state,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        let initialized = state.initialized.clone();
        drop(state);
        match initialized {
            Some(line) => self.write_server(generation, &line),
            None => Ok(()),
        }
    }

    /// Reads server replies until a drop, then settles the unanswered requests.
    fn read_server(self: Arc<Self>, generation: u64, stream: UnixStream) {
        let mut reader = BufReader::new(stream);
        let mut line = Vec::new();
        loop {
            match read_frame(&mut reader, &mut line, self.config.frame_limit) {
                Ok(Frame::Line) => self.server_line(generation, &line),
                Ok(Frame::TooLong) => {
                    note("server_frame_too_long", None);
                    break;
                }
                Ok(Frame::Eof) | Err(_) => break,
            }
        }
        self.lost(generation);
    }

    fn server_line(&self, generation: u64, line: &[u8]) {
        if line.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let scanned = scan(line);
        if scanned.is_response() {
            let mut state = self.lock();
            let id = scanned.id.as_deref();
            if id.is_some() && id == state.rehandshake_id.as_deref() && !state.rehandshake_done {
                let current = state
                    .connection
                    .as_ref()
                    .is_some_and(|c| c.generation == generation);
                if current {
                    // The reply to the repeated `initialize`: the client is not waiting for it.
                    state.rehandshake_done = true;
                    drop(state);
                    self.wake.notify_all();
                    return;
                }
            }
            if let Some(id) = id {
                state.pending.remove(id);
            }
        }
        self.send_to_client(line);
    }

    /// The connection `generation` dropped: each of its unanswered requests gets an error, there is no resend.
    fn lost(&self, generation: u64) {
        let failed: Vec<String> = {
            let mut state = self.lock();
            if state
                .connection
                .as_ref()
                .is_none_or(|c| c.generation != generation)
            {
                return;
            }
            state.connection = None;
            let ids: Vec<String> = state
                .pending
                .iter()
                .filter(|(_, g)| **g == generation)
                .map(|(id, _)| id.clone())
                .collect();
            for id in &ids {
                state.pending.remove(id);
            }
            ids
        };
        self.wake.notify_all();
        note("connection_lost", None);
        for id in failed {
            self.send_to_client(error_line(&id, LOST));
        }
    }

    fn write_server(&self, generation: u64, line: &[u8]) -> io::Result<()> {
        // A duplicate of the descriptor is taken under the lock, the write goes without it: a slow server must not hold up the reply reader.
        let mut writer = {
            let state = self.lock();
            let Some(connection) = state
                .connection
                .as_ref()
                .filter(|c| c.generation == generation)
            else {
                return Err(io::ErrorKind::NotConnected.into());
            };
            connection.writer.try_clone()?
        };
        // One write per line.
        let mut framed = Vec::with_capacity(line.len() + 1);
        framed.extend_from_slice(line);
        framed.push(b'\n');
        writer.write_all(&framed)?;
        writer.flush()
    }

    fn send_to_client(&self, line: impl AsRef<[u8]>) {
        let mut out = match self.out.lock() {
            Ok(out) => out,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut framed = line.as_ref().to_vec();
        framed.push(b'\n');
        // The client closed stdout -- the bridge is no longer needed here.
        if out.write_all(&framed).and_then(|()| out.flush()).is_err() {
            std::process::exit(0);
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn scan_id(line: Option<&[u8]>) -> Option<String> {
    scan(line?).id
}

/// A JSON-RPC error reply with the given `id` (verbatim). The texts are bridge constants, there is nothing to escape in them except quotes.
pub fn error_line(id: &str, message: &str) -> String {
    error_with_code(id, SERVER_ERROR, message)
}

fn error_with_code(id: &str, code: i32, message: &str) -> String {
    let message = message.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":{code},"message":"{message}"}}}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_line_is_valid_json_rpc_with_the_same_id() {
        assert_eq!(
            error_line("7", "lost"),
            r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"lost"}}"#
        );
        assert!(error_line("\"abc\"", "a\"b").contains(r#""id":"abc""#));
        assert!(error_line("null", "x").contains(r#""id":null"#));
    }
}
