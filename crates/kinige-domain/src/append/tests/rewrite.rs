//! Перезапись группы `body`: `base`/`supersedes`, лимит 64, дословность чужого, ошибки (ADR-0001 §3.3, §3.4, §4).

use super::*;

#[test]
fn unconfirmed_version_gets_base_from_itself_and_supersedes_the_old_hlc() {
    let row = message("Идея");
    let new_hlc = hlc(1_760_000_100_000);
    let out = rewrite_body(
        &current(&row, false),
        "Идея\n\nи ещё",
        NOW + 100_000,
        &new_hlc,
    )
    .unwrap();
    assert_eq!(
        body_group(&out.groups_raw),
        format!(
            r#"{{"base":null,"body_md":"Идея\n\nи ещё","edited_at":1760000100000,"supersedes":["{}"]}}"#,
            hlc(1_760_000_000_000)
        )
    );
    assert_eq!(out.body_base, None);
    assert_eq!(
        out.body_supersedes,
        format!(r#"["{}"]"#, hlc(1_760_000_000_000))
    );
    assert_eq!(out.body_md, "Идея\n\nи ещё");
    assert_eq!(out.edited_at, NOW + 100_000);
    assert_eq!(out.schema_version, 1);
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(&out.hlc_json).unwrap();
    assert_eq!(hlcs["body"], json!({"h": new_hlc, "sv": 1}));
    for other in ["core", "flags", "placement", "deleted"] {
        assert_eq!(
            hlcs[other],
            json!({"h": hlc(1_760_000_000_000), "sv": 1}),
            "{other} не тронута"
        );
    }
}

#[test]
fn second_rewrite_of_an_unconfirmed_version_accumulates_supersedes() {
    let row = message("Идея");
    let first = rewrite_body(
        &current(&row, false),
        "a\n\nb",
        NOW + 1,
        &hlc(1_760_000_100_000),
    )
    .unwrap();
    let second = rewrite_body(
        &CurrentBody {
            groups_raw: &first.groups_raw,
            hlc_json: &first.hlc_json,
            body_confirmed: false,
        },
        "a\n\nb\n\nc",
        NOW + 2,
        &hlc(1_760_000_200_000),
    )
    .unwrap();
    assert_eq!(
        second.body_supersedes,
        format!(
            r#"["{}","{}"]"#,
            hlc(1_760_000_000_000),
            hlc(1_760_000_100_000)
        )
    );
    assert_eq!(second.body_base, None, "base первой версии остаётся");
}

#[test]
fn confirmed_version_becomes_base_and_supersedes_is_empty() {
    let row = message("Идея");
    let out = rewrite_body(
        &current(&row, true),
        "Идея\n\nи ещё",
        NOW + 5,
        &hlc(1_760_000_100_000),
    )
    .unwrap();
    assert_eq!(
        body_group(&out.groups_raw),
        format!(
            r#"{{"base":"{}","body_md":"Идея\n\nи ещё","edited_at":1760000000005,"supersedes":[]}}"#,
            hlc(1_760_000_000_000)
        )
    );
    assert_eq!(out.body_base, Some(hlc(1_760_000_000_000)));
    assert_eq!(out.body_supersedes, "[]");
}

#[test]
fn unknown_groups_unknown_body_fields_and_extra_hlc_fields_survive_byte_for_byte() {
    let row = message("Идея");
    // Группа новой версии с пробелами в байтах и своим sv, лишнее поле в записи HLC чужой группы, неизвестное поле в body.
    let future = r#"{ "z": [1, 2],  "a": "ё" }"#;
    let mut groups = parse_groups_raw(&row.groups_raw).unwrap();
    groups.insert(
        "future".into(),
        RawValue::from_string(future.into()).unwrap(),
    );
    groups.insert(
        "body".into(),
        RawValue::from_string(
            r#"{"base":null,"body_md":"Идея","edited_at":null,"flair":"x","supersedes":[]}"#.into(),
        )
        .unwrap(),
    );
    let groups_raw = serde_json::to_string(&groups).unwrap();
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&row.hlc_json).unwrap();
    hlcs.insert(
        "future".into(),
        RawValue::from_string(format!(
            r#"{{ "h":"{}", "sv":2, "x":true }}"#,
            hlc(1_760_000_000_001)
        ))
        .unwrap(),
    );
    hlcs.insert(
        "flags".into(),
        RawValue::from_string(format!(
            r#"{{"h":"{}","sv":1,"note":"keep"}}"#,
            hlc(1_760_000_000_000)
        ))
        .unwrap(),
    );
    let hlc_json = serde_json::to_string(&hlcs).unwrap();

    let out = rewrite_body(
        &CurrentBody {
            groups_raw: &groups_raw,
            hlc_json: &hlc_json,
            body_confirmed: false,
        },
        "Идея\n\nи ещё",
        NOW,
        &hlc(1_760_000_100_000),
    )
    .unwrap();

    let after = parse_groups_raw(&out.groups_raw).unwrap();
    assert_eq!(after["future"].get(), future, "неизвестная группа дословно");
    for name in ["core", "flags", "placement", "deleted"] {
        assert_eq!(after[name].get(), groups[name].get(), "{name} дословно");
    }
    let body: Value = serde_json::from_str(after["body"].get()).unwrap();
    assert_eq!(body["flair"], "x", "неизвестное поле body сохранено");
    assert_eq!(
        after["body"].get(),
        String::from_utf8(jcs::canonical_bytes(&body).unwrap()).unwrap(),
        "перезаписанная группа каноничная"
    );
    let hlc_after: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&out.hlc_json).unwrap();
    assert_eq!(hlc_after["future"].get(), hlcs["future"].get());
    assert_eq!(hlc_after["flags"].get(), hlcs["flags"].get());
    assert_eq!(out.schema_version, 2, "максимум по группам");
}

