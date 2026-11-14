//! The lineage of generations across restores (ADR-0001 §6.3, ADR-0017, ADR-0020): a branch that a restore abandoned is placed by where it
//! forked (also when it was itself restored from a backup of a branch), and the lineage is capped. Over HTTP, on real dumps. The other
//! rules a restore applies are in `restore_rules.rs`.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::*;
use serde_json::{Value, json};

#[tokio::test]
async fn a_branch_cut_from_an_older_backup_than_the_one_restored_now_is_placed_by_its_fork() {
    // The S4a scenario `review_round_3_a_branch_cut_from_an_older_backup_than_the_one_restored_now`, over HTTP with a real dump.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let (older, older_manifest) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    put(&rig, &acct, 2, 52, &msg(&maker, 2, 52, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    rig.clock.advance(1);
    let (newer, newer_manifest) = take_into(&rig, &root).await;
    assert!(older_manifest.cut_ms < newer_manifest.cut_ms);

    // The installation is restored from the older backup: a branch, G2, cut at the older backup's time. A device enters it from 0.
    let installation = clean_like(&rig).await;
    restore_into(&installation, &older).await;
    let branch = generation(&installation).await;
    let device = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (_, answer) = pull(&installation, &device, 0, 0, 0, 10).await;
    assert_eq!(answer["result"], "generation_changed");
    assert_eq!(answer["generation"], branch.to_string());
    let lineage = answer["lineage"].clone();
    assert_eq!(lineage[1]["cut_ms"], older_manifest.cut_ms);

    // Then the newer backup is restored over it: the lineage is [G1, G3] and G2 is abandoned.
    let report = restore_into(&installation, &newer).await;
    let current = report.generation;
    assert_ne!(current, branch);
    let server = rows(
        &installation,
        "SELECT generation::text AS g FROM lineage ORDER BY pos",
    )
    .await;
    assert_eq!(server.len(), 2, "{server:?}");
    assert!(
        !server.iter().any(|l| l.contains(&branch.to_string())),
        "the branch is not in the lineage"
    );
    let device = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };

    // The device on the branch is told it missed the drop: the drop was compacted after the branch's fork. Sending no lineage is answered
    // the same way (the safe side), and a branch forked after the drop has missed nothing.
    let ask = |lineage: Option<Value>, after: u64| {
        let mut body = json!({"generation": branch.to_string(), "after": after, "known_epoch": 0});
        if let Some(lineage) = lineage {
            body["lineage"] = lineage;
        }
        let (rig, device) = (&installation, &device);
        async move {
            rig.http
                .post("/v1/pull", Some(&device.token), &body)
                .await
                .1
        }
    };
    let answer = ask(Some(lineage.clone()), 1).await;
    assert_eq!(
        (answer["result"].as_str(), answer["stale"].as_bool()),
        (Some("generation_changed"), Some(true)),
        "{answer}"
    );
    assert_eq!(answer["generation"], current.to_string());
    assert_eq!(
        answer["lineage"][1]["cut_ms"], newer_manifest.cut_ms,
        "the new generation was cut from the newer backup"
    );
    assert_eq!(ask(None, 1).await["stale"], true);
    let after_the_drop = json!([
        {"generation": lineage[0]["generation"], "cut_ms": 0},
        {"generation": branch.to_string(), "cut_ms": newer_manifest.cut_ms},
    ]);
    assert_eq!(ask(Some(after_the_drop), 1).await["stale"], false);
    assert_eq!(
        ask(Some(lineage), 0).await["stale"],
        false,
        "a pull from 0 is never stale"
    );
    // A lineage that is too long or not made of numbers is a 400.
    let long: Vec<Value> = (0..65)
        .map(|n| json!({"generation": n.to_string(), "cut_ms": 0}))
        .collect();
    for bad in [
        Value::Array(long),
        json!([{"generation": "x", "cut_ms": 0}]),
        json!([{"generation": "1"}]),
        json!("nope"),
    ] {
        let (status, _) = installation
            .http
            .post(
                "/v1/pull",
                Some(&device.token),
                &json!({"generation": "1", "after": 1, "known_epoch": 0, "lineage": bad}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    // A lineage that does not end with the cursor's generation is malformed too (round 2 of the review): it is not the lineage that put
    // the device into that generation, and the server does not guess which of the two is meant.
    let ends_elsewhere = json!([{"generation": "7", "cut_ms": 0}]);
    let (status, answer) = installation
        .http
        .post(
            "/v1/pull",
            Some(&device.token),
            &json!({"generation": "8", "after": 1, "known_epoch": 0, "lineage": ends_elsewhere}),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("lineage"))
    );

    // A push carries no lineage, its cursor has three parts, and a pusher whose generation is not the current one is stale whatever
    // it says: it pulls first (appendix §A.3).
    let item_op = id(70);
    let fresh = msg(&maker, 3, 70, T0 + 182 * DAY);
    let item = Item {
        entity: "message",
        id: &id(3),
        op: &item_op,
        bytes: &fresh,
    };
    let probe = |cursor: String| {
        let (rig, device, item) = (&installation, &device, &item);
        async move { push_with(rig, device, &cursor, Some("1"), item).await }
    };
    let (status, answer) = probe(format!("{branch}.1.0")).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(outcome(&answer["results"][0]), "stale");
    assert_eq!(
        probe(format!("{branch}.1.0.5")).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, answer) = probe(format!("{current}.3.1")).await;
    assert_eq!(
        outcome(&answer["results"][0]),
        "ok",
        "a pusher in the current generation, up to date"
    );
}

#[tokio::test]
async fn a_branch_that_was_itself_restored_is_placed_by_where_it_forked_and_not_by_its_own_cut() {
    // The round 1 critical of the guardian, on real dumps. B1 is cut; a message X is purged and the drop K is compacted; B2 is cut and holds
    // it. B1 is restored (generation G1), a backup B3 of that branch is cut after K and restored (G2): the device enters G2 from 0 and has X
    // alive. Then B2 is restored: the server's lineage is [G0, G3]. The device's own cut time (B3's) is after K; it forked from the server's
    // history at G1, cut at B1, before K, so it has missed K and X must not be pushed back.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let (b1, b1_manifest) = take(&rig).await;
    let root = b1.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    put(&rig, &acct, 2, 52, &msg(&maker, 2, 52, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    let drop_time = field(
        &rows(&rig, "SELECT compacted_ms FROM drops").await[0],
        "compacted_ms",
    );
    rig.clock.advance(1);
    let (b2, _) = take_into(&rig, &root).await;

    let installation = clean_like(&rig).await;
    let g1 = restore_into(&installation, &b1).await.generation;
    let (b3, b3_manifest) = take_into(&installation, &root).await;
    assert!(b3_manifest.cut_ms > drop_time, "B3 is cut after K");
    let g2 = restore_into(&installation, &b3).await.generation;
    let session = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (_, entered) = pull(&installation, &session, 0, 0, 0, 10).await;
    assert_eq!(entered["generation"], g2.to_string());
    let lineage = entered["lineage"].clone();
    assert_eq!(
        lineage.as_array().map(Vec::len),
        Some(3),
        "[G0, G1, G2]: {lineage}"
    );
    assert_eq!(lineage[1]["generation"], g1.to_string());
    assert_eq!(lineage[1]["cut_ms"], b1_manifest.cut_ms);
    assert_eq!(lineage[2]["cut_ms"], b3_manifest.cut_ms);

    restore_into(&installation, &b2).await;
    let session = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let ask = |lineage: Value| {
        let body =
            json!({"generation": g2.to_string(), "after": 1, "known_epoch": 0, "lineage": lineage});
        let (rig, session) = (&installation, &session);
        async move {
            rig.http
                .post("/v1/pull", Some(&session.token), &body)
                .await
                .1
        }
    };
    let answer = ask(lineage.clone()).await;
    assert_eq!(answer["result"], "generation_changed", "{answer}");
    assert_eq!(
        answer["stale"], true,
        "the fork is G1, cut before K: K is missed"
    );
    // What the first rule said, with only the device's own generation and its own cut time (after K): not stale. That was the hole.
    // A lineage cut down to the device's own entry shares nothing with the server's and names no fork (round 2 of the review): cut 0,
    // everything missed, the safe side.
    let own_only = json!([lineage[2].clone()]);
    assert_eq!(
        ask(own_only).await["stale"],
        true,
        "a lineage cut down to the device's own entry cannot be placed"
    );
    // The fork is found only past the first shared generation: the same lineage without its shared head is not placed late either.
    let without_head = json!([lineage[1].clone(), lineage[2].clone()]);
    assert_eq!(ask(without_head).await["stale"], true);
}

#[tokio::test]
async fn the_lineage_is_capped_and_a_device_that_fell_off_it_is_told_it_missed_everything() {
    // Every restore appends a generation; the server keeps the newest 64 (`rules::pull::MAX_LINEAGE`, ADR-0001 §6.3) and a device sends
    // its lineage with every pull. A device whose lineage shares nothing with the server's any more has missed every drop.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    // A device that got the first generation, with the lineage it was given, and a server that has seen 70 restores since (the rows a
    // backup of such a server holds; making 70 real restores would take a minute for the same state).
    let first = generation(&rig).await;
    // Its cut time is not zero and in the future of every drop: taken as the fork it would say "nothing missed". A lineage that shares
    // nothing with the server's names no fork at all (round 2 of the review), whatever its first entry's cut time is.
    let lineage = json!([{"generation": first.to_string(), "cut_ms": 4_000_000_000_000u64}]);
    let client = rig.state.pool.get().await.expect("connection");
    client
        .batch_execute(
            "INSERT INTO lineage (pos, generation, cut_ms) SELECT n, 1000000 + n, n FROM generate_series(2, 71) AS n",
        )
        .await
        .expect("seed the lineage");
    drop(client);
    let (dir, _) = take(&rig).await;

    restore_into(&rig, &dir).await;
    let kept = rows(&rig, "SELECT generation FROM lineage ORDER BY pos").await;
    assert_eq!(
        kept.len(),
        64,
        "the newest 64 generations of the 72: {}",
        kept.len()
    );
    assert!(
        !kept.iter().any(|g| g.contains(&first.to_string())),
        "and the first one has fallen off"
    );
    let session = Acct {
        token: login_token(&rig, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (status, answer) = rig
        .http
        .post(
            "/v1/pull",
            Some(&session.token),
            &json!({"generation": first.to_string(), "after": 1, "known_epoch": 0, "lineage": lineage}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["result"], "generation_changed");
    assert_eq!(answer["stale"], true, "{answer}");
    assert_eq!(answer["lineage"].as_array().map(Vec::len), Some(64));
}
