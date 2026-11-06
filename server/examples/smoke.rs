//! The client of the compose smoke test (`deploy/smoke.sh`, spike S4c-2): a small program that speaks the HTTP API of appendix §A.6 to a
//! running stack, through its reverse proxy and TLS (a self-signed certificate is accepted: this is a test client). Synthetic data only.
//!
//! - `smoke prepare <state.json> <https://host:port> <ip>`: registers an account, pushes three envelopes, uploads a blob in two parts
//!   (resuming by offset), pulls, and saves what it needs to check the stack later. `<ip>` is where `localhost` (the site name of the
//!   proxy) is reached.
//! - `smoke verify <state.json> <https://host:port> <ip>`: against a stack that was restored from a backup of the first: the old
//!   session is gone, a login works, the old cursor is answered `GenerationChanged`, every envelope comes back byte for byte, and the
//!   blob downloads byte for byte.
//! - `smoke oversize <https://host:port> <ip>`: a push body above the proxy's cap is refused before the server sees it.
// A test client, not app code: it prints its progress and a failed step is the error message (the lints `clippy.toml` relaxes for tests).
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use kinige_crypto::blob::seal_blob;
use kinige_crypto::header::{GroupStamp, Header, Kind};
use kinige_crypto::secret::BlobKey;
use kinige_crypto::{AccountId, Dek, Operation, seal_operation};
use kinige_server::hashing::hash_hex;
use kinige_server::testing::Creds;
use kinige_server::wire::{encode_frames, from_b64, to_b64};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::net::SocketAddr;

const DEVICE: &str = "66666666-6666-4666-8666-666666666666";

/// The wall clock of the HLCs: now, because the stack's server judges `clock_skew` by its own (real) clock.
fn wall() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0))
}

#[derive(Serialize, Deserialize)]
struct State {
    login: String,
    auth_key: String,
    token: String,
    generation: String,
    head: u64,
    /// `(entity, id, op, envelope as base64)` in push order.
    envelopes: Vec<(String, String, String, String)>,
    blob_id: String,
    blob_len: usize,
    blob_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["prepare", state, base, ip] => prepare(state, &client(ip)?, base).await,
        ["verify", state, base, ip] => verify(state, &client(ip)?, base).await,
        ["oversize", base, ip] => oversize(&client(ip)?, base).await,
        _ => Err(
            "usage: smoke prepare|verify <state.json> <base url> <ip> | oversize <base url> <ip>"
                .to_string(),
        ),
    }
}

fn client(ip: &str) -> Result<Client, String> {
    let address: SocketAddr = format!("{ip}:0")
        .parse()
        .map_err(|_| "bad ip".to_string())?;
    Client::builder()
        .danger_accept_invalid_certs(true)
        .resolve("localhost", address)
        .user_agent("kinige-dev")
        .build()
        .map_err(|e| e.to_string())
}

