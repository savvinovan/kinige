//! Мост `kinige-mcp` как собранный бинарник с подставным сервером на временном сокете (ADR-0009, слайс 4a): пересылка,
//! обрыв без повторной отправки, повтор рукопожатия, запуск приложения только когда процесса нет, предел кадра, stderr.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const WAIT_MS: &str = "300";
const SHORT: Duration = Duration::from_millis(700);
const LONG: Duration = Duration::from_secs(6);
const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"Test","version":"1"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const MARKER: &str = r#"{"jsonrpc":"2.0","method":"kinige/reconnect"}"#;

/// Подставной сервер: слушает сокет, принимает соединения.
struct Server {
    listener: UnixListener,
}

struct Conn {
    lines: Receiver<String>,
    writer: UnixStream,
}

impl Server {
    fn bind(path: &Path) -> Server {
        let listener = UnixListener::bind(path).unwrap();
        listener.set_nonblocking(true).unwrap();
        Server { listener }
    }

    fn accept(&self, within: Duration) -> Option<Conn> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Ok((stream, _)) = self.listener.accept() {
                stream.set_nonblocking(false).unwrap();
                let reader = BufReader::new(stream.try_clone().unwrap());
                let (send, lines) = mpsc::channel();
                thread::spawn(move || {
                    for line in reader.lines() {
                        let Ok(line) = line else { return };
                        if send.send(line).is_err() {
                            return;
                        }
                    }
                });
                return Some(Conn {
                    lines,
                    writer: stream,
                });
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }
}

impl Conn {
    fn next(&self, within: Duration) -> Option<String> {
        self.lines.recv_timeout(within).ok()
    }

    fn send(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }

    fn close(self) {
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
    }
}

/// Запущенный мост: stdin/stdout клиента и stderr.
struct Bridge {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    out: Receiver<String>,
    err: std::process::ChildStderr,
}

impl Bridge {
    fn start(socket: &Path, process: &str, open_log: &Path) -> Bridge {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kinige-mcp-bridge"))
            .arg("--socket")
            .arg(socket)
            .env("KINIGE_MCP_TEST_WAIT_MS", WAIT_MS)
            .env("KINIGE_MCP_TEST_PROCESS", process)
            .env("KINIGE_MCP_TEST_OPEN_LOG", open_log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, out) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if send.send(line).is_err() {
                    return;
                }
            }
        });
        Bridge {
            stdin: child.stdin.take(),
            err: child.stderr.take().unwrap(),
            out,
            child,
        }
    }

    fn say(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    fn hear(&self, within: Duration) -> Option<String> {
        self.out.recv_timeout(within).ok()
    }

    /// Закрывает stdin, ждёт завершения и возвращает весь stderr.
    fn finish(mut self) -> String {
        drop(self.stdin.take());
        let _ = self.child.wait();
        let mut text = String::new();
        let _ = self.err.read_to_string(&mut text);
        text
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        Env {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("k.sock")
    }

    fn open_log(&self) -> PathBuf {
        self.dir.path().join("open.log")
    }

    fn opens(&self) -> usize {
        fs::read_to_string(self.open_log()).map_or(0, |text| text.lines().count())
    }

    fn bridge(&self, process: &str) -> Bridge {
        Bridge::start(&self.socket(), process, &self.open_log())
    }
}

fn call(id: u32, marker: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"add_entry","arguments":{{"text":"{marker}"}}}}}}"#
    )
}

fn result(id: u32) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"content":[]}}}}"#)
}

#[test]
fn forwards_lines_both_ways_and_stdout_has_only_json_rpc() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(INIT);
    let mut conn = server.accept(LONG).expect("мост подключился");
    assert_eq!(conn.next(LONG).as_deref(), Some(INIT));
    conn.send(&result(1));
    conn.send(r#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#);
    let heard: Vec<String> = (0..2).filter_map(|_| bridge.hear(LONG)).collect();
    assert_eq!(heard[0], result(1));
    assert!(heard[1].contains("notifications/message"));
    assert!(
        heard
            .iter()
            .all(|line| line.starts_with('{') && line.ends_with('}'))
    );
    // Большой ответ приходит целиком.
    let big = format!(
        r#"{{"jsonrpc":"2.0","id":9,"result":{{"text":"{}"}}}}"#,
        "я".repeat(500_000)
    );
    conn.send(&big);
    assert_eq!(bridge.hear(LONG).as_deref(), Some(big.as_str()));
    assert_eq!(env.opens(), 0, "процесс есть: open не вызывается");
}

