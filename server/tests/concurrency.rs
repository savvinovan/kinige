//! ADR-0001 appendix §B item 23 and PRD §10.2 (criteria S4c): parallel pushes of one account never make the cursor skip a record, and
//! pushes of two accounts do not wait for each other. The first test has a demonstrated negative control: `seq` from a plain Postgres
//! sequence, without the account lock, makes the same check fail. The control lives here as raw SQL; nothing like it is in the server.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::*;
use kinige_server::Clock;
use kinige_server::store::compact;
use kinige_server::testing::Http;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::{sleep, timeout};

const PUSHERS: u32 = 8;
const PER_PUSHER: u32 = 20;
const TOTAL: u64 = (PUSHERS * PER_PUSHER) as u64;

/// A record id and an op id that no other pusher, round or test uses.
fn ids(round: u32, pusher: u32, n: u32) -> (String, String) {
    let base = u64::from(round) * 1_000_000 + u64::from(pusher) * 1000 + u64::from(n);
    (
        format!("{:08x}-0000-4000-8000-{:012x}", 1, base),
        format!("{:08x}-0000-4000-8000-{:012x}", 2, base),
    )
}

/// The `seq`s missing from `collected` below its maximum, and any repeated: what "the cursor skipped a record" looks like.
fn defects(collected: &[u64], total: u64) -> Vec<String> {
    let mut out = Vec::new();
    let mut sorted = collected.to_vec();
    sorted.sort_unstable();
    if sorted.windows(2).any(|w| w.first() == w.get(1)) {
        out.push("a seq was seen twice".to_string());
    }
    let missing: Vec<u64> = (1..=total).filter(|s| !sorted.contains(s)).collect();
    if !missing.is_empty() {
        out.push(format!("never collected: {missing:?}"));
    }
    if collected != sorted.as_slice() {
        out.push("collected out of order".to_string());
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_pushers_and_a_puller_never_skip_a_seq() {
    let rig = rig().await;
    for round in 0..3 {
        let acct = Arc::new(account(&rig).await);
        let generation = generation(&rig).await;
        let done = Arc::new(AtomicBool::new(false));

        let mut pushers = Vec::new();
        for pusher in 0..PUSHERS {
            let (http, acct) = (rig.http.clone(), Arc::clone(&acct));
            pushers.push(tokio::spawn(async move {
                let maker = Maker::new();
                for n in 0..PER_PUSHER {
                    let (id, op) = ids(round, pusher, n);
                    let bytes =
                        maker.upsert("message", &id, &op, &[("body", hlc(T0 - 5, n))], "fake");
                    let (_, answer) = http
                        .push(
                            Some(&acct.token),
                            &cursor(generation, 0, 0),
                            Some("1"),
                            body(&[Item {
                                entity: "message",
                                id: &id,
                                op: &op,
                                bytes: &bytes,
                            }]),
                        )
                        .await;
                    assert_eq!(answer["results"][0]["result"], "ok", "{answer}");
                }
            }));
        }

        // The puller runs the whole time, with small pages, and keeps what the cursor passed.
        let puller = {
            let (http, acct, done) = (rig.http.clone(), Arc::clone(&acct), Arc::clone(&done));
            tokio::spawn(async move {
                let (mut after, mut collected) = (0u64, Vec::new());
                loop {
                    let finished = done.load(Ordering::SeqCst);
                    let (_, page) = http
                        .post("/v1/pull", Some(&acct.token), &json!({"generation": generation.to_string(), "after": after, "known_epoch": 0, "limit": 7}))
                        .await;
                    for item in page["items"].as_array().expect("items") {
                        collected.push(item["seq"].as_u64().expect("seq"));
                    }
                    after = page["next"].as_u64().expect("next");
                    if finished && after >= page["head"].as_u64().expect("head") {
                        return collected;
                    }
                    sleep(Duration::from_millis(1)).await;
                }
            })
        };
        for pusher in pushers {
            pusher.await.expect("pusher");
        }
        done.store(true, Ordering::SeqCst);
        let collected = puller.await.expect("puller");
        assert_eq!(
            defects(&collected, TOTAL),
            Vec::<String>::new(),
            "round {round}"
        );
        assert_eq!(
            collected,
            log_seqs(&rig, &acct).await,
            "the cursor saw exactly the log"
        );
        assert_eq!(collected.len() as u64, TOTAL);
    }
}

/// The negative control: the same check, with `seq` from a plain sequence and no account lock. A transaction takes `n`, another takes
/// `n + 1` and commits first, a reader moves its cursor past `n + 1`, and `n` appears behind it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn control_seq_from_a_plain_sequence_without_the_lock_does_skip_records() {
    let rig = rig().await;
    let pool = rig.db.pool().expect("pool");
    pool.get()
        .await
        .expect("connection")
        .batch_execute("CREATE SEQUENCE ctrl_seq")
        .await
        .expect("sequence");
    let mut worst = Vec::new();
    for attempt in 0..5 {
        let acct = Arc::new(account(&rig).await);
        let first = {
            let client = pool.get().await.expect("connection");
            u64::try_from(client.query_one("SELECT CASE WHEN is_called THEN last_value ELSE last_value - 1 END FROM ctrl_seq", &[]).await.expect("last").get::<_, i64>(0)).expect("positive")
        };
        let done = Arc::new(AtomicBool::new(false));
        let mut pushers = Vec::new();
        for pusher in 0..PUSHERS {
            let (pool, acct) = (pool.clone(), Arc::clone(&acct));
            pushers.push(tokio::spawn(async move {
                for n in 0..PER_PUSHER {
                    let mut client = pool.get().await.expect("connection");
                    let tx = client.transaction().await.expect("tx");
                    let seq: i64 = tx.query_one("SELECT nextval('ctrl_seq')", &[]).await.expect("nextval").get(0);
                    // Another transaction can take the next number and commit while this one is still between taking its own and committing.
                    sleep(Duration::from_millis(u64::from((pusher * 7 + n * 3) % 11))).await;
                    let (id, op) = ids(900 + attempt, pusher, n);
                    tx.execute(
                        "INSERT INTO envelopes (account_id, seq, op_id, entity, entity_id, kind, groups, at_ms, len, bytes) \
                         VALUES ($1, $2, $3, 'message', $4, 'upsert', '{}', 0, 1, '\\x00')",
                        &[&acct.creds.account_id, &seq, &op, &id],
                    )
                    .await
                    .expect("insert");
                    tx.commit().await.expect("commit");
                }
            }));
        }
        let puller = {
            let (pool, acct, done) = (pool.clone(), Arc::clone(&acct), Arc::clone(&done));
            tokio::spawn(async move {
                // The lock-free design's reader: the head is the highest committed `seq`, and the cursor moves to the last one it read.
                let (mut after, mut collected) = (first, Vec::new());
                loop {
                    let finished = done.load(Ordering::SeqCst);
                    let client = pool.get().await.expect("connection");
                    let rows = client
                        .query("SELECT seq FROM envelopes WHERE account_id = $1 AND seq > $2 ORDER BY seq", &[&acct.creds.account_id, &i64::try_from(after).expect("fits")])
                        .await
                        .expect("read");
                    for row in &rows {
                        let seq = u64::try_from(row.get::<_, i64>(0)).expect("positive");
                        collected.push(seq);
                        after = seq;
                    }
                    if finished && rows.is_empty() {
                        return collected;
                    }
                    sleep(Duration::from_millis(1)).await;
                }
            })
        };
        for pusher in pushers {
            pusher.await.expect("pusher");
        }
        done.store(true, Ordering::SeqCst);
        let collected = puller.await.expect("puller");
        // The very check the real test applies, on this round's numbers counted from 1.
        let relative: Vec<u64> = collected.iter().map(|seq| seq - first).collect();
        let found = defects(&relative, TOTAL);
        if !found.is_empty() {
            worst = found;
            break;
        }
    }
    assert!(
        !worst.is_empty(),
        "five rounds of eight racing writers on a plain sequence never skipped a record: the control proves nothing"
    );
    println!("negative control: the check of the real test fails on a plain sequence: {worst:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_accounts_do_not_block_each_other_and_one_account_does_wait_for_its_lock() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let generation = generation(&rig).await;

    // Hold A's lock the way a push in flight does.
    let mut holder = rig.state.pool.get().await.expect("connection");
    let lock = holder.transaction().await.expect("tx");
    lock.query_one(
        "SELECT seq_head FROM accounts WHERE id = $1 FOR UPDATE",
        &[&a.creds.account_id],
    )
    .await
    .expect("lock");

    let send = |http: Http, token: String, n: u8, maker_bytes: Vec<u8>| async move {
        let (id, op) = (uuid(n), uuid(100 + n));
        http.push(
            Some(&token),
            &cursor(generation, 0, 0),
            Some("1"),
            body(&[Item {
                entity: "message",
                id: &id,
                op: &op,
                bytes: &maker_bytes,
            }]),
        )
        .await
        .1
    };
    let for_a = maker.upsert(
        "message",
        &uuid(1),
        &uuid(101),
        &[("body", hlc(T0 - 5, 0))],
        "fake",
    );
    let for_b = maker.upsert(
        "message",
        &uuid(2),
        &uuid(102),
        &[("body", hlc(T0 - 5, 0))],
        "fake",
    );
    let blocked = tokio::spawn(send(rig.http.clone(), a.token.clone(), 1, for_a));

    // B is not held up by A's lock, and neither is a reader of A.
    let answer = timeout(
        Duration::from_secs(2),
        send(rig.http.clone(), b.token.clone(), 2, for_b),
    )
    .await
    .expect("B's push finished while A was locked");
    assert_eq!(answer["results"][0]["result"], "ok");
    let (_, page) = timeout(Duration::from_secs(2), pull(&rig, &a, generation, 0, 0, 10))
        .await
        .expect("a pull of A finished while A was locked");
    assert_eq!(page["head"], 0);

    // A's own push waits, and goes through once the lock is released.
    sleep(Duration::from_millis(300)).await;
    assert!(
        !blocked.is_finished(),
        "A's push did not wait for A's lock: seq would not be issued under it"
    );
    lock.commit().await.expect("release");
    drop(holder);
    let answer = timeout(Duration::from_secs(5), blocked)
        .await
        .expect("A's push finished after the release")
        .expect("task");
    assert_eq!(
        (
            &answer["results"][0]["result"],
            &answer["results"][0]["seq"]
        ),
        (&json!("ok"), &json!(1))
    );
}

/// The mirror of the test above: compaction takes the same account lock as a push, so it cannot interleave with one; it waits for a
/// lock that is held, and it does not hold up another account.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn compaction_waits_for_a_held_account_lock_and_not_for_another_account() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let mut holder = rig.state.pool.get().await.expect("connection");
    let lock = holder.transaction().await.expect("tx");
    lock.query_one(
        "SELECT seq_head FROM accounts WHERE id = $1 FOR UPDATE",
        &[&a.creds.account_id],
    )
    .await
    .expect("lock");

    let now = rig.clock.now_ms();
    let pool = rig.state.pool.clone();
    let id_a = a.creds.account_id.clone();
    let waiting = tokio::spawn(async move { compact::account(&pool, &id_a, now).await });
    timeout(
        Duration::from_secs(2),
        compact::account(&rig.state.pool, &b.creds.account_id, now),
    )
    .await
    .expect("B's compaction finished while A was locked")
    .expect("compaction");
    sleep(Duration::from_millis(300)).await;
    assert!(
        !waiting.is_finished(),
        "A's compaction did not wait for A's lock: it could interleave with a push"
    );
    lock.commit().await.expect("release");
    drop(holder);
    timeout(Duration::from_secs(5), waiting)
        .await
        .expect("A's compaction finished after the release")
        .expect("task")
        .expect("compaction");
}
