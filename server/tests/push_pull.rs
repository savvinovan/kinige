//! Push and pull over HTTP against a real Postgres: every answer of ADR-0001 appendix §A.3, the server's check order of §A.4, the pages of
//! §6.1. Each test names the row or the step it pins.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use kinige_server::Clock;
use kinige_server::config::Config;
use kinige_server::dto::PullResponse;
use kinige_server::wire::{encode_frames, from_b64};
use serde_json::{Value, json};

const MSG: usize = 1;

fn id(n: u8) -> String {
    uuid(n)
}

/// A one-group upsert of message `n` at `wall`, with op `op_n`.
fn msg(maker: &Maker, n: u8, op_n: u8, wall: u64) -> Vec<u8> {
    maker.upsert(
        "message",
        &id(n),
        &id(op_n),
        &[("body", hlc(wall, 0))],
        "fake entry",
    )
}

#[tokio::test]
async fn ok_assigns_consecutive_seqs_and_a_pull_returns_the_bytes_with_the_declared_names() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let mut sent = Vec::new();
    for n in 1..=5u8 {
        let bytes = msg(&maker, n, 100 + n, T0 - 1000);
        let result = push(
            &rig,
            &acct,
            &Item {
                entity: "message",
                id: &id(n),
                op: &id(100 + n),
                bytes: &bytes,
            },
        )
        .await;
        assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(n)));
        sent.push(bytes);
    }
    let gen_ = generation(&rig).await;
    let mut seen = Vec::new();
    let mut after = 0;
    let mut pages = 0;
    loop {
        let (status, page) = pull(&rig, &acct, gen_, after, 0, 2).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        let PullResponse::Page {
            next,
            head,
            items,
            after: echoed,
            generation,
            ..
        } = serde_json::from_value(page).expect("page")
        else {
            panic!("a page");
        };
        assert_eq!(
            (head, echoed, generation.as_str()),
            (5, after, gen_.to_string().as_str())
        );
        for item in &items {
            let index = usize::try_from(item.seq).expect("seq") - 1;
            assert_eq!(
                from_b64(&item.envelope).as_deref(),
                sent.get(index).map(Vec::as_slice)
            );
            assert_eq!(
                (item.entity.as_str(), item.id.as_str(), item.op.as_str()),
                (
                    "message",
                    id(item.seq as u8).as_str(),
                    id(100 + item.seq as u8).as_str()
                )
            );
            seen.push(item.seq);
        }
        pages += 1;
        if next >= head {
            break;
        }
        assert_eq!(
            Some(next),
            items.last().map(|i| i.seq),
            "a truncated page ends at its last envelope"
        );
        after = next;
    }
    assert_eq!((seen, pages), (vec![1, 2, 3, 4, 5], 3));
    let (_, empty) = pull(&rig, &acct, gen_, 5, 0, 2).await;
    assert_eq!(
        (
            empty["next"].clone(),
            empty["items"].as_array().map(Vec::len)
        ),
        (json!(5), Some(0))
    );
}

