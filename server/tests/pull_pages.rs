//! Pull pages over HTTP against a real Postgres (ADR-0001 §6.1, appendix §A.6): the envelopes with the names the server declares, the
//! cursor `next`, the byte cap of a page, and what a pull refuses.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::*;
use kinige_server::config::Config;
use kinige_server::dto::PullResponse;
use kinige_server::wire::from_b64;
use serde_json::{Value, json};

#[tokio::test]
async fn ok_assigns_consecutive_seqs_and_a_pull_returns_the_bytes_with_the_declared_names() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let mut sent = Vec::new();
    for n in 1..=5u8 {
        let bytes = msg(&maker, n, 100 + n, T0 - 1000);
        let result = put(&rig, &acct, n, 100 + n, &bytes).await;
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
    // At the head the page is empty and the cursor stays; past the head it does not move back or forward.
    let (_, empty) = pull(&rig, &acct, gen_, 5, 0, 2).await;
    assert_eq!(
        (
            empty["next"].clone(),
            empty["items"].as_array().map(Vec::len)
        ),
        (json!(5), Some(0))
    );
    let (_, beyond) = pull(&rig, &acct, gen_, 9, 0, 2).await;
    assert_eq!(
        (
            beyond["next"].clone(),
            beyond["head"].clone(),
            beyond["items"].as_array().map(Vec::len)
        ),
        (json!(9), json!(5), Some(0)),
        "after > head: next = after"
    );
}

#[tokio::test]
async fn a_page_stops_at_the_byte_cap_and_its_first_envelope_always_goes() {
    let maker = Maker::new();
    // Four envelopes of one size: the cap that fits one and a byte more takes two per page.
    let envelopes: Vec<Vec<u8>> = (1..=4u8).map(|n| msg(&maker, n, 100 + n, T0)).collect();
    let size = envelopes.first().map_or(0, Vec::len);
    for (page_bytes, per_page) in [(size + 1, 2usize), (1, 1), (0, 1)] {
        let rig = rig_with(Config {
            page_bytes,
            ..Config::default()
        })
        .await;
        let acct = account(&rig).await;
        for (n, bytes) in (1..=4u8).zip(&envelopes) {
            put(&rig, &acct, n, 100 + n, bytes).await;
        }
        let gen_ = generation(&rig).await;
        let (mut after, mut seen) = (0u64, Vec::new());
        for _ in 0..10 {
            let (_, page) = pull(&rig, &acct, gen_, after, 0, 1000).await;
            let items = page["items"].as_array().expect("items");
            assert!(
                !items.is_empty() && items.len() <= per_page,
                "a cap of {page_bytes} bytes: {} items",
                items.len()
            );
            seen.extend(items.iter().filter_map(|i| i["seq"].as_u64()));
            after = page["next"].as_u64().expect("next");
            if after >= page["head"].as_u64().expect("head") {
                break;
            }
            assert_eq!(
                Some(after),
                items.last().and_then(|i| i["seq"].as_u64()),
                "a page cut by bytes ends at its last envelope"
            );
        }
        assert_eq!(
            seen,
            vec![1, 2, 3, 4],
            "a cap of {page_bytes} bytes skips nothing"
        );
    }
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
