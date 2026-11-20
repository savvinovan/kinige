//! A fake Bot API server for the slice 5c tests: plain HTTP on 127.0.0.1, one thread per connection, a script of replies, a log of the
//! requests with their headers. Nothing here talks to Telegram, and the test tokens do not have the real token shape.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the client sent.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    /// The request target as written: the path and the query, still percent-encoded.
    pub target: String,
    /// Header names in lower case and their values, in the order they came.
    pub headers: Vec<(String, String)>,
}

/// What the server does about a request.
pub enum Reply {
    /// A status and a JSON body.
    Json(u16, String),
    /// A status, extra headers and a body.
    Raw(u16, Vec<(String, String)>, Vec<u8>),
    /// A status and a `Location`, no body.
    Redirect(u16, String),
    /// Declares `declared` bytes and sends `body` (a lie about the length when they differ), then closes.
    Declared(u64, Vec<u8>),
    /// Accepts the connection and never answers.
    Stall,
    /// Closes the connection without a byte.
    Hangup,
}

pub type Script = Arc<dyn Fn(&Recorded) -> Reply + Send + Sync>;

pub struct FakeBot {
    pub addr: SocketAddr,
    pub requests: Arc<Mutex<Vec<Recorded>>>,
}

impl FakeBot {
    pub fn start(script: Script) -> FakeBot {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests: Arc<Mutex<Vec<Recorded>>> = Arc::default();
        let log = requests.clone();
        let spawn = |work: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new()
                .name("fake-bot".into())
                .spawn(work)
                .expect("a thread of the fake server");
        };
        spawn(Box::new(move || {
            for stream in listener.incoming().flatten() {
                let (script, log) = (script.clone(), log.clone());
                spawn(Box::new(move || serve(stream, &script, &log)));
            }
        }));
        FakeBot { addr, requests }
    }

    /// `http://127.0.0.1:<port>`.
    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn seen(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

fn serve(mut stream: TcpStream, script: &Script, log: &Mutex<Vec<Recorded>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
        if head.len() > 16 * 1024 {
            return;
        }
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let mut lines = text.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (method, target) = (
        first.next().unwrap_or_default().to_string(),
        first.next().unwrap_or_default().to_string(),
    );
    let headers = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_string()))
        .collect();
    let recorded = Recorded {
        method,
        target,
        headers,
    };
    log.lock().unwrap().push(recorded.clone());
    match script(&recorded) {
        Reply::Json(status, body) => write_reply(
            &mut stream,
            status,
            &[("content-type".into(), "application/json".into())],
            body.len() as u64,
            body.as_bytes(),
        ),
        Reply::Raw(status, headers, body) => {
            write_reply(&mut stream, status, &headers, body.len() as u64, &body)
        }
        Reply::Redirect(status, location) => write_reply(
            &mut stream,
            status,
            &[("location".into(), location)],
            0,
            b"",
        ),
        Reply::Declared(declared, body) => write_reply(&mut stream, 200, &[], declared, &body),
        Reply::Stall => std::thread::sleep(Duration::from_secs(30)),
        Reply::Hangup => {}
    }
}

fn write_reply(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(String, String)],
    length: u64,
    body: &[u8],
) {
    let mut out =
        format!("HTTP/1.1 {status} X\r\ncontent-length: {length}\r\nconnection: close\r\n");
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// A `getStickerSet` answer of `count` static stickers named `S0`, `S1`, ... (`extra` objects are added as they are).
pub fn set_json(title: &str, count: usize, extra: &[serde_json::Value]) -> String {
    let mut stickers: Vec<serde_json::Value> = (0..count)
        .map(|i| {
            serde_json::json!({
                "file_id": format!("S{i}"), "file_unique_id": format!("u{i}"), "width": 512, "height": 400,
                "is_animated": false, "is_video": false, "emoji": "😺", "file_size": 1000
            })
        })
        .collect();
    stickers.extend(extra.iter().cloned());
    serde_json::json!({"ok": true, "result": {"name": "TestPack", "title": title, "stickers": stickers}}).to_string()
}

/// A `getFile` answer for `file_id`: its path is `stickers/<file_id>.webp`.
pub fn file_json(file_id: &str) -> String {
    serde_json::json!({"ok": true, "result": {"file_id": file_id, "file_path": format!("stickers/{file_id}.webp")}}).to_string()
}
