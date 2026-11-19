//! Slice 7b: the changes a details command carries. An absent key leaves a field, `null` clears a name or a description, and every
//! other shape is refused with an error that never quotes the value.

use super::AttachmentDetailsChanges;
use serde_json::{Value, json};

fn parse(value: Value) -> Result<AttachmentDetailsChanges, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn an_absent_key_leaves_the_field_and_null_clears_it() {
    assert_eq!(
        parse(json!({})).unwrap(),
        AttachmentDetailsChanges::default()
    );
    let sent = parse(json!({"name": null, "description": "x", "spoiler": false})).unwrap();
    assert_eq!(sent.name, Some(None), "null clears");
    assert_eq!(sent.description, Some(Some("x".to_string())));
    assert_eq!(sent.spoiler, Some(false));
}

#[test]
fn garbage_is_refused_and_the_error_never_quotes_the_value() {
    let secret = "SECRETMARK";
    for garbage in [
        json!({"spoiler": null}),
        json!({"spoiler": secret}),
        json!({"spoiler": 1}),
        json!({"name": 7}),
        json!({"name": [secret]}),
        json!({"description": {"a": secret}}),
        json!({"blobKey": secret}),
        json!({"filename": secret}),
        json!({"name": "x", secret: 1}),
        // serde would read a struct from an array: `["x"]` must not become a name.
        json!([secret]),
        json!([secret, "d", true]),
        json!(secret),
        json!(null),
        json!(7),
        json!(true),
    ] {
        let error = parse(garbage.clone()).expect_err(&garbage.to_string());
        assert!(
            !error.to_string().contains(secret),
            "the error quotes the value: {error}"
        );
    }
}
