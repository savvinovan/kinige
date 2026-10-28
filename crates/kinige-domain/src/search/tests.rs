use super::*;

/// Основы слов через пробел: проверка «эти формы — одно слово для поиска».
fn stems(text: &str) -> String {
    index_text(text)
}

#[test]
fn index_text_keeps_only_stems_of_words() {
    assert_eq!(index_text("Купить МОЛОКО и хлеб!"), "куп молок и хлеб");
    assert_eq!(index_text(""), "");
    assert_eq!(index_text("  ... \n ** "), "");
    // Разметка Markdown и адреса дают слова, как любой другой текст.
    assert_eq!(
        index_text("**жирный** [ссылка](https://example.org/a_b) #дела"),
        "жирн ссылк https exampl org a b дел"
    );
}

#[test]
fn word_forms_of_the_spike_pairs_share_a_stem() {
    let same = [
        ("книга", "книги"),
        ("книга", "книгу"),
        ("книга", "книгой"),
        ("книги", "книгам"),
        ("ёлка", "елка"),
        ("ёлка", "Ёлки"),
        ("длинный", "длиннее"),
        ("длинный", "длинная"),
        ("молоко", "молока"),
        ("молоко", "молоком"),
        ("работать", "работал"),
        ("красивый", "красиво"),
        ("город", "городом"),
        ("встреча", "встречей"),
        ("run", "running"),
        ("run", "Runs"),
        ("city", "cities"),
        ("meeting", "meetings"),
    ];
    for (a, b) in same {
        assert_eq!(stems(a), stems(b), "{a} / {b}");
    }
    // Известные промахи Snowball: это не ошибка, а граница метода (ADR-0008).
    let apart = [
        ("бежал", "бегу"),
        ("человек", "люди"),
        ("день", "дня"),
        ("идти", "шёл"),
        ("go", "went"),
        // «й» не склеивается с «и»: «йод» и «иод» — разные слова.
        ("йод", "иод"),
    ];
    for (a, b) in apart {
        assert_ne!(stems(a), stems(b), "{a} / {b}");
    }
}

#[test]
fn yo_is_folded_and_case_does_not_matter() {
    assert_eq!(index_text("ЁЛКА Ёлка ёлка елка"), "елк елк елк елк");
    assert_eq!(index_text("ПРИВЕТ"), index_text("привет"));
}

#[test]
fn mixed_languages_digits_and_other_alphabets() {
    assert_eq!(index_text("Привет world 2026"), "привет world 2026");
    assert_eq!(index_text("meeting в 15:30"), "meet в 15 30");
    // Слово со смесью букв и цифр, с латиницей и кириллицей, и чужой алфавит остаются как есть.
    assert_eq!(
        index_text("abc123 日本語 ÅngströM"),
        "abc123 日本語 ångström"
    );
    assert_eq!(index_text("mixрусский"), "mixрусский");
}

#[test]
fn query_last_word_is_a_prefix_while_typing() {
    assert_eq!(fts_query("книги").as_deref(), Some("\"книг\"*"));
    assert_eq!(fts_query("моло").as_deref(), Some("\"мол\"*"));
    assert_eq!(fts_query("длинн").as_deref(), Some("\"длин\"*"));
    assert_eq!(
        fts_query("купить моло").as_deref(),
        Some("\"куп\" \"мол\"*")
    );
    // Пробел или знак после слова — слово закончено: точный терм.
    assert_eq!(fts_query("книги ").as_deref(), Some("\"книг\""));
    assert_eq!(fts_query("книги,").as_deref(), Some("\"книг\""));
    assert_eq!(
        fts_query("купить молоко ").as_deref(),
        Some("\"куп\" \"молок\"")
    );
    // Один знак — не префикс: `м*` совпало бы с четвертью индекса.
    assert_eq!(fts_query("я").as_deref(), Some("\"я\""));
    assert_eq!(fts_query("купить в").as_deref(), Some("\"куп\" \"в\""));
    assert_eq!(fts_query("мо").as_deref(), Some("\"мо\"*"));
}

