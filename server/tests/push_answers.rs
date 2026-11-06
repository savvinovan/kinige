//! The answers to a push over HTTP against a real Postgres: `ok`, `duplicate`, `op_conflict`, `too_old`, `clock_skew`, `purged` and
//! `stale_purge` of ADR-0001 appendix §A.3, in the order of §A.4 steps 3-7. Each test names the row or the step it pins.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::*;
use kinige_server::config::Config;
use serde_json::json;

#[tokio::test]
async fn duplicate_for_the_same_bytes_and_op_conflict_for_other_bytes() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    assert_eq!(put(&rig, &acct, 1, 50, &bytes).await["seq"], 1);
    let again = put(&rig, &acct, 1, 50, &bytes).await;
    assert_eq!((outcome(&again), &again["seq"]), ("duplicate", &json!(1)));
    // The nonce is random, so sealing the same record under the same op twice gives other bytes (ADR-0001 §6.1).
    let rebuilt = msg(&maker, 1, 50, T0);
    let conflict = put(&rig, &acct, 1, 50, &rebuilt).await;
    assert_eq!(outcome(&conflict), "op_conflict");
    let next = msg(&maker, 2, 51, T0);
    let ok = put(&rig, &acct, 2, 51, &next).await;
    assert_eq!(
        ok["seq"], 2,
        "neither the duplicate nor the conflict consumed a seq"
    );
}

#[tokio::test]
async fn too_old_below_the_minimum_protocol_but_a_duplicate_still_gets_its_answer() {
    let rig = rig_with(Config {
        min_protocol: 2,
        ..Config::default()
    })
    .await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    let (item_id, item_op) = (id(1), id(50));
    let item = Item {
        entity: "message",
        id: &item_id,
        op: &item_op,
        bytes: &bytes,
    };
    let current = cursor(generation(&rig).await, 0, 0);
    for protocol in [None, Some("0"), Some("1")] {
        let (status, answer) = push_with(&rig, &acct, &current, protocol, &item).await;
        assert_eq!(
            (status, outcome(&answer["results"][0])),
            (StatusCode::OK, "too_old"),
            "{protocol:?}"
        );
    }
    assert!(log_seqs(&rig, &acct).await.is_empty());
    let (_, accepted) = push_with(&rig, &acct, &current, Some("2"), &item).await;
    assert_eq!(outcome(&accepted["results"][0]), "ok");
    // Step 3 comes before step 4: a repeat of an accepted envelope is a duplicate whatever the client's version.
    let (_, repeat) = push_with(&rig, &acct, &current, Some("1"), &item).await;
    assert_eq!(outcome(&repeat["results"][0]), "duplicate");
    let (status, _) = push_with(&rig, &acct, &current, Some("v1"), &item).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a protocol header that is not a number"
    );
}

#[tokio::test]
async fn clock_skew_beyond_24_hours_and_nothing_is_stored() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let fine = msg(&maker, 1, 50, T0 + DAY);
    assert_eq!(outcome(&put(&rig, &acct, 1, 50, &fine).await), "ok");
    let skewed = msg(&maker, 2, 51, T0 + DAY + 1);
    assert_eq!(
        outcome(&put(&rig, &acct, 2, 51, &skewed).await),
        "clock_skew"
    );
    // A `purged` snapshot is held to the same limit.
    let purge = maker.purged(
        "message",
        &id(3),
        &id(52),
        &[("body", hlc(T0 + DAY + 1, 0))],
    );
    assert_eq!(
        outcome(&put(&rig, &acct, 3, 52, &purge).await),
        "clock_skew"
    );
    assert_eq!(log_seqs(&rig, &acct).await, vec![1]);
    // The server's clock decides: a millisecond later the same envelope is fine.
    rig.clock.advance(1);
    assert_eq!(outcome(&put(&rig, &acct, 2, 51, &skewed).await), "ok");
}

#[tokio::test]
async fn purged_stale_purge_and_a_repeated_purged_with_the_accepted_snapshot() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let two_groups = maker.upsert(
        "message",
        &id(1),
        &id(50),
        &[("body", hlc(T0 - 10, 0)), ("core", hlc(T0 - 20, 0))],
        "fake",
    );
    put(&rig, &acct, 1, 50, &two_groups).await;

    // A group missing from the snapshot counts as newer than it.
    let missing = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0 - 10, 0))]);
    assert_eq!(
        outcome(&put(&rig, &acct, 1, 51, &missing).await),
        "stale_purge"
    );
    // A group newer than the snapshot's.
    let older = maker.purged(
        "message",
        &id(1),
        &id(52),
        &[("body", hlc(T0 - 11, 0)), ("core", hlc(T0 - 20, 0))],
    );
    assert_eq!(
        outcome(&put(&rig, &acct, 1, 52, &older).await),
        "stale_purge"
    );
    // A snapshot that covers every known group is accepted.
    let accepted = [("body", hlc(T0 - 10, 0)), ("core", hlc(T0 - 20, 0))];
    let purge = maker.purged("message", &id(1), &id(53), &accepted);
    let result = put(&rig, &acct, 1, 53, &purge).await;
    assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(2)));

    // After a purge, every other envelope of the entity gets `purged`.
    let late = maker.upsert("message", &id(1), &id(54), &[("body", hlc(T0, 0))], "fake");
    assert_eq!(outcome(&put(&rig, &acct, 1, 54, &late).await), "purged");
    // A repeated `purged` is a no-op `ok` that carries the snapshot the server accepted, not its own.
    let again = maker.purged("message", &id(1), &id(55), &[("body", hlc(T0 - 1, 0))]);
    let result = put(&rig, &acct, 1, 55, &again).await;
    assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(2)));
    assert_eq!(
        result["purged_of"],
        json!({"body": hlc(T0 - 10, 0), "core": hlc(T0 - 20, 0)})
    );
    assert_eq!(
        log_seqs(&rig, &acct).await,
        vec![1, 2],
        "nothing was stored by the refusals and the repeat"
    );

    // Step 3 before step 6: a repeat of an accepted upsert is a duplicate, not `purged`.
    assert_eq!(
        outcome(&put(&rig, &acct, 1, 50, &two_groups).await),
        "duplicate"
    );
    // A purge of an entity the server never saw is accepted (a device purged what it never pushed).
    let unseen = maker.purged("message", &id(9), &id(56), &[("body", hlc(T0, 0))]);
    assert_eq!(outcome(&put(&rig, &acct, 9, 56, &unseen).await), "ok");
}

#[tokio::test]
async fn an_account_sees_only_its_own_log() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    put(&rig, &a, 1, 50, &bytes).await;
    // The same ids in another account are another record with its own numbering (§7: the store is keyed by the account).
    let result = put(&rig, &b, 1, 50, &bytes).await;
    assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(1)));
    let (_, page) = pull(&rig, &b, generation(&rig).await, 0, 0, 10).await;
    assert_eq!(page["items"].as_array().map(Vec::len), Some(1));
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &a, 1, 51, &purge).await;
    let late = maker.upsert(
        "message",
        &id(1),
        &id(52),
        &[("body", hlc(T0 + 1, 0))],
        "fake",
    );
    assert_eq!(
        outcome(&put(&rig, &b, 1, 52, &late).await),
        "ok",
        "A's purge does not touch B's record"
    );
}