#[test]
fn no_socket_and_no_process_opens_the_app_once_then_answers_with_an_error_and_the_same_id() {
    let env = Env::new();
    let mut bridge = env.bridge("absent");
    bridge.say(&call(5, "m"));
    let line = bridge.hear(LONG).expect("ошибка");
    assert!(
        line.contains(r#""id":5"#) && line.contains(r#""error""#),
        "{line}"
    );
    assert!(line.contains("Откройте Kinige"), "{line}");
    assert_eq!(env.opens(), 1);
    // Уведомление без сокета пропадает молча, мост жив.
    bridge.say(INITIALIZED);
    assert!(bridge.hear(SHORT).is_none() || env.opens() >= 1);
}

/// Регрессия приёмки 4a: ошибка проверки процесса (путь с `..`, симлинк) запускала `open` поверх работающего приложения, а
/// это Reopen и окно на экране. Отвечающий сокет значит, что приложение работает, и `open` не нужен, что бы ни сказала проверка.
#[test]
fn a_listening_socket_never_triggers_open_even_if_the_process_check_says_absent() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("absent");
    bridge.say(&call(8, "m"));
    let mut conn = server.accept(LONG).expect("мост подключился");
    assert!(conn.next(LONG).unwrap().contains(r#""id":8"#));
    conn.send(&result(8));
    assert_eq!(bridge.hear(LONG).as_deref(), Some(result(8).as_str()));
    assert_eq!(env.opens(), 0);
}

#[test]
fn process_present_but_no_socket_gives_journal_unavailable_and_never_opens() {
    let env = Env::new();
    let mut bridge = env.bridge("present");
    bridge.say(&call(6, "m"));
    let line = bridge.hear(LONG).expect("ошибка");
    assert!(
        line.contains(r#""id":6"#) && line.contains("journal_unavailable"),
        "{line}"
    );
    assert_eq!(env.opens(), 0);
}

#[test]
fn the_socket_appearing_within_the_wait_lets_the_call_through_in_both_cases() {
    for process in ["present", "absent"] {
        let env = Env::new();
        let socket = env.socket();
        let server = thread::spawn(move || {
            thread::sleep(Duration::from_millis(120));
            let server = Server::bind(&socket);
            let mut conn = server.accept(LONG).expect("подключение");
            let line = conn.next(LONG).expect("строка");
            conn.send(&result(7));
            line
        });
        let mut bridge = env.bridge(process);
        bridge.say(&call(7, "m"));
        assert_eq!(
            bridge.hear(LONG).as_deref(),
            Some(result(7).as_str()),
            "{process}"
        );
        assert!(server.join().unwrap().contains(r#""id":7"#));
        assert_eq!(env.opens(), usize::from(process == "absent"));
    }
}

#[test]
fn a_cut_with_add_entry_in_flight_answers_with_an_error_and_never_resends() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(&call(3, "MARKER-ONE"));
    let mut conn = server.accept(LONG).unwrap();
    let got = conn.next(LONG).unwrap();
    assert!(got.contains("MARKER-ONE"));
    conn.send("");
    conn.close();
    let line = bridge.hear(LONG).expect("ошибка вместо ответа");
    assert!(
        line.contains(r#""id":3"#) && line.contains("потеряно"),
        "{line}"
    );
    // Повторной отправки нет: новое соединение не появляется, пока клиент сам не напишет.
    assert!(
        server.accept(SHORT).is_none(),
        "мост не должен переподключаться сам"
    );
    // Следующий запрос идёт в новое соединение, и старый туда не попадает.
    bridge.say(&call(4, "MARKER-TWO"));
    let mut second = server.accept(LONG).unwrap();
    assert!(second.next(LONG).unwrap().contains("MARKER-TWO"));
    assert!(second.next(SHORT).is_none(), "ни одного повтора");
    second.send(&result(4));
    assert_eq!(bridge.hear(LONG).as_deref(), Some(result(4).as_str()));
}

#[test]
fn after_a_reconnect_the_handshake_is_repeated_and_its_answer_never_reaches_the_client() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(INIT);
    let mut first = server.accept(LONG).unwrap();
    assert_eq!(first.next(LONG).as_deref(), Some(INIT));
    first.send(&result(1));
    assert_eq!(bridge.hear(LONG).as_deref(), Some(result(1).as_str()));
    bridge.say(INITIALIZED);
    assert_eq!(first.next(LONG).as_deref(), Some(INITIALIZED));
    bridge.say(&call(2, "in-flight"));
    assert!(first.next(LONG).unwrap().contains("in-flight"));
    first.close();
    let lost = bridge.hear(LONG).unwrap();
    assert!(
        lost.contains(r#""id":2"#) && lost.contains("потеряно"),
        "{lost}"
    );

    bridge.say(&call(3, "after"));
    let mut second = server.accept(LONG).unwrap();
    assert_eq!(second.next(LONG).as_deref(), Some(MARKER));
    assert_eq!(
        second.next(LONG).as_deref(),
        Some(INIT),
        "initialize повторён дословно"
    );
    second.send(&result(1));
    assert_eq!(second.next(LONG).as_deref(), Some(INITIALIZED));
    assert!(second.next(LONG).unwrap().contains("after"));
    second.send(&result(3));
    // Клиент видит только ответ на свой вызов: ответа на повторный initialize у него нет.
    assert_eq!(bridge.hear(LONG).as_deref(), Some(result(3).as_str()));
    assert!(bridge.hear(SHORT).is_none());
}

#[test]
fn a_cut_before_initialize_repeats_nothing() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(&call(1, "early"));
    let first = server.accept(LONG).unwrap();
    first.next(LONG).unwrap();
    first.close();
    assert!(bridge.hear(LONG).unwrap().contains("потеряно"));
    bridge.say(&call(2, "again"));
    let second = server.accept(LONG).unwrap();
    let line = second.next(LONG).unwrap();
    assert!(
        line.contains("again"),
        "первая строка нового соединения — сам запрос: {line}"
    );
}

/// Сервер, который принял соединение, но не отвечает на повторный `initialize`: вызов клиента не уходит, клиент получает
/// ошибку со своим `id`, а не молчание.
#[test]
fn a_server_that_never_answers_the_repeated_initialize_gives_the_call_an_error_and_receives_no_call()
 {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(INIT);
    let mut first = server.accept(LONG).unwrap();
    assert_eq!(first.next(LONG).as_deref(), Some(INIT));
    first.send(&result(1));
    bridge.hear(LONG).unwrap();
    bridge.say(INITIALIZED);
    assert_eq!(first.next(LONG).as_deref(), Some(INITIALIZED));
    first.close();
    bridge.say(&call(3, "never-delivered"));
    let mut second = server.accept(LONG).unwrap();
    assert_eq!(second.next(LONG).as_deref(), Some(MARKER));
    assert_eq!(second.next(LONG).as_deref(), Some(INIT));
    // Ответа на initialize нет.
    let line = bridge.hear(LONG).expect("ошибка вместо молчания");
    assert!(
        line.contains(r#""id":3"#) && line.contains("journal_unavailable"),
        "{line}"
    );
    assert!(
        second.next(SHORT).is_none(),
        "вызов на сервер, не прошедший рукопожатие, не уходит"
    );
    second.send("");
}

#[test]
fn text_that_is_not_json_is_answered_with_a_parse_error_and_id_null_and_nothing_is_forwarded() {
    let env = Env::new();
    let mut bridge = env.bridge("absent");
    bridge.say("{not json");
    let line = bridge.hear(LONG).expect("ошибка разбора");
    assert!(
        line.contains(r#""id":null"#) && line.contains("-32700"),
        "{line}"
    );
    assert_eq!(env.opens(), 0, "мусор не запускает приложение");
    // Мост жив и работает дальше.
    bridge.say(&call(2, "after-garbage"));
    let line = bridge.hear(LONG).expect("ошибка недоступности");
    assert!(line.contains(r#""id":2"#), "{line}");
}

#[test]
fn a_client_answer_to_a_server_request_is_not_an_unanswered_request() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(INIT);
    let mut conn = server.accept(LONG).unwrap();
    conn.next(LONG).unwrap();
    conn.send(&result(1));
    bridge.hear(LONG).unwrap();
    // Сервер просит у клиента корни; клиент отвечает; затем обрыв.
    conn.send(r#"{"jsonrpc":"2.0","id":"srv-1","method":"roots/list"}"#);
    assert!(bridge.hear(LONG).unwrap().contains("roots/list"));
    bridge.say(r#"{"jsonrpc":"2.0","id":"srv-1","result":{"roots":[]}}"#);
    assert!(conn.next(LONG).unwrap().contains("srv-1"));
    conn.close();
    assert!(
        bridge.hear(SHORT).is_none(),
        "ответ клиента не получает ошибки"
    );
}

#[test]
fn a_frame_over_4_mib_is_rejected_on_both_sides() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(INIT);
    let mut conn = server.accept(LONG).unwrap();
    conn.next(LONG).unwrap();
    conn.send(&result(1));
    bridge.hear(LONG).unwrap();
    // От клиента: ошибка с настоящим id (он стоит раньше места обрыва), на сервер ничего не уходит, мост жив.
    let huge = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"x","params":"{}"}}"#,
        "z".repeat(4 * 1024 * 1024 + 10)
    );
    bridge.say(&huge);
    let line = bridge.hear(LONG).expect("ошибка");
    assert!(
        line.contains(r#""id":2"#) && line.contains("4 MiB"),
        "{line}"
    );
    // id после длинного значения не достать из сохранённого начала: тогда null.
    let late_id = format!(
        r#"{{"jsonrpc":"2.0","method":"x","params":"{}","id":9}}"#,
        "z".repeat(4 * 1024 * 1024 + 10)
    );
    bridge.say(&late_id);
    let line = bridge.hear(LONG).expect("ошибка");
    assert!(
        line.contains(r#""id":null"#) && line.contains("4 MiB"),
        "{line}"
    );
    bridge.say(&call(3, "ok"));
    assert!(
        conn.next(LONG).unwrap().contains(r#""id":3"#),
        "на сервер ушёл только следующий запрос"
    );
    // От сервера: соединение закрывается, неотвеченный запрос получает ошибку.
    let huge_reply = format!(
        r#"{{"jsonrpc":"2.0","id":3,"result":"{}"}}"#,
        "z".repeat(4 * 1024 * 1024 + 10)
    );
    let _ = conn.writer.write_all(huge_reply.as_bytes());
    let _ = conn.writer.write_all(b"\n");
    let line = bridge.hear(LONG).expect("ошибка вместо огромного ответа");
    assert!(
        line.contains(r#""id":3"#) && line.contains("потеряно"),
        "{}",
        &line[..line.len().min(200)]
    );
}

#[test]
fn stderr_holds_only_categories_and_never_contents() {
    let env = Env::new();
    let server = Server::bind(&env.socket());
    let mut bridge = env.bridge("present");
    bridge.say(&call(1, "СЕКРЕТНЫЙ-МАРКЕР-9917"));
    let conn = server.accept(LONG).unwrap();
    assert!(conn.next(LONG).unwrap().contains("СЕКРЕТНЫЙ"));
    conn.close();
    bridge.hear(LONG).unwrap();
    bridge.say(r#"{"jsonrpc":"2.0","id":"СЕКРЕТНЫЙ-ID-9917\n\u001b[31m","method":"tools/call","params":{"arguments":{"text":"СЕКРЕТНЫЙ-МАРКЕР-9917"}}}"#);
    server.accept(LONG).unwrap().close();
    bridge.hear(LONG).unwrap();
    let stderr = bridge.finish();
    assert!(
        stderr.lines().all(|line| line.starts_with("kinige-mcp: ")),
        "{stderr}"
    );
    assert!(
        !stderr.contains("МАРКЕР") && !stderr.contains("СЕКРЕТНЫЙ-ID"),
        "{stderr}"
    );
}

#[test]
fn without_socket_argument_the_bridge_exits_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_kinige-mcp-bridge"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