#[tokio::test]
async fn duplicate_for_the_same_bytes_and_op_conflict_for_other_bytes() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let bytes = msg(&maker, MSG as u8, 50, T0);
    let item = Item {
        entity: "message",
        id: &id(MSG as u8),
        op: &id(50),
        bytes: &bytes,
    };
    assert_eq!(push(&rig, &acct, &item).await["seq"], 1);
    let again = push(&rig, &acct, &item).await;
    assert_eq!((outcome(&again), &again["seq"]), ("duplicate", &json!(1)));
    // The nonce is random, so sealing the same record under the same op twice gives other bytes (ADR-0001 §6.1).
    let rebuilt = msg(&maker, MSG as u8, 50, T0);
    let conflict = push(
        &rig,
        &acct,
        &Item {
            bytes: &rebuilt,
            ..item
        },
    )
    .await;
    assert_eq!(outcome(&conflict), "op_conflict");
    let next = msg(&maker, 2, 51, T0);
    let ok = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(2),
            op: &id(51),
            bytes: &next,
        },
    )
    .await;
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
    let item = Item {
        entity: "message",
        id: &id(1),
        op: &id(50),
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
    assert_eq!(
        outcome(
            &push(
                &rig,
                &acct,
                &Item {
                    entity: "message",
                    id: &id(1),
                    op: &id(50),
                    bytes: &fine
                }
            )
            .await
        ),
        "ok"
    );
    let skewed = msg(&maker, 2, 51, T0 + DAY + 1);
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(2),
            op: &id(51),
            bytes: &skewed,
        },
    )
    .await;
    assert_eq!(outcome(&result), "clock_skew");
    // A `purged` snapshot is held to the same limit.
    let purge = maker.purged(
        "message",
        &id(3),
        &id(52),
        &[("body", hlc(T0 + DAY + 1, 0))],
    );
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(3),
            op: &id(52),
            bytes: &purge,
        },
    )
    .await;
    assert_eq!(outcome(&result), "clock_skew");
    assert_eq!(log_seqs(&rig, &acct).await, vec![1]);
    // The server's clock decides: a day later the same envelope is fine.
    rig.clock.advance(1);
    assert_eq!(
        outcome(
            &push(
                &rig,
                &acct,
                &Item {
                    entity: "message",
                    id: &id(2),
                    op: &id(51),
                    bytes: &skewed
                }
            )
            .await
        ),
        "ok"
    );
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
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &two_groups,
        },
    )
    .await;

    // A group missing from the snapshot counts as newer than it.
    let missing = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0 - 10, 0))]);
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(51),
            bytes: &missing,
        },
    )
    .await;
    assert_eq!(outcome(&result), "stale_purge");
    // A group newer than the snapshot's.
    let older = maker.purged(
        "message",
        &id(1),
        &id(52),
        &[("body", hlc(T0 - 11, 0)), ("core", hlc(T0 - 20, 0))],
    );
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(52),
            bytes: &older,
        },
    )
    .await;
    assert_eq!(outcome(&result), "stale_purge");
    // A snapshot that covers every known group is accepted.
    let accepted = [("body", hlc(T0 - 10, 0)), ("core", hlc(T0 - 20, 0))];
    let purge = maker.purged("message", &id(1), &id(53), &accepted);
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(53),
            bytes: &purge,
        },
    )
    .await;
    assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(2)));

    // After a purge, every other envelope of the entity gets `purged`.
    let late = maker.upsert("message", &id(1), &id(54), &[("body", hlc(T0, 0))], "fake");
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(54),
            bytes: &late,
        },
    )
    .await;
    assert_eq!(outcome(&result), "purged");
    // A repeated `purged` is a no-op `ok` that carries the snapshot the server accepted, not its own.
    let again = maker.purged("message", &id(1), &id(55), &[("body", hlc(T0 - 1, 0))]);
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(55),
            bytes: &again,
        },
    )
    .await;
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
    let result = push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &two_groups,
        },
    )
    .await;
    assert_eq!(outcome(&result), "duplicate");
    // A purge of an entity the server never saw is accepted (a device purged what it never pushed).
    let unseen = maker.purged("message", &id(9), &id(56), &[("body", hlc(T0, 0))]);
    assert_eq!(
        outcome(
            &push(
                &rig,
                &acct,
                &Item {
                    entity: "message",
                    id: &id(9),
                    op: &id(56),
                    bytes: &unseen
                }
            )
            .await
        ),
        "ok"
    );
}