#[test]
fn query_cannot_carry_fts5_syntax() {
    let query = fts_query("\" OR * NEAR( ^ - :col").expect("слова есть");
    assert_eq!(query, "\"or\" \"near\" \"col\"*");
    // В запросе нет ничего, кроме кавычек, пробелов, звёздочки в конце и букв или цифр основ.
    for input in [
        "a\"b",
        "x AND y OR NOT z",
        "col:val",
        "title:*",
        "NEAR(a b, 5)",
        "{a b} : c",
        "^start",
        "-minus",
        "\"unclosed",
        "(a OR b) AND c*",
        "a*b",
    ] {
        let query = fts_query(input).unwrap();
        let stripped = query.trim_end_matches('*');
        for term in stripped.split(' ') {
            assert!(
                term.starts_with('"') && term.ends_with('"') && term.len() >= 3,
                "{input} -> {query}"
            );
            let inner = &term[1..term.len() - 1];
            assert!(
                inner.chars().all(char::is_alphanumeric),
                "{input} -> {query}"
            );
        }
        assert_eq!(
            query.matches('*').count(),
            usize::from(query.ends_with('*'))
        );
    }
}

#[test]
fn query_without_words_is_none() {
    for input in ["", " ", "\n\t", "***", "\" \"", "- : ^ ( )", "…", "!?"] {
        assert_eq!(fts_query(input), None, "{input:?}");
    }
}

#[test]
fn query_with_digits_yo_and_mixed_text() {
    assert_eq!(fts_query("Ёлка 2026 ").as_deref(), Some("\"елк\" \"2026\""));
    assert_eq!(fts_query("meeting 15").as_deref(), Some("\"meet\" \"15\"*"));
    assert_eq!(
        fts_query("Привет, world").as_deref(),
        Some("\"привет\" \"world\"*")
    );
}

#[test]
fn highlight_finds_the_forms_that_are_in_the_text() {
    let text = "Книга на столе. Я читаю книгу, а книжка рядом. Книга!";
    assert_eq!(highlight_terms(text, "книги "), ["Книга", "книгу"]);
    // Другого слова с похожим началом нет среди совпавших.
    assert!(!highlight_terms(text, "книги ").contains(&"книжка".to_string()));
}

#[test]
fn highlight_follows_the_prefix_rule_for_the_last_word() {
    let text = "Купил молоко и молока ещё. Молодой человек, мол.";
    // «моло» — основа `мол`, начало основ «молок», «молод»; слово «мол» тоже совпало.
    assert_eq!(
        highlight_terms(text, "моло"),
        ["молоко", "молока", "Молодой", "мол"]
    );
    // Закончено пробелом — только основа `молок`.
    assert_eq!(highlight_terms(text, "моло "), ["мол"]);
    assert_eq!(highlight_terms(text, "молоко "), ["молоко", "молока"]);
    // Не последние термы — точные.
    assert_eq!(
        highlight_terms(text, "молоко ещ"),
        ["молоко", "молока", "ещё"]
    );
}

#[test]
fn highlight_edge_cases() {
    assert!(highlight_terms("текст", "").is_empty());
    assert!(highlight_terms("текст", "***").is_empty());
    assert!(highlight_terms("текст", "нет").is_empty());
    assert!(highlight_terms("", "нет").is_empty());
    // Ё и регистр не мешают, слово отдаётся как написано.
    assert_eq!(highlight_terms("Ёлка и ёлки", "елка "), ["Ёлка", "ёлки"]);
    // Слов не больше предела.
    let many: String = (0..200).map(|i| format!("мол{i} ")).collect();
    assert_eq!(highlight_terms(&many, "мол").len(), 64);
}

