//! Мост stdio <-> Unix-сокет Kinige (ADR-0009). Базу не открывает. Запросы не переотправляет: при обрыве каждый
//! неотвеченный запрос получает ошибку JSON-RPC, потому что `add_entry` в полёте мог уже записаться. После переподключения
//! повторяет `initialize` и `notifications/initialized` клиента; ответ на повторный `initialize` клиенту не уходит.

use crate::frames::{FRAME_LIMIT, Frame, read_frame};
use crate::host::Host;
use crate::log::note;
use crate::scan::{Scan, scan};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// Первая строка соединения после обрыва: сервер не пишет в журнал второе «подключение» за тот же сеанс клиента.
pub const RECONNECT_MARKER: &str = r#"{"jsonrpc":"2.0","method":"kinige/reconnect"}"#;

const CONNECT_RETRY: Duration = Duration::from_millis(100);

const LOST: &str = "Соединение с Kinige потеряно, проверьте журнал обращений (connection to Kinige lost; check the access log)";
const NOT_RUNNING: &str = "Откройте Kinige (Open Kinige: the app is not running)";
const UNAVAILABLE: &str =
    "journal_unavailable: Kinige is starting or recovering, try again in a moment";
const TOO_LONG: &str = "invalid_argument: the message is larger than 4 MiB";

pub struct Config {
    pub socket: PathBuf,
    /// Сколько ждать сокет (10 с); тесты укорачивают.
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

/// Текущее соединение: поколение растёт с каждым новым, чтобы обрыв старого не трогал новое.
struct Connection {
    generation: u64,
    writer: UnixStream,
}

#[derive(Default)]
struct State {
    connection: Option<Connection>,
    generation: u64,
    /// Неотвеченные запросы клиента (сообщения с `method` и `id`): `id` дословно → поколение, в котором отправлены.
    pending: HashMap<String, u64>,
    initialize: Option<Vec<u8>>,
    initialized: Option<Vec<u8>>,
    /// `id` повторного `initialize`, ответ на который уходит не клиенту, а мосту.
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

/// Почему соединения нет.
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

    /// Читает сообщения клиента, пока stdin не закончится. Возвращается при EOF.
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
                    note("client_frame_too_long", None);
                    self.send_to_client(error_line("null", TOO_LONG));
                }
                Ok(Frame::Eof) | Err(_) => return,
            }
        }
    }

    fn client_line(self: &Arc<Self>, line: &[u8]) {
        let scanned = scan(line);
        let connected = self.connect();
        let id = scanned.id.as_deref();
        let Err(why) = connected else {
            self.remember_and_forward(line, &scanned);
            return;
        };
        // Соединения нет: запрос получает ошибку с тем же `id`, уведомление пропадает.
        note("unreachable", id);
        if scanned.is_request() {
            let text = match why {
                Unreachable::NotRunning => NOT_RUNNING,
                Unreachable::Unavailable => UNAVAILABLE,
            };
            self.send_to_client(error_line(id.unwrap_or("null"), text));
        }
    }

    /// Запоминает рукопожатие клиента и неотвеченный запрос, затем пишет строку серверу.
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
                // Соединение оборвалось между проверкой и отправкой: запрос не ушёл, но и ждать ему нечего.
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

    /// Устанавливает соединение, если его нет. После обрыва повторяет рукопожатие клиента.
    fn connect(self: &Arc<Self>) -> Result<(), Unreachable> {
        if self.lock().connection.is_some() {
            return Ok(());
        }
        let running = self.host.process_running();
        if !running {
            self.host.launch();
        }
        let deadline = Instant::now() + self.config.wait;
        let stream = loop {
            match UnixStream::connect(&self.config.socket) {
                Ok(stream) => break stream,
                Err(_) if Instant::now() < deadline => thread::sleep(CONNECT_RETRY),
                Err(_) => {
                    return Err(if running {
                        Unreachable::Unavailable
                    } else {
                        Unreachable::NotRunning
                    });
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
        thread::spawn(move || me.read_server(generation, reader));
        let Some((initialize, _)) = replay else {
            return Ok(());
        };
        if self.replay_handshake(generation, &initialize).is_err() {
            self.lost(generation);
            return Err(Unreachable::Unavailable);
        }
        Ok(())
    }

    /// Маркер переподключения, `initialize`, ожидание ответа на него, затем `notifications/initialized`.
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

    /// Читает ответы сервера до обрыва, затем разбирает неотвеченные запросы.
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
                    // Ответ на повторный `initialize`: клиент его не ждёт.
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

    /// Соединение `generation` оборвалось: каждый его неотвеченный запрос получает ошибку, повторной отправки нет.
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
        // Дубликат дескриптора берётся под замком, запись идёт без него: медленный сервер не должен держать читателя ответов.
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
        // Одна запись на строку.
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
        // Клиент закрыл stdout — мост здесь больше не нужен.
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

/// Ответ-ошибка JSON-RPC с заданным `id` (дословно). Тексты — константы моста, экранировать в них нечего, кроме кавычек.
pub fn error_line(id: &str, message: &str) -> String {
    let message = message.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32000,"message":"{message}"}}}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_line_is_valid_json_rpc_with_the_same_id() {
        assert_eq!(
            error_line("7", "потеряно"),
            r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"потеряно"}}"#
        );
        assert!(error_line("\"abc\"", "a\"b").contains(r#""id":"abc""#));
        assert!(error_line("null", "x").contains(r#""id":null"#));
    }
}
