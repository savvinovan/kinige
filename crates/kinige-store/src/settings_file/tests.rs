//! Разбор файла настроек: по каждому ключу — неверный тип, недопустимое значение, `null`, отсутствие, неизвестный ключ;
//! «охотник за мусором»: случайные байты, не-UTF-8, BOM, глубокая вложенность, огромные строки и числа. Только ошибки, без паник.

use super::parse::{Field, FileError, MAX_BYTES, parse};
use kinige_domain::{AgentMode, KeyIssueKind, TextSize};

fn kinds(text: &str) -> Vec<(String, KeyIssueKind)> {
    let parsed = parse(text.as_bytes()).expect("файл разобран");
    parsed.issues.into_iter().map(|i| (i.key, i.kind)).collect()
}

#[test]
fn every_key_accepts_its_values_null_and_absence() {
    let parsed = parse(
        br#"{"text_size":"large","shortcuts":{"capture":"Alt+Ctrl+KeyJ","clipboard":null},
            "mirror":{"folder":"/tmp"},"agent_access":{"enabled":true,"mode":"read_write","chats":[{"id":"a"},{"title":"b"}]}}"#,
    )
    .unwrap();
    assert_eq!(parsed.text_size, Field::Value(TextSize::Large));
    assert_eq!(
        parsed.shortcuts[0],
        Field::Value("Ctrl+Alt+KeyJ".to_string())
    );
    assert_eq!(parsed.shortcuts[1], Field::Default);
    assert_eq!(parsed.mirror_folder, Field::Value("/tmp".to_string()));
    assert_eq!(parsed.agent_enabled, Field::Value(true));
    assert_eq!(parsed.agent_mode, Field::Value(AgentMode::ReadWrite));
    assert!(matches!(&parsed.chats, Field::Value(list) if list.len() == 2));
    assert!(parsed.issues.is_empty() && parsed.unknown.is_empty());

    let empty = parse(b"{}").unwrap();
    assert_eq!(empty.text_size, Field::Absent);
    assert_eq!(empty.chats, Field::Absent);
    let nulls = parse(
        br#"{"text_size":null,"mirror":{"folder":null},"agent_access":{"enabled":null,"mode":null,"chats":null}}"#,
    )
    .unwrap();
    assert_eq!(nulls.text_size, Field::Default);
    assert_eq!(nulls.mirror_folder, Field::Default);
    assert_eq!(nulls.agent_enabled, Field::Default);
    assert_eq!(nulls.agent_mode, Field::Default);
    assert_eq!(nulls.chats, Field::Default);
}

