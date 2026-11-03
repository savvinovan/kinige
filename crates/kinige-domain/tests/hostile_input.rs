//! Hostile strings at the input of the domain parsers (KINIG-28): multibyte characters at every position of the samples
//! (HLC stamp, dates, UUID, backup names, timestamps), unicode garbage, NUL, very long strings. No parser panics: the
//! answer is `Err`, `None` or an ordinary result. String slices at foreign offsets would fail here on a character boundary.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

use kinige_domain::Language;
use kinige_domain::backup::{classify, expired};
use kinige_domain::chat::{key_after, normalize_icon, normalize_title};
use kinige_domain::emoji::is_emoji_sequence;
use kinige_domain::hlc::{is_uuid, is_uuid_v4, parse};
use kinige_domain::journal::{date_parts, is_journal_date, is_valid_journal_date};
use kinige_domain::link::validate_external_url;
use kinige_domain::mirror::{
    MirrorEntry, MirrorReply, day_file_segments, relative_day_link, render_day_file,
};
use kinige_domain::on_this_day::{past_dates, year_of};
use kinige_domain::search::{fts_query, highlight_terms, index_text, snippet, title_matches};
use kinige_domain::shortcuts::canonical_accelerator;
use kinige_domain::tags::parse_tags;
use kinige_domain::tasks::{set_task_item, task_item_state, task_items};
use kinige_domain::telegram::{parse_export, render_body};
use serde_json::{Value, json};

const DEVICE: &str = "123e4567-e89b-42d3-a456-426614174000";
/// Characters of 2, 3 and 4 bytes in UTF-8, a combining character and a NUL character.
const UNITS: [&str; 7] = ["я", "€", "😀", "\u{301}", "\0", "\u{202e}", "\u{200d}"];

fn samples() -> Vec<String> {
    vec![
        format!("0000000000001-00001-{DEVICE}"),
        DEVICE.to_string(),
        "2026-03-14".to_string(),
        "2026-02-29".to_string(),
        "kinige-daily-2026-03-14.db".to_string(),
        "kinige-pre-import-20260314T101500Z.db".to_string(),
        "kinige-pre-migration-v3-20260314T101500Z.db".to_string(),
        "20260314T101500Z".to_string(),
        "Ctrl+Alt+KeyN".to_string(),
        "- [ ] задача #тег".to_string(),
        "# заголовок\n> цитата\n1. пункт\n```\nкод\n```\n[ссылка](https://example.org) <https://x.y>".to_string(),
        "a000001".to_string(),
        "📚".to_string(),
        "https://example.org/path".to_string(),
    ]
}

/// Samples with a multibyte character inserted at every position and substituted for every character.
fn mutations() -> Vec<String> {
    let mut out = samples();
    for sample in samples() {
        let chars: Vec<char> = sample.chars().collect();
        for at in 0..=chars.len() {
            for unit in UNITS {
                let head: String = chars.iter().take(at).collect();
                let tail: String = chars.iter().skip(at).collect();
                out.push(format!("{head}{unit}{tail}"));
                let tail_without: String = chars.iter().skip(at + 1).collect();
                out.push(format!("{head}{unit}{tail_without}"));
            }
        }
        // Truncations by bytes, like truncated paths and names.
        for cut in 0..sample.len() {
            if let Some(prefix) = sample.get(..cut) {
                out.push(prefix.to_string());
            }
        }
    }
    for unit in UNITS {
        for len in [
            0, 1, 2, 7, 8, 9, 10, 15, 16, 17, 35, 36, 37, 55, 56, 57, 200,
        ] {
            out.push(unit.repeat(len));
        }
    }
    out.push("x".repeat(60_000));
    out.push("я".repeat(30_000));
    out
}

#[test]
fn string_parsers_never_panic_on_hostile_text() {
    let corpus = mutations();
    assert!(corpus.len() > 5_000, "{}", corpus.len());
    for text in &corpus {
        let _ = parse(text);
        let _ = is_uuid(text);
        let _ = is_uuid_v4(text);
        let _ = date_parts(text);
        let _ = is_journal_date(text);
        let _ = is_valid_journal_date(text);
        let _ = year_of(text);
        let _ = past_dates(text, 1990);
        let _ = classify(text);
        let _ = expired(std::slice::from_ref(text));
        let _ = day_file_segments("Чат", text);
        let _ = day_file_segments(text, "2026-03-14");
        let _ = key_after(Some(text));
        let _ = normalize_title(text);
        let _ = normalize_icon(Some(text));
        let _ = is_emoji_sequence(text);
        let _ = validate_external_url(text);
        let _ = canonical_accelerator(text);
        let _ = fts_query(text);
        let _ = index_text(text);
        let _ = title_matches(text, text);
        let _ = parse_tags(text);
        let _ = task_items(text);
        for index in [0, 1, usize::MAX] {
            let _ = task_item_state(text, index);
            let _ = set_task_item(text, index, true);
        }
    }
}