#[test]
fn corrupt_input_is_an_error_and_nothing_is_returned() {
    let row = message("Идея");
    let newer = hlc(1_760_000_100_000);
    let ok = |groups_raw: &str, hlc_json: &str| {
        rewrite_body(
            &CurrentBody {
                groups_raw,
                hlc_json,
                body_confirmed: false,
            },
            "x",
            NOW,
            &newer,
        )
    };
    assert!(ok(&row.groups_raw, &row.hlc_json).is_ok());
    for groups_raw in ["", "{", "[]", "{}", r#"{"body":5}"#, r#"{"body":[]}"#] {
        assert!(
            matches!(ok(groups_raw, &row.hlc_json), Err(GroupError::Corrupt(_))),
            "{groups_raw:?}"
        );
    }
    for hlc_json in [
        "",
        "{",
        "{}",
        r#"{"body":{"h":"a"}}"#,
        r#"{"body":{"sv":1}}"#,
    ] {
        assert!(
            matches!(ok(&row.groups_raw, hlc_json), Err(GroupError::Corrupt(_))),
            "{hlc_json:?}"
        );
    }
    let bad_base = row.groups_raw.replace(r#""base":null"#, r#""base":5"#);
    assert_ne!(bad_base, row.groups_raw);
    assert!(matches!(
        ok(&bad_base, &row.hlc_json),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn body_group_of_a_newer_version_is_not_rewritten() {
    let row = message("Идея");
    let hlc_json = row.hlc_json.replace(
        &format!(r#""body":{{"h":"{}","sv":1}}"#, hlc(1_760_000_000_000)),
        &format!(r#""body":{{"h":"{}","sv":2}}"#, hlc(1_760_000_000_000)),
    );
    assert_ne!(hlc_json, row.hlc_json);
    assert_eq!(
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
        Err(GroupError::UnsupportedVersion)
    );
}

#[test]
fn new_hlc_must_be_strictly_greater() {
    let row = message("Идея");
    for bad in [hlc(1_760_000_000_000), hlc(1_759_999_999_999)] {
        assert_eq!(
            rewrite_body(&current(&row, false), "x", NOW, &bad),
            Err(GroupError::HlcNotNewer),
            "{bad}"
        );
    }
    // Тот же момент, но счётчик больше.
    let next = format!("{:013}-00001-{DEVICE}", 1_760_000_000_000u64);
    assert!(rewrite_body(&current(&row, false), "x", NOW, &next).is_ok());
}

#[test]
fn group_limit_counts_canonical_bytes_after_joining() {
    let row = message("Идея");
    let newer = hlc(1_760_000_100_000);
    // Кавычки и переводы строк удваиваются в JCS.
    let escaped = "\"\n".repeat(256 * 1024 - 8);
    assert_eq!(
        rewrite_body(&current(&row, false), &escaped, NOW, &newer),
        Err(GroupError::TooLarge)
    );
    let plain = "я".repeat(200 * 1024);
    assert!(rewrite_body(&current(&row, false), &plain, NOW, &newer).is_ok());
    let too_long = "я".repeat(300 * 1024);
    assert_eq!(
        rewrite_body(&current(&row, false), &too_long, NOW, &newer),
        Err(GroupError::TooLarge)
    );
}
