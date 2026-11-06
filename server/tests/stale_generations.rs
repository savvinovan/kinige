//! The stale rule and the generations over HTTP (ADR-0001 §6.3, appendix §A.3 `stale`, ADR-0017): a push and a pull whose cursor is behind a
//! drop it missed, and the placement of a generation by the lineage.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

/// Entity 1 is pushed, purged, and compacted away 181 days later: a drop at seq 2, epoch 1. Entity 2 stays at seq 3.
async fn make_a_drop(rig: &Rig, acct: &Acct, maker: &Maker) {
    put(rig, acct, 1, 50, &msg(maker, 1, 50, T0)).await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(rig, acct, 1, 51, &purge).await;
    put(rig, acct, 2, 52, &msg(maker, 2, 52, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(rig, acct).await;
}

#[tokio::test]
async fn stale_when_the_pushers_cursor_is_behind_a_drop_or_in_any_generation_but_the_current() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let gen_ = generation(&rig).await;
    make_a_drop(&rig, &acct, &maker).await;

    let fresh = msg(&maker, 3, 53, T0 + 181 * DAY);
    let (item_id, item_op) = (id(3), id(53));
    let item = Item {
        entity: "message",
        id: &item_id,
        op: &item_op,
        bytes: &fresh,
    };
    let probe = |cursor: String| {
        let (rig, acct, item) = (&rig, &acct, &item);
        async move {
            let (status, answer) = push_with(rig, acct, &cursor, Some("1"), item).await;
            assert_eq!(status, StatusCode::OK, "{answer}");
            outcome(&answer["results"][0]).to_string()
        }
    };
    assert_eq!(
        probe(cursor(gen_, 1, 0)).await,
        "stale",
        "behind the drop, the epoch not seen"
    );
    assert_eq!(
        probe(cursor(gen_ + 1, 3, 1)).await,
        "stale",
        "a generation that is not the current one, whether the lineage knows it or not"
    );
    assert_eq!(
        probe(cursor(gen_, 1, 1)).await,
        "ok",
        "the device already saw epoch 1"
    );
    let next = msg(&maker, 4, 54, T0 + 181 * DAY);
    let (item_id, item_op) = (id(4), id(54));
    let at_zero = push_with(
        &rig,
        &acct,
        &cursor(gen_, 0, 0),
        Some("1"),
        &Item {
            entity: "message",
            id: &item_id,
            op: &item_op,
            bytes: &next,
        },
    )
    .await;
    assert_eq!(
        outcome(&at_zero.1["results"][0]),
        "ok",
        "a cursor at 0 is never stale"
    );
}

#[tokio::test]
async fn a_pull_says_stale_or_generation_changed_by_the_rule_of_the_lineage() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let gen_ = generation(&rig).await;
    make_a_drop(&rig, &acct, &maker).await;

    let (_, answer) = pull(&rig, &acct, gen_, 1, 0, 10).await;
    assert_eq!(
        answer["result"], "stale",
        "a drop above the cursor that the device has not seen"
    );
    assert_eq!(answer["epoch"], 1);
    let (_, answer) = pull(&rig, &acct, gen_, 1, 1, 10).await;
    assert_eq!(answer["result"], "page", "the device has seen epoch 1");
    let (_, answer) = pull(&rig, &acct, gen_, 0, 0, 10).await;
    assert_eq!(
        (answer["result"].clone(), answer["epoch"].clone()),
        (json!("page"), json!(1)),
        "a pull from 0 is never stale"
    );

    // A restore adds a generation to the lineage (S4c-2); here the row is written by hand. The cursor of the old generation is told.
    let client = rig.state.pool.get().await.expect("connection");
    client
        .execute("INSERT INTO lineage (pos, generation) VALUES (2, 777)", &[])
        .await
        .expect("lineage");
    let (_, answer) = pull(&rig, &acct, gen_, 1, 0, 10).await;
    assert_eq!(
        (
            answer["result"].clone(),
            answer["generation"].clone(),
            answer["stale"].clone()
        ),
        (json!("generation_changed"), json!("777"), json!(true)),
        "the drop was made in the old generation above the cursor: both causes at once"
    );
    let (_, answer) = pull(&rig, &acct, gen_, 1, 1, 10).await;
    assert_eq!(answer["stale"], false, "the device had seen the drop");
    let (_, answer) = pull(&rig, &acct, 777, 0, 0, 10).await;
    assert_eq!(answer["result"], "page");
    let (_, answer) = pull(&rig, &acct, 12345, 5, 0, 10).await;
    assert_eq!(
        answer["result"], "generation_changed",
        "a generation outside the lineage"
    );
}

/// The S4c-1 carry-over: a pusher whose generation is in the lineage but is not the current one is stale, also when it has missed no drop
/// (it pulls first, gets `GenerationChanged`, and resyncs); the pusher in the current generation is not.
#[tokio::test]
async fn a_push_from_a_generation_in_the_lineage_but_not_the_current_one_is_stale() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let old = generation(&rig).await;
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    // A restore appended a generation (written by hand here; `tests/backup_restore.rs` restores for real). No drop exists at all.
    let client = rig.state.pool.get().await.expect("connection");
    client
        .execute("INSERT INTO lineage (pos, generation, cut_ms) VALUES (2, 888, 5)", &[])
        .await
        .expect("lineage");
    let fresh = msg(&maker, 2, 51, T0);
    let (item_id, item_op) = (id(2), id(51));
    let item = Item {
        entity: "message",
        id: &item_id,
        op: &item_op,
        bytes: &fresh,
    };
    let (status, answer) = push_with(&rig, &acct, &cursor(old, 1, 0), Some("1"), &item).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        outcome(&answer["results"][0]),
        "stale",
        "in the lineage, not current, up to date: stale all the same"
    );
    assert_eq!(log_seqs(&rig, &acct).await, vec![1], "and nothing was stored");
    let (_, answer) = push_with(&rig, &acct, &cursor(888, 1, 0), Some("1"), &item).await;
    assert_eq!(outcome(&answer["results"][0]), "ok");
}
