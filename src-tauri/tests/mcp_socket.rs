//! Сокет сервера MCP (ADR-0009, слайс 4a): права, симлинки, чужой владелец, второй экземпляр, длинный путь, перепривязка,
//! сбой `bind` и `chmod`, удаление при выходе.

mod common;

use common::mcp::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_lib::mcp::{self, McpHandle, SocketError};
use kinige_store::Store;
use serde_json::json;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::Manager;

fn mode(path: &Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777
}

fn error_of(fixture: &Fixture) -> Option<SocketError> {
    fixture.app.state::<McpHandle>().0.error()
}

/// Свой uid: `own_uid` строит пару сокетов tokio и поэтому требует среды выполнения.
fn own() -> u32 {
    tauri::async_runtime::block_on(async { mcp::own_uid().unwrap() })
}

fn plain_fixture() -> Fixture {
    let fixture = fixture();
    mcp::start(fixture.app.handle(), fixture.data.path());
    fixture
}

#[test]
fn the_socket_file_is_0600_and_its_folder_0700() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    assert_eq!(mode(&mcp.socket), 0o600);
    assert_eq!(mode(mcp.socket.parent().unwrap()), 0o700);
    assert!(error_of(&mcp.fixture).is_none());
}

#[test]
fn a_symlinked_or_loose_folder_makes_mcp_unavailable_but_the_journal_keeps_working() {
    // Папка сокета — симлинк на чужое место.
    let fixture = fixture();
    let elsewhere = tempfile::tempdir().unwrap();
    symlink(elsewhere.path(), fixture.data.path().join("mcp")).unwrap();
    mcp::start(fixture.app.handle(), fixture.data.path());
    assert_eq!(error_of(&fixture), Some(SocketError::DirUnsafe));
    assert!(
        std::fs::read_dir(elsewhere.path())
            .unwrap()
            .next()
            .is_none(),
        "в чужую папку ничего не положено"
    );
    // Журнал работает, ошибка сокета видна окну.
    let main = window(&fixture, "main");
    let access = call(&main, "get_agent_access", json!({})).unwrap();
    assert_eq!(access["socket"]["error"], "dir_unsafe");
    assert_eq!(access["socket"]["listening"], false);
    fixture
        .app
        .state::<Store>()
        .send_message(INBOX_ID, "запись", &kinige_store::Now::system())
        .unwrap();

    // Права шире 0700 — тоже небезопасно.
    let loose = fixture_without_start();
    let dir = loose.data.path().join("mcp");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    mcp::start(loose.app.handle(), loose.data.path());
    assert_eq!(error_of(&loose), Some(SocketError::DirUnsafe));
}

fn fixture_without_start() -> Fixture {
    fixture()
}

#[test]
fn a_responding_old_socket_means_a_second_kinige_and_its_server_is_not_started() {
    let first = plain_fixture();
    assert!(error_of(&first).is_none());
    // Второй Kinige на тех же данных: свой каталог данных у фикстуры другой, поэтому берём путь первого напрямую.
    let path = first.app.state::<McpHandle>().0.path().to_path_buf();
    let uid = own();
    let second = tauri::async_runtime::block_on(async { mcp::bind(&path, uid) });
    assert_eq!(second.err(), Some(SocketError::SecondInstance));
    // Первый продолжает отвечать.
    assert!(UnixStream::connect(&path).is_ok());
}

#[test]
fn a_stale_socket_file_is_replaced_and_a_foreign_file_is_left_alone() {
    let fixture = fixture();
    let dir = fixture.data.path().join("mcp");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("kinige.sock");
    // Старый сокет без слушателя (процесс умер): удаляется, слушатель поднимается.
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    mcp::start(fixture.app.handle(), fixture.data.path());
    assert!(error_of(&fixture).is_none());
    assert!(UnixStream::connect(&path).is_ok());

    // На месте сокета лежит чужой обычный файл: не трогаем, MCP недоступен.
    let other = common::fixture();
    let dir = other.data.path().join("mcp");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(dir.join("kinige.sock"), "мои заметки").unwrap();
    mcp::start(other.app.handle(), other.data.path());
    assert_eq!(error_of(&other), Some(SocketError::BindFailed));
    assert_eq!(
        std::fs::read_to_string(dir.join("kinige.sock")).unwrap(),
        "мои заметки"
    );
}

#[test]
fn a_failed_chmod_after_bind_removes_the_socket_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("k.sock");
    let uid = own();
    let failed = tauri::async_runtime::block_on(async {
        mcp::bind_with(&path, uid, |_| Err(std::io::Error::other("chmod отказал")))
    });
    assert_eq!(failed.err(), Some(SocketError::BindFailed));
    assert!(
        !path.exists(),
        "файл сокета с неверными правами не остаётся"
    );
}

#[test]
fn a_long_data_dir_falls_back_to_a_short_path_in_the_temp_dir() {
    let fixture = fixture();
    // Много вложенных папок, а не одно длинное имя: имя компонента пути ограничено 255 байтами (NAME_MAX на Linux).
    let long = (0..6).fold(fixture.data.path().to_path_buf(), |path, _| {
        path.join("очень-длинное-имя-папки")
    });
    std::fs::create_dir_all(&long).unwrap();
    let expected = mcp::socket_path(&long, &std::env::temp_dir());
    assert!(expected.starts_with(std::env::temp_dir().join("kinige-mcp")));
    mcp::start(fixture.app.handle(), &long);
    assert_eq!(fixture.app.state::<McpHandle>().0.path(), expected);
    assert!(error_of(&fixture).is_none());
    assert!(UnixStream::connect(&expected).is_ok());
    mcp::shutdown(fixture.app.handle());
    let _ = std::fs::remove_dir(expected.parent().unwrap());
}

#[test]
fn a_socket_file_removed_from_outside_is_rebound_on_the_next_check() {
    // Укороченный интервал проверки; значение одно на весь процесс тестов этого файла.
    unsafe { std::env::set_var("KINIGE_MCP_RECHECK_MS", "150") };
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    assert!(UnixStream::connect(&mcp.socket).is_ok());
    std::fs::remove_file(&mcp.socket).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !mcp.socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(
        mcp.socket.exists(),
        "слушатель перепривязался к тому же пути"
    );
    assert_eq!(mode(&mcp.socket), 0o600);
    let mut client = mcp.client();
    client.handshake("Claude");
    assert!(!client.call("list_chats", json!({})).0);
}

#[test]
fn the_socket_is_removed_on_exit() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    assert!(mcp.socket.exists());
    mcp::shutdown(mcp.fixture.app.handle());
    assert!(!mcp.socket.exists());
}

#[test]
fn a_connection_from_another_user_is_closed_without_an_answer() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    // Подменённый `peer_cred`: сервер считает своим другой uid, значит наш собственный uid для него «чужой».
    let (ours, theirs) = tauri::async_runtime::block_on(async {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        (a, b)
    });
    let uid = own();
    let app = mcp.fixture.app.handle().clone();
    let serving =
        tauri::async_runtime::spawn(
            async move { mcp::serve_connection(app, theirs, uid + 1).await },
        );
    let mut client = {
        let std_stream = ours.into_std().unwrap();
        std_stream.set_nonblocking(false).unwrap();
        std_stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        std_stream
    };
    use std::io::{Read, Write};
    let _ = client.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
    let mut buffer = [0u8; 16];
    assert_eq!(
        client.read(&mut buffer).unwrap_or(0),
        0,
        "соединение закрыто, ответа нет"
    );
    tauri::async_runtime::block_on(serving).unwrap();
}
