//! Benchmark of voice notes (slice 7a): `send_voice` for 1 MiB and 15 MiB bodies, and opening a chat that holds 200 voice notes. Prints
//! only numbers. Targets: send p95 under 100 ms (1 MiB) and under 500 ms (15 MiB); a chat of 200 voice notes opens in under 300 ms (p95).
//!
//! Usage (the data directory is temporary, not the real database!):
//! `cargo run --release -p kinige-store --example voice_bench -- <data directory>`

// The example is a console utility: printing to stdout and panicking on any error are allowed.
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::string_slice
)]

use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, StoreOptions};
use std::path::PathBuf;
use std::time::Instant;

const MIB: usize = 1024 * 1024;
const RUNS: usize = 30;

fn main() {
    let data = PathBuf::from(std::env::args().nth(1).expect("data directory"));
    std::fs::create_dir_all(&data).unwrap();
    let mut options = StoreOptions::new(data.join("kinige.db"), Some(data.join("mirror")));
    options.start_mirror = false;
    options.start_maintenance = false;
    let store = Store::open(options).unwrap();
    let now = Now::system();

    let body = |size: usize| {
        let mut bytes = vec![0, 0, 0, 0x20];
        bytes.extend_from_slice(b"ftypM4A ");
        bytes.extend(
            (0..size.saturating_sub(12)).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8),
        );
        bytes
    };
    for (label, size, limit_ms) in [("1 MiB", MIB, 100.0), ("15 MiB", 15 * MIB, 500.0)] {
        let bytes = body(size);
        let mut times: Vec<f64> = (0..RUNS)
            .map(|_| {
                let t = Instant::now();
                store
                    .send_voice(INBOX_ID, None, &bytes, 40_000, &now)
                    .unwrap();
                ms(t)
            })
            .collect();
        println!(
            "send_voice {label}: {} (limit p95 {limit_ms} ms)",
            stats(&mut times)
        );
    }

    // Top the chat up to 200 voice notes (small bodies: the cost of opening is rows and JSON, not bytes).
    let small = body(4096);
    let have = store
        .list_messages(INBOX_ID, None, 500, &now)
        .unwrap()
        .messages
        .len();
    for _ in have..200 {
        store
            .send_voice(INBOX_ID, None, &small, 40_000, &now)
            .unwrap();
    }
    let mut opens: Vec<f64> = (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            let page = store.list_messages(INBOX_ID, None, 200, &now).unwrap();
            assert!(page.messages.iter().all(|m| !m.attachments.is_empty()));
            ms(t)
        })
        .collect();
    println!(
        "open a chat of 200 voice notes (list_messages, 200 entries): {} (limit p95 300 ms)",
        stats(&mut opens)
    );
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// "median 3.1 ms, p95 5.0 ms, max 6.2 ms over 30 runs".
fn stats(times: &mut [f64]) -> String {
    times.sort_by(f64::total_cmp);
    let at = |share: f64| times[((times.len() as f64 - 1.0) * share).round() as usize];
    format!(
        "median {:.1} ms, p95 {:.1} ms, max {:.1} ms over {} runs",
        at(0.5),
        at(0.95),
        times[times.len() - 1],
        times.len()
    )
}
