//! The one measurement of S4a (ADR-0001 §6.2 item 4, "to be measured in S4a"): binding a database of N entries (a Telegram import) to an
//! account. `cargo run --release --bin measure -- [N]`. It measures the protocol's share: building the envelopes from the database
//! (the real merge/format code of `kinige-domain`, plaintext), the server taking them in order, and the bytes an encrypted envelope
//! would have on the wire. Encryption (S4b), the network and SQLite (slice 11) are not in it.

#![allow(clippy::print_stdout)]

use s4a_sync::client::{Client, inbox_key};
use s4a_sync::envelope::{Entity, Envelope};
use s4a_sync::server::{PushResponse, Server};
use s4a_sync::switches::Switches;
use serde_json::json;
use std::time::Instant;

const T0: u64 = 1_800_000_000_000;
const CHATS: usize = 40;

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);
    println!(
        "binding a database of {n} entries in {CHATS} chats (synthetic text, 300-700 bytes, 5 years of days)"
    );

    // The local database: what slice 4's import leaves (all records `server_known = 0`, no outbox, no account).
    let started = Instant::now();
    let mut client = Client::new(0, 0, false, false, Switches::default(), T0);
    let chats: Vec<_> = (0..CHATS).filter_map(|_| client.create_chat(T0)).collect();
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for i in 0..n {
        let day = usize::try_from(next() % 1826).unwrap_or(0);
        let date = chrono_free_date(day);
        let words = 50 + usize::try_from(next() % 50).unwrap_or(0);
        let body: String = (0..words)
            .map(|_| WORDS[usize::try_from(next()).unwrap_or(0) % WORDS.len()])
            .collect::<Vec<_>>()
            .join(" ");
        let chat = &chats[i % chats.len()];
        client.create_message_with(chat, &body, &date, T0 + i as u64);
        client.writes.clear();
    }
    println!(
        "local database built: {:.2} s ({} records)",
        started.elapsed().as_secs_f64(),
        client.records.len()
    );

    // Binding: log in, full pull from 0 (the server is empty), then an envelope for every record that never left the device.
    let mut server = Server::new(Switches::default());
    let started = Instant::now();
    client.begin_binding(server.generation);
    while client.resync.is_some() {
        let (generation, after, epoch) = client.pull_request();
        let response = server.pull(generation, after, epoch, 500);
        client.on_pull_response(response, T0, |_| {});
    }
    let build = started.elapsed();
    let envelopes: Vec<_> = client.outbox.iter().map(|row| row.env.clone()).collect();
    println!(
        "envelopes built from the database: {} in {:.2} s ({:.1} us each)",
        envelopes.len(),
        build.as_secs_f64(),
        build.as_secs_f64() * 1e6 / envelopes.len().max(1) as f64
    );

    let started = Instant::now();
    let mut accepted = 0usize;
    for env in &envelopes {
        if matches!(server.push(env, T0), PushResponse::Ok(_)) {
            accepted += 1;
        }
    }
    let push = started.elapsed();
    println!(
        "the in-memory server took {accepted} of them in {:.2} s ({:.1} us each)",
        push.as_secs_f64(),
        push.as_secs_f64() * 1e6 / accepted.max(1) as f64
    );

    let (mut plain, mut wire, mut largest) = (0usize, 0usize, 0usize);
    for env in &envelopes {
        let payload = payload_len(env);
        let size = wire_size(env);
        plain += payload;
        wire += size;
        largest = largest.max(size);
    }
    println!(
        "payload (plaintext JSON of the groups): {:.1} MB; wire estimate with the header, nonce, tag and padding to 1 KiB (ADR-0001 appendix A.1): {:.1} MB, average {} bytes, largest {} bytes",
        plain as f64 / 1e6,
        wire as f64 / 1e6,
        wire / envelopes.len().max(1),
        largest
    );
    for batch in [100usize, 500, 1000] {
        println!(
            "  push in batches of {batch}: {} requests",
            envelopes.len().div_ceil(batch)
        );
    }
    let messages = envelopes
        .iter()
        .filter(|e| e.header.entity == Entity::Message)
        .count();
    println!(
        "of the envelopes: {messages} messages, {} chats (Inbox included: {})",
        envelopes.len() - messages,
        client.records.contains_key(&inbox_key())
    );
}

/// A date `day` days after 2021-01-01 in `YYYY-MM-DD`, without a calendar crate: the days of a 365-day year are enough here, the
/// text only has to be a real date (day 1..=28 of each month).
fn chrono_free_date(day: usize) -> String {
    let year = 2021 + day / 336;
    let month = 1 + (day % 336) / 28;
    let dom = 1 + day % 28;
    format!("{year}-{month:02}-{dom:02}")
}

/// The payload of §5.3: `{"groups":{"<g>": <canonical JSON of the group>}}`.
fn payload_len(env: &Envelope) -> usize {
    let groups: serde_json::Map<String, serde_json::Value> = env
        .payload
        .iter()
        .map(|(name, raw)| {
            (
                name.clone(),
                serde_json::from_str(raw).unwrap_or(serde_json::Value::Null),
            )
        })
        .collect();
    json!({"groups": groups}).to_string().len()
}

/// `1 + 4 + 4 + header + 24 + (payload padded to a multiple of 1024, at least one byte of padding) + 16` (appendix A.1).
fn wire_size(env: &Envelope) -> usize {
    let groups: serde_json::Map<String, serde_json::Value> = env
        .header
        .groups
        .iter()
        .map(|(g, (h, sv))| (g.clone(), json!({"h": h, "sv": sv})))
        .collect();
    let header = json!({"entity": format!("{:?}", env.header.entity).to_lowercase(), "id": env.header.id, "op": env.header.op_id, "kind": "upsert", "groups": groups}).to_string().len();
    let padded = (payload_len(env) / 1024 + 1) * 1024;
    1 + 4 + 4 + header + 24 + padded + 16
}

const WORDS: &[&str] = &[
    "morning",
    "coffee",
    "walk",
    "river",
    "meeting",
    "idea",
    "book",
    "rain",
    "train",
    "call",
    "dinner",
    "plan",
    "draft",
    "winter",
    "garden",
    "market",
    "music",
    "letter",
    "window",
    "ticket",
    "project",
    "trip",
    "lunch",
    "photo",
    "note",
    "list",
    "later",
    "again",
    "maybe",
    "today",
    "утро",
    "кофе",
    "прогулка",
    "река",
    "встреча",
    "идея",
    "книга",
    "дождь",
    "поезд",
    "звонок",
    "ужин",
    "план",
    "зима",
    "сад",
    "рынок",
];
