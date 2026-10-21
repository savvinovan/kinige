//! Замер задержки отправки (ТЗ §5: запись в ленте < 100 мс). Зависит от диска, поэтому в обычный прогон не входит:
//! `cargo test -p kinige-store --test latency -- --ignored --nocapture`.

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use std::time::Instant;

#[test]
#[ignore = "замер зависит от диска; запускается вручную"]
fn send_latency_with_full_synchronous_commit() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let mut samples: Vec<f64> = Vec::new();
    for i in 0..200 {
        let started = Instant::now();
        store
            .send_message(INBOX_ID, &format!("замер {i}"), &moscow(T0 + i))
            .unwrap();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.total_cmp(b));
    let median = samples[samples.len() / 2];
    let p95 = samples[samples.len() * 95 / 100];
    let max = samples[samples.len() - 1];
    println!(
        "отправка (транзакция + fsync, synchronous=FULL + fullfsync), 200 записей: медиана {median:.2} мс, p95 {p95:.2} мс, максимум {max:.2} мс"
    );
    assert!(p95 < 100.0, "p95 {p95:.1} мс");
}