#[test]
fn text_renderers_never_panic_on_hostile_markdown() {
    // Rendering and search over the entry body: also on a large input (a megabyte), but without quadratic passes over all samples.
    for text in mutations().iter().filter(|t| t.len() < 400) {
        let _ = highlight_terms(text, text);
        let _ = highlight_terms(text, "задача tag");
        for max in [0, 1, 2, 3, 4, 10, 80, usize::MAX] {
            let _ = snippet(text, &["задача".to_string(), text.clone()], max);
        }
        let entry = |body: &str, reply: Option<MirrorReply>| MirrorEntry {
            time: text.clone(),
            body_md: body.to_string(),
            reply,
        };
        let entries = [
            entry(text, None),
            entry(&format!("{text}\n{text}\n---\n{text}\n==="), None),
            entry(
                &format!("- [ ] {text}\n- [x] {text}\n> {text}\n# {text}"),
                None,
            ),
            entry(
                &format!("```{text}\n{text}\n```\n{text}"),
                Some(MirrorReply::Gone),
            ),
            entry(
                text,
                Some(MirrorReply::Original {
                    date: text.clone(),
                    time: text.clone(),
                    link: Some(text.clone()),
                }),
            ),
        ];
        let _ = render_day_file(text, &entries);
        let _ = relative_day_link(text, &[text.clone(), text.clone(), text.clone()]);
    }
    for big in [
        "я".repeat(100_000),
        "[".repeat(20_000),
        "`".repeat(20_000),
        "<".repeat(20_000),
        "#a ".repeat(20_000),
    ] {
        let _ = parse_tags(&big);
        let _ = snippet(&big, &["я".to_string()], 80);
    }
}

#[test]
fn the_telegram_reader_answers_garbage_with_an_error() {
    let deep_array = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    let deep_object = format!("{}1{}", "{\"a\":".repeat(10_000), "}".repeat(10_000));
    for text in [
        "",
        "null",
        "[]",
        "{}",
        "\u{0}",
        r#"{"name": 5, "messages": "x"}"#,
        r#"{"messages": [{"id": "я", "date": 5, "text": {}}]}"#,
        r#"{"messages": [null, 1, "a", []]}"#,
        &deep_array,
        &deep_object,
        &"я".repeat(100_000),
    ] {
        let _ = parse_export(text);
    }
}

#[test]
fn a_valid_telegram_export_with_hostile_texts_is_read_and_rendered_without_a_panic() {
    let hostile: Vec<String> = mutations().into_iter().filter(|t| t.len() < 300).collect();
    let mut rendered = 0usize;
    for chunk in hostile.chunks(40) {
        let messages: Vec<Value> = chunk
            .iter()
            .enumerate()
            .map(|(id, text)| {
                json!({
                    "id": id + 1,
                    "type": "message",
                    "date": "2024-03-05T09:41:07",
                    "date_unixtime": "1709632867",
                    "from": text,
                    "from_id": format!("user{id}"),
                    "forwarded_from": text,
                    "reply_to_message_id": text,
                    "text": text,
                    "text_entities": [
                        {"type": "plain", "text": text},
                        {"type": "bold", "text": text},
                        {"type": "code", "text": text},
                        {"type": "pre", "text": text, "language": text},
                        {"type": "text_link", "text": text, "href": text},
                        {"type": "link", "text": text},
                        {"type": "mention", "text": text},
                        {"type": "hashtag", "text": text},
                        {"type": "blockquote", "text": text},
                        {"type": "custom_emoji", "text": text},
                        {"type": text, "text": text},
                    ],
                })
            })
            .collect();
        let export =
            json!({"name": chunk.first(), "type": "saved_messages", "id": 1, "messages": messages});
        let parsed = parse_export(&export.to_string()).expect("a valid export is readable");
        for chat in &parsed.chats {
            for message in &chat.messages {
                for language in [Language::En, Language::Ru] {
                    if let Some(body) = render_body(message, message.from.as_deref(), language) {
                        let _ = parse_tags(&body);
                        let _ = snippet(&body, &[], 80);
                        rendered += 1;
                    }
                }
            }
        }
    }
    assert!(rendered > 1_000, "{rendered}");
}
