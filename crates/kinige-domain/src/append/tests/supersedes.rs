//! Предел `supersedes` в группе `body` (64, старейшие отбрасываются) и строгий разбор меток HLC (ADR-0001 §3.4).

use super::*;

#[test]
fn supersedes_keeps_the_newest_64() {
    let row = message("Идея");
    let mut groups_raw = row.groups_raw.clone();
    let mut hlc_json = row.hlc_json.clone();
    let mut all = vec![hlc(1_760_000_000_000)];
    for step in 1..=70u64 {
        let new_hlc = hlc(1_760_000_000_000 + step);
        let out = rewrite_body(
            &CurrentBody {
                groups_raw: &groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false,
            },
            "текст",
            NOW,
            &new_hlc,
        )
        .unwrap();
        groups_raw = out.groups_raw;
        hlc_json = out.hlc_json;
        all.push(new_hlc);
        let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
        assert!(list.len() <= MAX_SUPERSEDES, "шаг {step}");
        // Всегда последние версии, кроме текущей.
        let expected: Vec<&String> = all[..all.len() - 1]
            .iter()
            .rev()
            .take(MAX_SUPERSEDES)
            .rev()
            .collect();
        assert_eq!(list.iter().collect::<Vec<_>>(), expected, "шаг {step}");
    }
    let list: Vec<String> = serde_json::from_str(
        &serde_json::from_str::<Value>(&body_group(&groups_raw)).unwrap()["supersedes"].to_string(),
    )
    .unwrap();
    assert_eq!(list.len(), MAX_SUPERSEDES);
    assert_eq!(list.last(), Some(&hlc(1_760_000_000_000 + 69)));
}

/// Запись, у которой в `supersedes` уже `count` меток (старейшая — `l = 1_000 + 0`), текущая версия — `l = 1_000 + count`.
fn with_supersedes(count: u64) -> (String, String, Vec<String>) {
    let row = message("Идея");
    let old: Vec<String> = (0..count).map(|i| hlc(1_000 + i)).collect();
    let mut groups = parse_groups_raw(&row.groups_raw).unwrap();
    groups.insert(
        "body".into(),
        RawValue::from_string(
            String::from_utf8(
                jcs::canonical_bytes(&json!({
                    "base": null, "body_md": "Идея", "edited_at": null, "supersedes": old
                }))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&row.hlc_json).unwrap();
    hlcs.insert(
        "body".into(),
        RawValue::from_string(json!({"h": hlc(1_000 + count), "sv": 1}).to_string()).unwrap(),
    );
    (
        serde_json::to_string(&groups).unwrap(),
        serde_json::to_string(&hlcs).unwrap(),
        old,
    )
}

#[test]
fn supersedes_is_exactly_64_at_the_boundary_and_drops_only_the_oldest() {
    // 62 прежних + текущая версия → после правки 63 метки: ничего не отброшено.
    // 63 прежних + текущая → ровно 64: ничего не отброшено.
    // 64 прежних + текущая → было бы 65: отбрасывается ровно одна, самая старая.
    for (count, dropped) in [(62u64, 0usize), (63, 0), (64, 1), (70, 7)] {
        let (groups_raw, hlc_json, old) = with_supersedes(count);
        let out = rewrite_body(
            &CurrentBody {
                groups_raw: &groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false,
            },
            "x",
            NOW,
            &hlc(1_000_000),
        )
        .unwrap();
        let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
        let mut expected: Vec<String> = old.clone();
        expected.push(hlc(1_000 + count));
        let expected: Vec<String> = expected.split_off(dropped);
        assert_eq!(
            list.len(),
            expected.len().min(MAX_SUPERSEDES),
            "count={count}"
        );
        assert_eq!(list, expected, "count={count}");
        assert!(list.len() <= 64);
        let group: Value = serde_json::from_str(&body_group(&out.groups_raw)).unwrap();
        assert_eq!(
            group["supersedes"],
            json!(list),
            "группа и колонка совпадают"
        );
        assert_eq!(
            list.last(),
            Some(&hlc(1_000 + count)),
            "прежняя версия в списке"
        );
        if dropped > 0 {
            assert!(!list.contains(&old[0]), "старейшая отброшена");
        }
    }
    let (groups_raw, hlc_json, _) = with_supersedes(63);
    let out = rewrite_body(
        &CurrentBody {
            groups_raw: &groups_raw,
            hlc_json: &hlc_json,
            body_confirmed: false,
        },
        "x",
        NOW,
        &hlc(1_000_000),
    )
    .unwrap();
    let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
    assert_eq!(list.len(), 64, "ровно 64, не 63 и не 65");
}

#[test]
fn malformed_hlc_labels_are_corrupt_not_ordered_by_string_comparison() {
    let row = message("Идея");
    // Новая метка повреждена, но «больше» как строка.
    for bad in ["zzz", "9999999999999-00000-not-a-uuid", ""] {
        assert!(
            matches!(
                rewrite_body(&current(&row, false), "x", NOW, bad),
                Err(GroupError::Corrupt(_))
            ),
            "{bad:?}"
        );
    }
    // Прежняя метка повреждена: ошибка, а не HlcNotNewer и не успех.
    let hlc_json = row.hlc_json.replace(
        &format!(r#""body":{{"h":"{}","sv":1}}"#, hlc(1_760_000_000_000)),
        r#""body":{"h":"0","sv":1}"#,
    );
    assert_ne!(hlc_json, row.hlc_json);
    assert!(matches!(
        rewrite_body(
            &CurrentBody {
                groups_raw: &row.groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false
            },
            "x",
            NOW,
            &hlc(1_760_000_100_000)
        ),
        Err(GroupError::Corrupt(_))
    ));
}
