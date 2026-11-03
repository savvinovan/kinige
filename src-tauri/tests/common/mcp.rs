//! Helpers for the MCP server tests: an app with a running listener and a JSON-RPC client over a Unix socket.

#![allow(dead_code)]

use super::{Fixture, fixture};
use kinige_domain::AgentMode;
use kinige_lib::mcp::{self, McpHandle};
use kinige_store::Store;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

pub struct Mcp {
    pub fixture: Fixture,
    pub socket: PathBuf,
}

/// An app with access turned on, the mode and visible chats set, and the listener up in the fixture's data directory.
pub fn mcp(mode: AgentMode, chats: &[&str]) -> Mcp {
    mcp_with_recheck(mode, chats, Duration::from_secs(600))
}

/// The same with a short listener check interval (rebinding, restart after a panic).
pub fn mcp_with_recheck(mode: AgentMode, chats: &[&str], recheck: Duration) -> Mcp {
    let fixture = fixture();
    {
        let store = fixture.app.state::<Store>();
        store.set_agent_enabled(true).unwrap();
        store.set_agent_mode(mode).unwrap();
        store
            .set_agent_chats(chats.iter().map(|id| id.to_string()).collect())
            .unwrap();
    }
    mcp::start_with_recheck(fixture.app.handle(), fixture.data.path(), recheck);
    let socket = fixture.app.state::<McpHandle>().0.path().to_path_buf();
    Mcp { fixture, socket }
}

impl Mcp {
    pub fn store(&self) -> tauri::State<'_, Store> {
        self.fixture.app.state::<Store>()
    }

    pub fn client(&self) -> Client {
        Client::connect(&self.socket)
    }
}

fn is_closed_by_peer(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionAborted
    )
}

pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next: u64,
}

impl Client {
    pub fn connect(socket: &std::path::Path) -> Client {
        Client::try_connect(socket).expect("the socket accepts connections")
    }

    pub fn try_connect(socket: &std::path::Path) -> Option<Client> {
        let stream = UnixStream::connect(socket).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        // Writing a big frame into a socket that the server stopped reading must not hang the test.
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Some(Client {
            reader: BufReader::new(stream.try_clone().unwrap()),
            writer: stream,
            next: 1,
        })
    }

    pub fn send_raw(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }

    /// A part of a line without a newline: this is how reading the marker in parts is checked.
    pub fn send_partial(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    pub fn send(&mut self, value: Value) {
        self.send_raw(value.to_string().as_bytes());
    }

    /// The next server message; `None` means the connection is closed.
    pub fn recv(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&line).expect("the server sends JSON")),
        }
    }

    /// A request and its reply (server notifications are skipped).
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let message = self.recv().expect("the server replied");
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    pub fn handshake(&mut self, name: &str) -> Value {
        let result = self.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": name, "version": "1"}}),
        );
        self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        result
    }

    pub fn tool_names(&mut self) -> Vec<String> {
        let result = self.request("tools/list", json!({}));
        result["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect()
    }

    /// A tool call: `(isError, reply text)`.
    pub fn call(&mut self, tool: &str, arguments: Value) -> (bool, String) {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        let result = &reply["result"];
        (
            result["isError"].as_bool().unwrap_or(false),
            result["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )
    }

    /// The server closed the connection. If it closed it without reading everything sent (a frame over the limit), the kernel sends RST, and the read
    /// returns `ConnectionReset` instead of end of stream: that also counts as "closed by the server". A read timeout does not.
    pub fn closed_by_server(&mut self) -> bool {
        let mut rest = Vec::new();
        match self.reader.read_to_end(&mut rest) {
            Ok(_) => self.recv().is_none(),
            Err(error) => is_closed_by_peer(&error),
        }
    }

    /// A line without `unwrap`: the server may have closed the connection in the middle of a big write (`BrokenPipe`, `ConnectionReset`).
    /// Returns whether the line went through whole.
    pub fn send_raw_lossy(&mut self, bytes: &[u8]) -> bool {
        match self
            .writer
            .write_all(bytes)
            .and_then(|()| self.writer.write_all(b"\n"))
        {
            Ok(()) => true,
            Err(error) if is_closed_by_peer(&error) => false,
            Err(error) => panic!("write to the socket: {error}"),
        }
    }
}