async fn prepare(path: &str, http: &Client, base: &str) -> Result<(), String> {
    let creds = Creds::random();
    let (_, answer) = post(
        http,
        base,
        "/v1/register",
        None,
        &serde_json::to_value(creds.register_request()).unwrap(),
    )
    .await?;
    let token = answer["token"].as_str().ok_or("no token")?.to_string();
    println!("ok: registered");

    // Three envelopes, sealed with a synthetic key (the server never opens them).
    let account = AccountId::from_bytes([0x11; 16]);
    let dek = Dek::generate(1).map_err(|e| e.to_string())?;
    let first = pull(http, base, &token, "0", 0, 0).await?;
    let generation = first["generation"]
        .as_str()
        .ok_or("no generation")?
        .to_string();
    let mut envelopes = Vec::new();
    for n in 1u8..=3 {
        let (id, op) = (uuid(n), uuid(100 + n));
        let header = Header {
            entity: "message".to_string(),
            id: id.clone(),
            op: op.clone(),
            kind: Kind::Upsert,
            groups: BTreeMap::from([(
                "body".to_string(),
                GroupStamp {
                    h: format!("{:013}-{n:05}-{DEVICE}", wall()),
                    sv: 1,
                },
            )]),
            purged_of: None,
            blob: None,
        };
        let payload = BTreeMap::from([(
            "body".to_string(),
            json!({"text": format!("fake smoke entry {n}")}).to_string(),
        )]);
        let bytes = seal_operation(
            &account,
            &dek,
            &Operation {
                header,
                groups: payload,
            },
        )
        .map_err(|e| e.to_string())?;
        envelopes.push(("message".to_string(), id, op, to_b64(&bytes)));
    }
    let frames: Vec<(String, String, String, Vec<u8>)> = envelopes
        .iter()
        .map(|(e, i, o, b)| (e.clone(), i.clone(), o.clone(), from_b64(b).unwrap()))
        .collect();
    let tuples: Vec<(&str, &str, &str, &[u8])> = frames
        .iter()
        .map(|(e, i, o, b)| (e.as_str(), i.as_str(), o.as_str(), b.as_slice()))
        .collect();
    let body = encode_frames(&tuples).ok_or("frames")?;
    let (status, answer) = request(
        http,
        Method::POST,
        &format!("{base}/v1/push"),
        Some(&token),
        &[
            ("x-kinige-cursor", format!("{generation}.0.0")),
            ("x-kinige-protocol", "1".to_string()),
        ],
        body,
    )
    .await?;
    let results = answer["results"].as_array().ok_or("no results")?;
    if status != StatusCode::OK || results.iter().any(|r| r["result"] != "ok") {
        return Err(format!("push: {status} {answer}"));
    }
    println!("ok: pushed {} envelopes", results.len());

    // A blob of 2.5 MiB of plaintext in two parts, the second resuming where the first stopped.
    let blob_id = "01930000-0000-7000-8000-000000000001".to_string();
    let plain: Vec<u8> = (0..(5 << 19)).map(|i: usize| (i % 251) as u8).collect();
    let blob = seal_blob(&plain, &BlobKey::from_bytes([7; 32])).map_err(|e| e.to_string())?;
    let cut = 1_000_000;
    for (from, to) in [(0, cut), (cut, blob.len())] {
        let (status, answer) = request(
            http,
            Method::PUT,
            &format!("{base}/v1/blobs/{blob_id}"),
            Some(&token),
            &[
                ("x-kinige-blob-size", blob.len().to_string()),
                ("x-kinige-blob-offset", from.to_string()),
            ],
            blob[from..to].to_vec(),
        )
        .await?;
        if status != StatusCode::OK {
            return Err(format!("blob upload: {status} {answer}"));
        }
    }
    println!("ok: blob uploaded in two parts");

    let page = pull(http, base, &token, &generation, 0, 0).await?;
    let head = page["head"].as_u64().ok_or("no head")?;
    if page["items"].as_array().map(Vec::len) != Some(3) {
        return Err(format!("pull: {page}"));
    }
    println!("ok: pulled 3 envelopes, head {head}");

    let state = State {
        login: creds.login.clone(),
        auth_key: to_b64(&creds.auth_key),
        token,
        generation,
        head,
        envelopes,
        blob_id,
        blob_len: blob.len(),
        blob_hash: hash_hex(&blob).ok_or("hash")?,
    };
    std::fs::write(path, serde_json::to_vec(&state).unwrap()).map_err(|e| e.to_string())
}