#[test]
fn a_wrong_type_or_value_is_an_error_of_that_key() {
    use KeyIssueKind::*;
    for (text, key, kind) in [
        (r#"{"text_size":1}"#, "text_size", Type),
        (r#"{"text_size":"huge"}"#, "text_size", Value),
        (r#"{"text_size":["large"]}"#, "text_size", Type),
        (r#"{"shortcuts":"Ctrl+Alt+KeyN"}"#, "shortcuts", Type),
        (r#"{"shortcuts":null}"#, "shortcuts", Type),
        (r#"{"shortcuts":{"capture":5}}"#, "shortcuts.capture", Type),
        (
            r#"{"shortcuts":{"capture":""}}"#,
            "shortcuts.capture",
            ShortcutInvalid,
        ),
        (
            r#"{"shortcuts":{"clipboard":"KeyV"}}"#,
            "shortcuts.clipboard",
            ShortcutInvalid,
        ),
        (
            r#"{"shortcuts":{"clipboard":"Cmd+KeyQ"}}"#,
            "shortcuts.clipboard",
            ShortcutInvalid,
        ),
        (r#"{"mirror":[]}"#, "mirror", Type),
        (r#"{"mirror":{"folder":true}}"#, "mirror.folder", Type),
        (r#"{"agent_access":5}"#, "agent_access", Type),
        (
            r#"{"agent_access":{"enabled":"true"}}"#,
            "agent_access.enabled",
            Type,
        ),
        (
            r#"{"agent_access":{"enabled":1}}"#,
            "agent_access.enabled",
            Type,
        ),
        (
            r#"{"agent_access":{"mode":"write"}}"#,
            "agent_access.mode",
            Value,
        ),
        (
            r#"{"agent_access":{"mode":true}}"#,
            "agent_access.mode",
            Type,
        ),
        (
            r#"{"agent_access":{"chats":"all"}}"#,
            "agent_access.chats",
            Type,
        ),
        (
            r#"{"agent_access":{"chats":[1]}}"#,
            "agent_access.chats[0]",
            Type,
        ),
        (
            r#"{"agent_access":{"chats":[{"id":1}]}}"#,
            "agent_access.chats[0]",
            Type,
        ),
        (
            r#"{"agent_access":{"chats":[{"title":[]}]}}"#,
            "agent_access.chats[0]",
            Type,
        ),
        (
            r#"{"agent_access":{"chats":[{}]}}"#,
            "agent_access.chats[0]",
            Value,
        ),
        (
            r#"{"agent_access":{"chats":[{"id":null,"title":null}]}}"#,
            "agent_access.chats[0]",
            Value,
        ),
    ] {
        assert_eq!(kinds(text), vec![(key.to_string(), kind)], "{text}");
    }
}

#[test]
fn unknown_keys_are_warnings_and_stay_in_the_document() {
    let parsed = parse(br#"{"zzz":1,"shortcuts":{"capture":"Ctrl+Alt+KeyN","q":1},"mirror":{"x":1},"agent_access":{"y":2}}"#).unwrap();
    let mut unknown = parsed.unknown.clone();
    unknown.sort();
    assert_eq!(
        unknown,
        ["agent_access.y", "mirror.x", "shortcuts.q", "zzz"]
    );
    assert!(parsed.raw.contains_key("zzz"));
    assert!(parsed.issues.is_empty());
    // Имя неизвестного ключа в окне не длиннее 64 знаков.
    let long = "я".repeat(5000);
    let parsed = parse(format!("{{\"{long}\":1}}").as_bytes()).unwrap();
    assert_eq!(parsed.unknown[0].chars().count(), 65);
}

#[test]
fn the_error_list_is_capped_so_junk_cannot_bloat_the_window() {
    let items = vec!["1"; 5000].join(",");
    let parsed = parse(format!(r#"{{"agent_access":{{"chats":[{items}]}}}}"#).as_bytes()).unwrap();
    assert_eq!(parsed.issues.len(), 100);
}

#[test]
fn files_that_are_not_a_json_object_are_file_errors() {
    assert_eq!(
        parse(b"").unwrap_err(),
        FileError::Parse { line: 1, column: 0 }
    );
    assert!(matches!(parse(b"{").unwrap_err(), FileError::Parse { .. }));
    assert!(matches!(
        parse(b"{} {}").unwrap_err(),
        FileError::Parse { .. }
    ));
    assert!(matches!(
        parse(b"{\"a\":1,}").unwrap_err(),
        FileError::Parse { .. }
    ));
    assert!(matches!(
        parse(b"{/*c*/}").unwrap_err(),
        FileError::Parse { .. }
    ));
    assert_eq!(parse(b"[]").unwrap_err(), FileError::NotObject);
    assert_eq!(parse(b"null").unwrap_err(), FileError::NotObject);
    assert_eq!(parse(b"\"text\"").unwrap_err(), FileError::NotObject);
    assert_eq!(parse(b"42").unwrap_err(), FileError::NotObject);
    assert_eq!(parse(b"\xEF\xBB\xBF{}").unwrap_err(), FileError::Bom);
    assert_eq!(parse(b"{\"a\":\"\xFF\"}").unwrap_err(), FileError::NotUtf8);
    assert_eq!(
        parse(&vec![b' '; MAX_BYTES as usize + 1]).unwrap_err(),
        FileError::TooLarge
    );
    let error = parse(br#"{"text_size": "#).unwrap_err();
    assert_eq!(
        error,
        FileError::Parse {
            line: 1,
            column: 14
        }
    );
    assert_eq!(error.category(), "parse");
    assert_eq!(FileError::Io(std::io::ErrorKind::Other).category(), "io");
}

#[test]
fn deep_nesting_huge_strings_and_numbers_never_panic() {
    let deep_array = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(parse(deep_array.as_bytes()).is_err());
    let deep_object = format!("{}1{}", "{\"a\":".repeat(50_000), "}".repeat(50_000));
    assert!(parse(deep_object.as_bytes()).is_err());
    let long_string = format!(
        "{{\"text_size\":\"{}\"}}",
        "x".repeat(MAX_BYTES as usize - 40)
    );
    let parsed = parse(long_string.as_bytes()).unwrap();
    assert_eq!(parsed.issues.len(), 1);
    for number in [
        "1e999999",
        "-1e999999",
        &"9".repeat(100_000),
        "0.0000000000000000001",
    ] {
        let text = format!("{{\"text_size\":{number},\"agent_access\":{{\"enabled\":{number}}}}}");
        match parse(text.as_bytes()) {
            Ok(parsed) => assert_eq!(parsed.issues.len(), 2),
            Err(error) => assert_eq!(error.category(), "parse"),
        }
    }
}

#[test]
fn random_bytes_and_mutated_documents_never_panic() {
    let mut seed: u64 = 0x1234_5678_9ABC_DEF1;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..3000 {
        let len = (next() % 400) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        let _ = parse(&bytes);
    }
    let base = br#"{"text_size":"large","shortcuts":{"capture":"Ctrl+Alt+KeyN"},"mirror":{"folder":null},"agent_access":{"enabled":true,"mode":"read","chats":[{"id":"x","title":"y"}]}}"#;
    for _ in 0..3000 {
        let mut doc = base.to_vec();
        for _ in 0..(next() % 4 + 1) {
            let at = (next() as usize) % doc.len();
            match next() % 3 {
                0 => doc[at] = next() as u8,
                1 => {
                    doc.remove(at);
                }
                _ => doc.insert(at, b"{}[]\",:0e-"[(next() % 10) as usize]),
            }
        }
        let _ = parse(&doc);
    }
}
