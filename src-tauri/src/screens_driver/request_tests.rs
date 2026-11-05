//! Tests of `request.rs`: the shapes, the limits, the id rule and the reply files (KINIG-92).

use super::*;

fn parse(text: &str) -> Result<Request, Reject> {
    parse_request("r1", text.as_bytes())
}

#[test]
fn a_press_parses_with_and_without_the_optional_fields() {
    let request =
        parse(r#"{"id":"r1","window":"main","step":{"kind":"press","name":"Play"}}"#).unwrap();
    assert_eq!(request.window, Window::Main);
    assert_eq!(
        request.step,
        Step::Press {
            name: "Play".into(),
            role: None,
            nth: None
        }
    );
    let request = parse(
        r#"{"id":"r1","window":"settings","step":{"kind":"press","name":"x","role":"button","nth":2}}"#,
    )
    .unwrap();
    assert_eq!(request.window.label(), "settings");
    assert!(matches!(request.step, Step::Press { nth: Some(2), .. }));
}

#[test]
fn every_step_kind_parses() {
    for step in [
        r#"{"kind":"press","name":"a"}"#,
        r#"{"kind":"menu","name":"a"}"#,
        r#"{"kind":"type","text":"abc"}"#,
        r#"{"kind":"type","name":"Search","text":"abc"}"#,
        r#"{"kind":"key","key":"k","code":"KeyK","mods":["meta","shift"]}"#,
        r#"{"kind":"wait","name":"a","gone":true,"timeoutMs":5000}"#,
        r#"{"kind":"has","name":"a"}"#,
        r#"{"kind":"scroll","name":"a"}"#,
        r#"{"kind":"text"}"#,
        r#"{"kind":"front"}"#,
    ] {
        let text = format!(r#"{{"id":"r1","window":"capture","step":{step}}}"#);
        assert!(parse(&text).is_ok(), "{step}");
    }
}

#[test]
fn the_step_is_forwarded_in_the_shape_the_page_reads() {
    let request =
        parse(r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a","timeoutMs":900}}"#)
            .unwrap();
    assert_eq!(
        serde_json::to_value(&request.step).unwrap(),
        serde_json::json!({"kind": "wait", "name": "a", "timeoutMs": 900})
    );
}

#[test]
fn garbage_is_refused_with_a_category_and_never_panics() {
    let cases: &[(&str, Reject)] = &[
        ("", Reject::BadJson),
        ("not json", Reject::BadJson),
        ("[]", Reject::BadJson),
        ("null", Reject::BadJson),
        ("\"r1\"", Reject::BadJson),
        ("{", Reject::BadJson),
        ("{}", Reject::IdMismatch),
        (
            r#"{"id":"other","window":"main","step":{"kind":"text"}}"#,
            Reject::IdMismatch,
        ),
        (
            r#"{"id":7,"window":"main","step":{"kind":"text"}}"#,
            Reject::IdMismatch,
        ),
        (
            r#"{"id":"r1","step":{"kind":"text"}}"#,
            Reject::UnknownWindow,
        ),
        (
            r#"{"id":"r1","window":"Main","step":{"kind":"text"}}"#,
            Reject::UnknownWindow,
        ),
        (
            r#"{"id":"r1","window":"debug","step":{"kind":"text"}}"#,
            Reject::UnknownWindow,
        ),
        (r#"{"id":"r1","window":"main"}"#, Reject::UnknownStep),
        (
            r#"{"id":"r1","window":"main","step":"text"}"#,
            Reject::UnknownStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"eval","code":"1"}}"#,
            Reject::UnknownStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"invoke","cmd":"x"}}"#,
            Reject::UnknownStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":7}}"#,
            Reject::UnknownStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press"}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":5}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":""}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","extra":1}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","nth":-1}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","nth":99999}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","role":"Button"}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a"}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a","timeoutMs":999999999}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"key","key":""}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"key","key":"k","mods":["super"]}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"type"}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"text","name":"x"}}"#,
            Reject::InvalidStep,
        ),
        (
            r#"{"id":"r1","window":"main","step":{"kind":"text"},"extra":1}"#,
            Reject::InvalidStep,
        ),
    ];
    for (text, want) in cases {
        assert_eq!(parse(text), Err(*want), "{text}");
    }
    // Invalid UTF-8 and a NUL.
    assert_eq!(
        parse_request("r1", &[0xff, 0xfe, 0x00]),
        Err(Reject::BadJson)
    );
    assert_eq!(parse_request("r1", b"\0\0\0"), Err(Reject::BadJson));
}