fn terms(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn snippet_of_short_text_is_the_plain_text() {
    assert_eq!(
        snippet("Купить молоко", &terms(&["молоко"]), 100),
        "Купить молоко"
    );
    assert_eq!(
        snippet(
            "**Купить** [молоко](https://x.org/a_b)\n- [ ] хлеб\n> цитата #дела",
            &[],
            100
        ),
        "Купить молоко хлеб цитата #дела"
    );
    assert_eq!(snippet("", &[], 10), "");
    assert_eq!(snippet("текст", &[], 0), "");
}

#[test]
fn snippet_strips_markdown_but_keeps_words_of_code_and_snake_case() {
    let md = "# Заголовок\n\n```rust\nfn main_run() {}\n```\n1. пункт `код` и ~~старое~~ \\*звезда\\* snake_case";
    assert_eq!(
        snippet(md, &[], 200),
        "Заголовок fn main_run() {} пункт код и старое *звезда* snake_case"
    );
    assert_eq!(
        snippet("<https://example.org/a> тут", &[], 100),
        "https://example.org/a тут"
    );
    assert_eq!(snippet("a < b и b > a", &[], 100), "a < b и b > a");
}

fn long_text() -> String {
    (0..40)
        .map(|i| format!("слово{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn snippet_centers_on_the_first_match_with_ellipses_inside_the_limit() {
    let text = long_text();
    let out = snippet(&text, &terms(&["слово20"]), 60);
    assert!(out.starts_with('…') && out.ends_with('…'), "{out}");
    assert!(out.chars().count() <= 60, "{out}");
    assert!(out.contains("слово20"));
    // Режем по границе слова: каждое слово сниппета — целое слово исходного текста.
    let source: Vec<&str> = text.split(' ').collect();
    for word in out.trim_matches('…').split(' ') {
        assert!(source.contains(&word), "обрезанное слово {word:?} в {out}");
    }
}

#[test]
fn snippet_without_a_match_is_the_start_of_the_text() {
    let text = long_text();
    let out = snippet(&text, &terms(&["нет"]), 40);
    assert!(out.starts_with("слово0 слово1"), "{out}");
    assert!(out.ends_with('…') && !out.starts_with('…'));
    assert!(out.chars().count() <= 40);
    assert_eq!(snippet(&text, &[], 40), out);
}

#[test]
fn snippet_with_a_match_at_the_very_end_of_a_long_text() {
    let text = long_text();
    let out = snippet(&text, &terms(&["слово39"]), 50);
    assert!(out.starts_with('…') && !out.ends_with('…'), "{out}");
    assert!(out.ends_with("слово39"));
    assert!(out.chars().count() <= 50);
}

#[test]
fn snippet_keeps_emoji_and_cyrillic_whole_at_the_edges() {
    // Обрезка в любом месте не рвёт знак: проверяем каждый предел, строка остаётся валидной и в пределе.
    let text =
        "🙂 привет, ёлка 🎄 встретилась 🙂🙂 друзьям мир 🌍 здесь и 😀 сейчас ещё раз 🙂 конец";
    for max in 1..text.chars().count() + 3 {
        for hit in ["ёлка", "здесь", "конец", "нет"] {
            let out = snippet(text, &terms(&[hit]), max);
            assert!(out.chars().count() <= max, "{max}: {out}");
        }
    }
    let out = snippet(text, &terms(&["встретилась"]), 30);
    assert!(out.contains("встретилась"), "{out}");
    for word in out.trim_matches('…').split(' ') {
        assert!(text.split(' ').any(|w| w == word), "{word:?} в {out}");
    }
}

#[test]
fn snippet_cuts_a_single_giant_word_by_characters() {
    let text = format!("{} хвост", "ж".repeat(200));
    let out = snippet(&text, &[], 20);
    assert_eq!(out.chars().count(), 20);
    assert!(out.ends_with('…'));
    assert!(out.starts_with("жжж"));
}

#[test]
fn snippet_matches_terms_regardless_of_case_and_yo() {
    let text = format!("{} Ёлка {}", long_text(), long_text());
    let out = snippet(&text, &terms(&["елка"]), 50);
    assert!(out.contains("Ёлка"), "{out}");
}
