use super::*;

#[test]
fn a_pack_name_comes_from_a_link_or_bare() {
    for (input, name) in [
        ("https://t.me/addstickers/TestPack", "TestPack"),
        ("t.me/addstickers/Test_Pack_2", "Test_Pack_2"),
        ("  TestPack\n", "TestPack"),
        ("a", "a"),
    ] {
        assert_eq!(pack_name(input), Some(name), "{input}");
    }
    let long = "a".repeat(64);
    assert_eq!(pack_name(&long), Some(long.as_str()));
}

#[test]
fn what_is_not_a_pack_name_is_refused() {
    let too_long = "a".repeat(65);
    for input in [
        "",
        " ",
        "http://t.me/addstickers/TestPack",
        "https://t.me/addstickers/",
        "https://t.me/addstickers/Test/Pack",
        "https://t.me/addstickers/Test?x=1",
        "https://t.me/addstickers/Test#x",
        "https://t.me/addemoji/Test",
        "https://example.com/addstickers/Test",
        "https://t.me.evil.example/addstickers/Test",
        "Test Pack",
        "Test-Pack",
        "Тест",
        "../x",
        "Test%2e",
        "Test\u{0}",
        too_long.as_str(),
    ] {
        assert_eq!(pack_name(input), None, "{input:?}");
    }
}

#[test]
fn the_token_rule_is_ours_and_loose() {
    for ok in [
        "1:a",
        "123456:abc-DEF_ghi",
        &format!("{}:{}", "9".repeat(20), "x".repeat(100)),
    ] {
        assert!(is_token(ok), "{ok}");
    }
    for bad in [
        "",
        ":",
        "1:",
        ":a",
        "a:b",
        "1:a:b",
        "1 :a",
        "1:a b",
        "1:a\n",
        "1:é",
        &format!("{}:a", "9".repeat(21)),
        &format!("1:{}", "x".repeat(101)),
    ] {
        assert!(!is_token(bad), "{bad:?}");
    }
}

#[test]
fn a_file_id_goes_into_a_query_value_and_nothing_else_may() {
    assert!(is_file_id("AgADBAADq_-0"));
    assert!(is_file_id(&"a".repeat(256)));
    for bad in [
        "",
        "a&b",
        "a#b",
        "a%2e%2e",
        "a b",
        "a=b",
        "a/b",
        "a.b",
        "a+b",
        "é",
        &"a".repeat(257),
    ] {
        assert!(!is_file_id(bad), "{bad:?}");
    }
}

#[test]
fn a_file_path_stays_inside_the_path_of_the_url() {
    assert!(is_file_path("stickers/file_12.webp"));
    assert!(is_file_path("a/b/c.png"));
    for bad in [
        "",
        "/stickers/a.webp",
        "../a.webp",
        "a/../b.webp",
        "a/./..",
        "a//b",
        "a/",
        "a?b",
        "a#b",
        "a%2e%2e/b",
        "a b",
        "a\\b",
        "http://x/y",
        &format!("{}.webp", "a".repeat(256)),
    ] {
        assert!(!is_file_path(bad), "{bad:?}");
    }
}

#[test]
fn a_title_gets_the_filename_rules_and_a_cap() {
    assert_eq!(clean_title("Test Pack"), "Test Pack");
    assert_eq!(clean_title("  Cats \u{202E}x  "), "Cats x");
    assert_eq!(clean_title("a/b\\c"), "c");
    assert_eq!(clean_title(""), DEFAULT_TITLE);
    assert_eq!(clean_title("   "), DEFAULT_TITLE);
    assert_eq!(clean_title("\u{0}\u{7}"), DEFAULT_TITLE);
    assert_eq!(clean_title(".."), DEFAULT_TITLE);
    let cut = clean_title(&"a".repeat(300));
    assert_eq!(cut.chars().count(), MAX_TITLE_CHARS);
    // The file-name rules cut at 255 bytes first, so a two-byte letter ends a character early; never over 128.
    let cut = clean_title(&"я".repeat(300));
    assert!(cut.chars().count() <= MAX_TITLE_CHARS && cut.chars().all(|c| c == 'я'));
}

#[test]
fn the_source_names_round_trip() {
    for source in [PackSource::TelegramExport, PackSource::TelegramLink] {
        assert_eq!(PackSource::parse(source.as_str()), Some(source));
    }
    assert_eq!(PackSource::parse("telegram"), None);
}
