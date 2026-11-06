//! Helpers shared by the flow tests. Every password, login and key here is synthetic and obviously fake.
#![allow(dead_code)]

use kinige_crypto::envelope::open;
use kinige_crypto::header::{GroupStamp, Header, Kind};
use kinige_crypto::secret::{Password, SecretBytes, ServerSecret};
use kinige_crypto::{AccountId, Dek, Operation, open_operation, seal_operation};
use s4b_keys::client::{Client, ClientError};
use s4b_keys::link::Link;
use s4b_keys::log::Log;
use s4b_keys::server::{Server, Token};
use std::collections::BTreeMap;

pub const LOGIN: &str = "fake.user@example.test";
pub const PASSWORD: &str = "fake password one, not real";
pub const NEW_PASSWORD: &str = "fake password two, not real";

pub fn password(text: &str) -> Password {
    SecretBytes::new(text.as_bytes().to_vec())
}

pub fn world() -> (Server, Log) {
    let log = Log::new();
    (
        Server::new(ServerSecret::from_bytes([0x61; 32]), log.clone()),
        log,
    )
}

/// A new device on the same log and server.
pub fn device(log: &Log) -> Client {
    Client::new(log.clone())
}

/// Registers `login` on a new device, confirming "Save", and returns the device with the words it was shown.
pub fn register(server: &mut Server, log: &Log, login: &str, pw: &str) -> (Client, String) {
    let mut client = device(log);
    client
        .begin_registration(login, &password(pw))
        .expect("begin");
    let words = client
        .shown_words()
        .expect("words shown")
        .phrase()
        .to_string();
    client.confirm_saved().expect("save");
    client
        .send_registration(&mut Link::new(server))
        .expect("register");
    (client, words)
}

pub fn login(
    server: &mut Server,
    client: &mut Client,
    login: &str,
    pw: &str,
) -> Result<(), ClientError> {
    client.login(&mut Link::new(server), login, &password(pw))
}

pub fn token(client: &Client) -> Token {
    client
        .account()
        .and_then(|account| account.token.clone())
        .expect("a token")
}

pub fn account_id(client: &Client) -> AccountId {
    client.account().expect("account").account_id
}

pub fn dek(client: &Client) -> Dek {
    client.dek().expect("dek").clone()
}

/// A real envelope for a message with a text, sealed with the client's DEK for its account.
pub fn write_envelope(client: &Client, server: &mut Server, n: u32) -> Vec<u8> {
    let account = account_id(client);
    let dek = dek(client);
    let hlc = format!(
        "{:013}-{:05}-66666666-6666-4666-8666-666666666666",
        1_762_300_000_000u64 + u64::from(n),
        0
    );
    let id = format!("00000000-0000-4000-8000-{n:012}");
    let header = Header {
        entity: "message".to_string(),
        id,
        op: format!("11111111-0000-4000-8000-{n:012}"),
        kind: Kind::Upsert,
        groups: BTreeMap::from([("body".to_string(), GroupStamp { h: hlc, sv: 1 })]),
        purged_of: None,
        blob: None,
    };
    let groups = BTreeMap::from([(
        "body".to_string(),
        format!(
            r#"{{"base":null,"body_md":"fake entry number {n}","edited_at":null,"supersedes":[]}}"#
        ),
    )]);
    let bytes = seal_operation(&account, &dek, &Operation { header, groups }).expect("seal");
    server.push_envelope(&token(client), &bytes).expect("push");
    bytes
}

/// Every envelope the server holds for the client's account, opened with the client's DEK.
pub fn read_all(client: &Client, server: &Server) -> Vec<String> {
    let account = account_id(client);
    let dek = dek(client);
    server
        .pull_envelopes(&token(client))
        .expect("pull")
        .iter()
        .map(|bytes| {
            let operation =
                open_operation(bytes, &account, &dek).expect("opens with the key from this device");
            let _ = open(bytes, &account, &dek).expect("opens");
            operation.groups["body"].clone()
        })
        .collect()
}