#[test]
fn too_long_names_and_texts_are_refused() {
    let long = "я".repeat(MAX_NAME_CHARS + 1);
    let text =
        format!(r#"{{"id":"r1","window":"main","step":{{"kind":"press","name":"{long}"}}}}"#);
    assert_eq!(parse(&text), Err(Reject::InvalidStep));
    let long = "x".repeat(MAX_TYPED_CHARS + 1);
    let text = format!(r#"{{"id":"r1","window":"main","step":{{"kind":"type","text":"{long}"}}}}"#);
    assert_eq!(parse(&text), Err(Reject::InvalidStep));
}

#[test]
fn an_id_that_is_a_path_is_not_an_id() {
    for bad in [
        "",
        ".",
        "..",
        "../x",
        "a/b",
        "a\\b",
        "a.b",
        "/etc/passwd",
        "a b",
        "a\0b",
        "é",
        "..\\..\\x",
        "x/..",
        "%2e%2e",
    ] {
        assert!(!valid_id(bad), "{bad:?}");
    }
    assert!(!valid_id(&"a".repeat(65)));
    for good in ["a", "r-1", "1730-42_7", "ABC", &"a".repeat(64)] {
        assert!(valid_id(good), "{good}");
    }
    assert_eq!(id_of_file_name("r1.json"), Some("r1"));
    for bad in [
        "r1.json.tmp",
        "..json",
        ".json",
        "r1.txt",
        "a b.json",
        "../r1.json",
        "r1",
        ".r1.json",
    ] {
        assert_eq!(id_of_file_name(bad), None, "{bad}");
    }
}

#[test]
fn a_reply_is_written_atomically_inside_the_replies_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_reply(dir.path(), &Reply::ok("r1", Some("yes".into()))).unwrap();
    assert_eq!(path, dir.path().join("replies").join("r1.json"));
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text, r#"{"id":"r1","ok":true,"value":"yes"}"#);
    let text =
        fs::read_to_string(write_reply(dir.path(), &Reply::error("r2", "not found")).unwrap())
            .unwrap();
    assert_eq!(text, r#"{"id":"r2","ok":false,"error":"not found"}"#);
    let names: Vec<_> = fs::read_dir(dir.path().join("replies"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 2, "no temporary file is left: {names:?}");
}

#[test]
fn a_reply_with_a_bad_id_writes_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("driver");
    fs::create_dir(&dir).unwrap();
    for bad in ["../escape", "a/b", "", "..", "x\0y"] {
        assert!(write_reply(&dir, &Reply::ok(bad, None)).is_err(), "{bad:?}");
    }
    assert!(!parent.path().join("escape.json").exists());
    assert!(!dir.join("replies").exists());
}

#[cfg(unix)]
#[test]
fn a_reply_does_not_follow_a_link_at_replies_or_at_the_temporary_name() {
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("victim");
    fs::write(&victim, b"keep").unwrap();

    // `replies` itself is a link to a directory elsewhere: nothing is written there.
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("replies")).unwrap();
    assert!(write_reply(dir.path(), &Reply::ok("r1", None)).is_err());
    assert!(!outside.path().join("r1.json").exists());

    // The temporary name is a link to a file: the file is not written through it.
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("replies")).unwrap();
    std::os::unix::fs::symlink(&victim, dir.path().join("replies").join(".r2.tmp")).unwrap();
    let path = write_reply(dir.path(), &Reply::ok("r2", Some("v".into()))).unwrap();
    assert_eq!(
        fs::read(&victim).unwrap(),
        b"keep",
        "the link's target is untouched"
    );
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        r#"{"id":"r2","ok":true,"value":"v"}"#
    );
}

#[test]
fn request_files_are_read_with_a_size_limit_and_only_if_regular() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("a.json");
    fs::write(&small, b"{}").unwrap();
    assert_eq!(read_request_file(&small).unwrap(), b"{}");
    let big = dir.path().join("b.json");
    fs::write(&big, vec![b' '; MAX_REQUEST_BYTES as usize + 1]).unwrap();
    assert_eq!(read_request_file(&big), Err(Reject::TooLarge));
    let exact = dir.path().join("c.json");
    fs::write(&exact, vec![b' '; MAX_REQUEST_BYTES as usize]).unwrap();
    assert!(read_request_file(&exact).is_ok());
    assert_eq!(read_request_file(dir.path()), Err(Reject::NotAFile));
    assert_eq!(
        read_request_file(&dir.path().join("none.json")),
        Err(Reject::Unreadable)
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("secret");
    fs::write(&secret, b"{}").unwrap();
    let link = dir.path().join("l.json");
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    assert_eq!(read_request_file(&link), Err(Reject::NotAFile));
}
