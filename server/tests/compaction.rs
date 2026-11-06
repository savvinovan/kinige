//! Compaction on Postgres with the server's injected clock (ADR-0001 §6.1, §10.2, ADR-0017): a `purged` older than 180 days goes with its
//! entity's envelopes and is a drop with a new epoch, the generation it was made in and the time; a covered `upsert` goes without one.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::*;
use kinige_server::Clock;
use kinige_server::rules::pull::Drop;
use kinige_server::store::compact;

const MSG: u8 = 1;

async fn compacted(rig: &Rig, acct: &Acct) -> compact::Stats {
    compact::account(&rig.state.pool, &acct.creds.account_id, rig.clock.now_ms())
        .await
        .expect("compaction")
}

async fn drops(rig: &Rig, acct: &Acct) -> Vec<Drop> {
    compact::drops(&rig.state.pool, &acct.creds.account_id)
        .await
        .expect("drops")
}

async fn count(rig: &Rig, table: &str, acct: &Acct) -> i64 {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query_one(
            &format!("SELECT count(*) FROM {table} WHERE account_id = $1"),
            &[&acct.creds.account_id],
        )
        .await
        .expect("count")
        .get(0)
}

#[tokio::test]
async fn a_purged_older_than_180_days_goes_with_its_entity_and_is_a_drop_with_a_time() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let g = generation(&rig).await;
    let id = uuid(MSG);
    let body_h = hlc(T0 - 5, 0);
    let up = maker.upsert(
        "message",
        &id,
        &uuid(50),
        &[("body", body_h.clone())],
        "fake",
    );
    let flags = maker.upsert(
        "message",
        &id,
        &uuid(51),
        &[("flags", hlc(T0 - 4, 0))],
        "fake",
    );
    let purge = maker.purged(
        "message",
        &id,
        &uuid(52),
        &[("body", body_h), ("flags", hlc(T0 - 4, 0))],
    );
    let other = maker.upsert(
        "message",
        &uuid(2),
        &uuid(53),
        &[("body", hlc(T0, 0))],
        "fake",
    );
    for (n, bytes) in [(50, &up), (51, &flags), (52, &purge)] {
        let item = Item {
            entity: "message",
            id: &id,
            op: &uuid(n),
            bytes,
        };
        assert_eq!(outcome(&push(&rig, &acct, &item).await), "ok");
    }
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &uuid(2),
            op: &uuid(53),
            bytes: &other,
        },
    )
    .await;

    rig.clock.advance(180 * DAY);
    assert_eq!(
        compacted(&rig, &acct).await,
        compact::Stats::default(),
        "180 days exactly is not beyond the retention"
    );
    assert_eq!(log_seqs(&rig, &acct).await, vec![1, 2, 3, 4]);

    rig.clock.advance(1);
    let stats = compacted(&rig, &acct).await;
    assert_eq!(
        stats,
        compact::Stats {
            removed: 3,
            dropped: true
        }
    );
    assert_eq!(
        log_seqs(&rig, &acct).await,
        vec![4],
        "the entity's envelopes went with its purged"
    );
    let found = drops(&rig, &acct).await;
    assert_eq!(found.len(), 1);
    let drop = found.first().copied().expect("a drop");
    assert_eq!(
        (drop.generation, drop.epoch, drop.seq, drop.compacted_ms),
        (g, 1, 3, rig.clock.now_ms()),
        "the generation it was made in, the new epoch, the highest seq dropped and the time"
    );
    assert_eq!(count(&rig, "purged_entities", &acct).await, 0);
    assert_eq!(
        count(&rig, "entity_known", &acct).await,
        1,
        "only the live entity is indexed"
    );

    // The head does not move back, and a pull from 0 is never stale: it sees the epoch.
    let (_, page) = pull(&rig, &acct, g, 0, 0, 10).await;
    assert_eq!(
        (
            page["head"].clone(),
            page["epoch"].clone(),
            page["items"][0]["seq"].clone()
        ),
        (
            serde_json::json!(4),
            serde_json::json!(1),
            serde_json::json!(4)
        )
    );
    // A repeat is a no-op, not a second drop.
    assert_eq!(compacted(&rig, &acct).await, compact::Stats::default());
    assert_eq!(drops(&rig, &acct).await.len(), 1);
    // The memory of an accepted op outlives its envelope (§A.4 step 3).
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id,
            op: &uuid(50),
            bytes: &up,
        },
    )
    .await;
    assert_eq!(
        (outcome(&result), &result["seq"]),
        ("duplicate", &serde_json::json!(1))
    );
}

#[tokio::test]
async fn a_covered_upsert_goes_without_a_drop_and_a_half_covered_one_stays() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let id = uuid(MSG);
    let first = maker.upsert(
        "message",
        &id,
        &uuid(50),
        &[("body", hlc(T0 - 30, 0)), ("flags", hlc(T0 - 30, 0))],
        "fake",
    );
    let second = maker.upsert(
        "message",
        &id,
        &uuid(51),
        &[("body", hlc(T0 - 20, 0))],
        "fake",
    );
    for (n, bytes) in [(50, &first), (51, &second)] {
        let item = Item {
            entity: "message",
            id: &id,
            op: &uuid(n),
            bytes,
        };
        push(&rig, &acct, &item).await;
    }
    rig.clock.advance(181 * DAY);
    let third = maker.upsert(
        "message",
        &id,
        &uuid(52),
        &[("body", hlc(T0 + 181 * DAY, 0))],
        "fake",
    );
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id,
            op: &uuid(52),
            bytes: &third,
        },
    )
    .await;
    let stats = compacted(&rig, &acct).await;
    assert_eq!(
        stats,
        compact::Stats {
            removed: 1,
            dropped: false
        },
        "seq 2 is covered by seq 3; seq 1 holds a `flags` nobody else has"
    );
    assert_eq!(log_seqs(&rig, &acct).await, vec![1, 3]);
    assert!(
        drops(&rig, &acct).await.is_empty(),
        "covered upserts are not a drop"
    );
    // So a device with a cursor behind them is not stale (§6.3): a later envelope says the same.
    let (_, page) = pull(&rig, &acct, generation(&rig).await, 1, 0, 10).await;
    assert_eq!(page["result"], "page");
}

#[tokio::test]
async fn every_account_is_compacted_on_its_own_and_young_envelopes_are_left_alone() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    for acct in [&a, &b] {
        let up = maker.upsert(
            "message",
            &uuid(1),
            &uuid(50),
            &[("body", hlc(T0 - 5, 0))],
            "fake",
        );
        push(
            &rig,
            acct,
            &Item {
                entity: "message",
                id: &uuid(1),
                op: &uuid(50),
                bytes: &up,
            },
        )
        .await;
    }
    let purge = maker.purged("message", &uuid(1), &uuid(51), &[("body", hlc(T0 - 5, 0))]);
    push(
        &rig,
        &a,
        &Item {
            entity: "message",
            id: &uuid(1),
            op: &uuid(51),
            bytes: &purge,
        },
    )
    .await;
    rig.clock.advance(181 * DAY);
    let removed = compact::all(&rig.state.pool, rig.clock.now_ms())
        .await
        .expect("all");
    assert_eq!(removed, 2, "account A's two, none of B's");
    assert_eq!(log_seqs(&rig, &a).await, Vec::<u64>::new());
    assert_eq!(log_seqs(&rig, &b).await, vec![1]);
    assert_eq!(
        (drops(&rig, &a).await.len(), drops(&rig, &b).await.len()),
        (1, 0)
    );
}