#[tokio::test]
async fn stale_when_the_pushers_cursor_is_behind_a_drop_or_in_another_generation() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let gen_ = generation(&rig).await;
    // Entity 1 is pushed, purged, and compacted away 181 days later: a drop at seq 2, epoch 1.
    let up = msg(&maker, 1, 50, T0);
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &up,
        },
    )
    .await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(51),
            bytes: &purge,
        },
    )
    .await;
    let other = msg(&maker, 2, 52, T0);
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(2),
            op: &id(52),
            bytes: &other,
        },
    )
    .await;
    rig.clock.advance(181 * DAY);
    kinige_server::store::compact::account(
        &rig.state.pool,
        &acct.creds.account_id,
        rig.clock.now_ms(),
    )
    .await
    .expect("compaction");

    let fresh = msg(&maker, 3, 53, T0 + 181 * DAY);
    let item = Item {
        entity: "message",
        id: &id(3),
        op: &id(53),
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
        "a generation the server does not have"
    );
    assert_eq!(
        probe(cursor(gen_, 1, 1)).await,
        "ok",
        "the device already saw epoch 1"
    );
    let next = msg(&maker, 4, 54, T0 + 181 * DAY);
    let at_zero = push_with(
        &rig,
        &acct,
        &cursor(gen_, 0, 0),
        Some("1"),
        &Item {
            entity: "message",
            id: &id(4),
            op: &id(54),
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
async fn lengths_and_headers_are_judged_before_the_session_and_the_bodies_before_they_are_buffered()
{
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    let good = body(&[Item {
        entity: "message",
        id: &id(1),
        op: &id(50),
        bytes: &bytes,
    }]);
    let current = cursor(generation(&rig).await, 0, 0);

    // Step 1 (400) comes before step 2 (401): garbage without a session is a 400, a valid body without one is a 401.
    let (status, answer) = rig
        .http
        .push(
            None,
            &current,
            Some("1"),
            b"garbage that is not a frame".to_vec(),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("frame_entity"))
    );
    let (status, _) = rig.http.push(None, &current, Some("1"), good.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = rig
        .http
        .push(Some("not-a-token"), &current, Some("1"), good.clone())
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The declared envelope size is refused with nothing of the envelope sent.
    let mut huge = vec![7u8];
    huge.extend_from_slice(b"message");
    huge.extend_from_slice(&[0x11; 32]);
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    let (status, answer) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), huge)
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    // A body that declares more than the cap in its Content-Length is refused before any of it is read.
    let request = Request::builder()
        .method("POST")
        .uri("/v1/push")
        .header("x-kinige-cursor", &current)
        .header("x-kinige-protocol", "1")
        .header("authorization", format!("Bearer {}", acct.token))
        .header(
            "content-length",
            (Config::default().max_push_body + 1).to_string(),
        )
        .body(Body::from(good.clone()))
        .expect("request");
    assert_eq!(
        rig.http.send(request).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );

    // The header must say what the frame declares; an unknown format byte and a bad header length are 400s.
    let wrong = encode_frames(&[("message", &id(2), &id(50), &bytes[..])]).expect("frame");
    let (status, answer) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), wrong)
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("header_mismatch"))
    );
    let mut future = bytes.clone();
    future[0] = 2;
    let (_, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            body(&[Item {
                entity: "message",
                id: &id(1),
                op: &id(50),
                bytes: &future,
            }]),
        )
        .await;
    assert_eq!(answer["error"], "unsupported_format");
    let (_, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            good[..good.len() - 1].to_vec(),
        )
        .await;
    assert_eq!(answer["error"], "frame_truncated");
    assert!(
        log_seqs(&rig, &acct).await.is_empty(),
        "nothing of a refused request was written"
    );

    // The cursor header is required and strict.
    for bad in [
        "",
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "1.-2.3",
        "1.2.99999999999999999999",
    ] {
        let (status, answer) = rig
            .http
            .push(Some(&acct.token), bad, Some("1"), good.clone())
            .await;
        assert_eq!(
            (status, answer["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("cursor")),
            "{bad:?}"
        );
    }
    let (status, _) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), good)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_batch_has_a_cap_and_every_envelope_of_a_batch_is_answered_in_order() {
    let rig = rig_with(Config {
        max_batch: 3,
        ..Config::default()
    })
    .await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let envelopes: Vec<Vec<u8>> = (1..=4u8).map(|n| msg(&maker, n, 100 + n, T0)).collect();
    let ids: Vec<(String, String)> = (1..=4u8).map(|n| (id(n), id(100 + n))).collect();
    let items: Vec<Item<'_>> = envelopes
        .iter()
        .zip(&ids)
        .map(|(bytes, (i, o))| Item {
            entity: "message",
            id: i,
            op: o,
            bytes,
        })
        .collect();
    let current = cursor(generation(&rig).await, 0, 0);
    let (status, _) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), body(&items))
        .await;
    assert_eq!(
        status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "four frames, a cap of three"
    );
    let (status, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            body(items.get(..3).expect("three")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let results = parse_push(&answer).results;
    assert_eq!(
        results.iter().map(|r| r.op.clone()).collect::<Vec<_>>(),
        vec![id(101), id(102), id(103)]
    );
    assert_eq!(
        results.iter().filter_map(|r| r.seq).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn a_pull_says_stale_or_generation_changed_by_the_rule_of_the_lineage() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let gen_ = generation(&rig).await;
    let up = msg(&maker, 1, 50, T0);
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &up,
        },
    )
    .await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    push(
        &rig,
        &acct,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(51),
            bytes: &purge,
        },
    )
    .await;
    rig.clock.advance(181 * DAY);
    kinige_server::store::compact::account(
        &rig.state.pool,
        &acct.creds.account_id,
        rig.clock.now_ms(),
    )
    .await
    .expect("compaction");

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