async fn verify(path: &str, http: &Client, base: &str) -> Result<(), String> {
    let state: State = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    // A dump has no sessions: the old token is refused.
    let (status, _) = request(
        http,
        Method::POST,
        &format!("{base}/v1/pull"),
        Some(&state.token),
        &[],
        json!({"generation": state.generation, "after": 0, "known_epoch": 0})
            .to_string()
            .into_bytes(),
    )
    .await?;
    if status != StatusCode::UNAUTHORIZED {
        return Err(format!("the old session still works: {status}"));
    }
    println!("ok: the old session is gone");
    let (_, login) = post(
        http,
        base,
        "/v1/login",
        None,
        &json!({"login": state.login, "auth_key": state.auth_key}),
    )
    .await?;
    let token = login["token"]
        .as_str()
        .ok_or("no token after login")?
        .to_string();
    println!("ok: logged in again");

    // The old cursor is told the generation changed, and gets the cut time of the new one.
    let changed = pull(http, base, &token, &state.generation, state.head, 0).await?;
    if changed["result"] != "generation_changed" || changed["generation"] == json!(state.generation)
    {
        return Err(format!(
            "the old cursor was not told the generation changed: {changed}"
        ));
    }
    if changed["cut_ms"].as_u64().unwrap_or(0) == 0 {
        return Err(format!("no cut time in the answer: {changed}"));
    }
    println!(
        "ok: the old cursor got generation_changed with cut_ms {}",
        changed["cut_ms"]
    );

    // Everything is there, byte for byte.
    let generation = changed["generation"]
        .as_str()
        .ok_or("no generation")?
        .to_string();
    let page = pull(http, base, &token, &generation, 0, 0).await?;
    let items = page["items"].as_array().ok_or("no items")?;
    if items.len() != state.envelopes.len() {
        return Err(format!(
            "{} envelopes after the restore, {} before",
            items.len(),
            state.envelopes.len()
        ));
    }
    for (item, (_, id, op, bytes)) in items.iter().zip(&state.envelopes) {
        if item["id"] != json!(id) || item["op"] != json!(op) || item["envelope"] != json!(bytes) {
            return Err(format!("envelope {id} differs after the restore"));
        }
    }
    println!("ok: {} envelopes, byte for byte", items.len());

    let (status, _, bytes) = raw(
        http,
        Method::GET,
        &format!("{base}/v1/blobs/{}", state.blob_id),
        Some(&token),
        &[],
        Vec::new(),
    )
    .await?;
    if status != StatusCode::OK
        || bytes.len() != state.blob_len
        || hash_hex(&bytes).as_deref() != Some(state.blob_hash.as_str())
    {
        return Err(format!(
            "the blob is not what it was: {status}, {} bytes",
            bytes.len()
        ));
    }
    let (status, _, part) = raw(
        http,
        Method::GET,
        &format!("{base}/v1/blobs/{}", state.blob_id),
        Some(&token),
        &[("range", "bytes=1000-1999".to_string())],
        Vec::new(),
    )
    .await?;
    if status != StatusCode::PARTIAL_CONTENT || part != bytes[1000..2000] {
        return Err(format!("the range of the blob is wrong: {status}"));
    }
    println!("ok: the blob downloads byte for byte, and a range of it");
    Ok(())
}

async fn oversize(http: &Client, base: &str) -> Result<(), String> {
    let body = vec![0u8; 20 << 20];
    let (status, _) = request(
        http,
        Method::POST,
        &format!("{base}/v1/push"),
        Some("x"),
        &[("x-kinige-cursor", "1.0.0".to_string())],
        body,
    )
    .await?;
    if status != StatusCode::PAYLOAD_TOO_LARGE {
        return Err(format!("a 20 MiB push body got {status}, not 413"));
    }
    println!("ok: a 20 MiB push body is refused with 413 at the proxy");
    Ok(())
}

async fn pull(
    http: &Client,
    base: &str,
    token: &str,
    generation: &str,
    after: u64,
    epoch: u64,
) -> Result<Value, String> {
    let body =
        json!({"generation": generation, "after": after, "known_epoch": epoch, "limit": 100});
    let (status, answer) = post(http, base, "/v1/pull", Some(token), &body).await?;
    if status != StatusCode::OK {
        return Err(format!("pull: {status}"));
    }
    Ok(answer)
}

async fn post(
    http: &Client,
    base: &str,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> Result<(StatusCode, Value), String> {
    let (status, answer) = request(
        http,
        Method::POST,
        &format!("{base}{path}"),
        token,
        &[],
        body.to_string().into_bytes(),
    )
    .await?;
    if !status.is_success() {
        return Err(format!("{path}: {status} {answer}"));
    }
    Ok((status, answer))
}

async fn request(
    http: &Client,
    method: Method,
    url: &str,
    token: Option<&str>,
    headers: &[(&str, String)],
    body: Vec<u8>,
) -> Result<(StatusCode, Value), String> {
    let (status, _, bytes) = raw(http, method, url, token, headers, body).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    ))
}

async fn raw(
    http: &Client,
    method: Method,
    url: &str,
    token: Option<&str>,
    headers: &[(&str, String)],
    body: Vec<u8>,
) -> Result<(StatusCode, reqwest::header::HeaderMap, Vec<u8>), String> {
    let mut request = http.request(method, url).body(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = request.send().await.map_err(|e| format!("{url}: {e}"))?;
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok((status, headers, bytes))
}

fn uuid(n: u8) -> String {
    format!("{n:08x}-0000-4000-8000-{n:012x}")
}