#[tokio::test]
async fn a_pull_refuses_what_it_cannot_read_with_a_4xx() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let gen_ = generation(&rig).await.to_string();
    let cases: Vec<(&str, Value)> = vec![
        (
            "generation",
            json!({"generation": "abc", "after": 0, "known_epoch": 0}),
        ),
        (
            "generation",
            json!({"generation": "-1", "after": 0, "known_epoch": 0}),
        ),
        (
            "after",
            json!({"generation": gen_, "after": 18446744073709551615u64, "known_epoch": 0}),
        ),
        (
            "known_epoch",
            json!({"generation": gen_, "after": 0, "known_epoch": 9223372036854775808u64}),
        ),
        (
            "json",
            json!({"generation": gen_, "after": -1, "known_epoch": 0}),
        ),
        (
            "json",
            json!({"generation": 5, "after": 0, "known_epoch": 0}),
        ),
        ("json", json!({})),
        ("json", json!([1, 2])),
    ];
    for (code, request) in cases {
        let (status, answer) = rig.http.post("/v1/pull", Some(&acct.token), &request).await;
        assert_eq!(
            (status, answer["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some(code)),
            "{request}"
        );
    }
    let (status, _) = rig
        .http
        .post(
            "/v1/pull",
            None,
            &json!({"generation": gen_, "after": 0, "known_epoch": 0}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // The page size is clamped, not refused.
    let (status, answer) = rig
        .http
        .post(
            "/v1/pull",
            Some(&acct.token),
            &json!({"generation": gen_, "after": 0, "known_epoch": 0, "limit": 0}),
        )
        .await;
    assert_eq!(
        (status, answer["result"].clone()),
        (StatusCode::OK, json!("page"))
    );
}

#[tokio::test]
async fn an_account_sees_only_its_own_log() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    push(
        &rig,
        &a,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &bytes,
        },
    )
    .await;
    // The same ids in another account are another record with its own numbering (§7: the store is keyed by the account).
    let result = push(
        &rig,
        &b,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(50),
            bytes: &bytes,
        },
    )
    .await;
    assert_eq!((outcome(&result), &result["seq"]), ("ok", &json!(1)));
    let (_, page) = pull(&rig, &b, generation(&rig).await, 0, 0, 10).await;
    assert_eq!(page["items"].as_array().map(Vec::len), Some(1));
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    push(
        &rig,
        &a,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(51),
            bytes: &purge,
        },
    )
    .await;
    let late = maker.upsert(
        "message",
        &id(1),
        &id(52),
        &[("body", hlc(T0 + 1, 0))],
        "fake",
    );
    let result = push(
        &rig,
        &b,
        &Item {
            entity: "message",
            id: &id(1),
            op: &id(52),
            bytes: &late,
        },
    )
    .await;
    assert_eq!(
        outcome(&result),
        "ok",
        "A's purge does not touch B's record"
    );
}
