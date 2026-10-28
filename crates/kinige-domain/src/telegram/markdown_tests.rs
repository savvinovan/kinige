//! Тесты `render_body`: таблицы «сущность → Markdown», экранирование, заглушки, пересылка, автор.

use super::*;
use crate::lang::Language;
use crate::tags::parse_tags;

pub(super) fn msg(text: Vec<TgSpan>) -> TgMessage {
    TgMessage {
        id: "1".to_owned(),
        service: false,
        unix: None,
        local: None,
        from: None,
        from_id: None,
        reply_to: None,
        forwarded: false,
        forwarded_from: None,
        saved_from: None,
        text,
        media: None,
    }
}

fn sp(kind: TgSpanKind, text: &str) -> TgSpan {
    TgSpan {
        kind,
        text: text.to_owned(),
    }
}

pub(super) fn plain(text: &str) -> TgSpan {
    sp(TgSpanKind::Plain, text)
}

pub(super) fn ru(spans: Vec<TgSpan>) -> Option<String> {
    render_body(&msg(spans), None, Language::Ru)
}

fn one(kind: TgSpanKind, text: &str) -> String {
    ru(vec![sp(kind, text)]).unwrap()
}

#[test]
fn entities_table() {
    let cases: Vec<(TgSpanKind, &str, &str)> = vec![
        (TgSpanKind::Plain, "просто", "просто"),
        (TgSpanKind::Bold, "жирный", "**жирный**"),
        (TgSpanKind::Italic, "курсив", "_курсив_"),
        (TgSpanKind::Strike, "было", "~~было~~"),
        (TgSpanKind::Code, "x = 1", "`x = 1`"),
        (
            TgSpanKind::TextLink {
                href: "https://example.org/a".to_owned(),
            },
            "сайт",
            "[сайт](https://example.org/a)",
        ),
        (
            TgSpanKind::Link,
            "https://example.org/a_b",
            "https://example.org/a_b",
        ),
        (TgSpanKind::Email, "a_b@example.org", "a_b@example.org"),
        (TgSpanKind::Phone, "+7 000 000-00-00", "+7 000 000-00-00"),
        (TgSpanKind::Mention, "@user_name", "@user_name"),
        (TgSpanKind::Hashtag, "#дела", "#дела"),
        (TgSpanKind::Cashtag, "$USD", "$USD"),
        (TgSpanKind::BotCommand, "/start", "/start"),
        // Остальные — как обычный текст.
        (TgSpanKind::Underline, "под чертой", "под чертой"),
        (TgSpanKind::Spoiler, "секрет", "секрет"),
        (TgSpanKind::MentionName, "Пётр", "Пётр"),
        (TgSpanKind::BankCard, "4111", "4111"),
        (TgSpanKind::CustomEmoji, "🙂", "🙂"),
        (TgSpanKind::Other("новое".to_owned()), "текст", "текст"),
    ];
    for (kind, text, expected) in cases {
        assert_eq!(one(kind.clone(), text), expected, "{kind:?}");
    }
}

#[test]
fn spaces_move_outside_the_markers() {
    assert_eq!(
        ru(vec![
            plain("а"),
            sp(TgSpanKind::Bold, " жирный "),
            plain("б")
        ])
        .unwrap(),
        "а **жирный** б"
    );
    assert_eq!(
        ru(vec![
            plain("а"),
            sp(TgSpanKind::Italic, "\tкурсив\u{a0}"),
            plain("б")
        ])
        .unwrap(),
        "а\t_курсив_\u{a0}б"
    );
    // Пустая сущность маркеров не получает.
    assert_eq!(
        ru(vec![plain("а"), sp(TgSpanKind::Bold, "   "), plain("б")]).unwrap(),
        "а   б"
    );
    assert_eq!(ru(vec![sp(TgSpanKind::Bold, "")]), None);
}

#[test]
fn multiline_entities_are_wrapped_per_line() {
    assert_eq!(one(TgSpanKind::Bold, "раз\n\nдва"), "**раз**\n\n**два**");
    assert_eq!(one(TgSpanKind::Strike, "a\nb"), "~~a~~\n~~b~~");
}

#[test]
fn inline_code_uses_one_more_backtick_than_the_longest_run() {
    assert_eq!(one(TgSpanKind::Code, "a`b"), "``a`b``");
    assert_eq!(one(TgSpanKind::Code, "a``b`c"), "```a``b`c```");
    // Кавычка с краю: пробелы по бокам, иначе она склеится с оградой.
    assert_eq!(one(TgSpanKind::Code, "`x"), "`` `x ``");
    // В коде ничего не экранируется.
    assert_eq!(one(TgSpanKind::Code, "a*b_c[d]"), "`a*b_c[d]`");
}

#[test]
fn pre_block_goes_on_its_own_lines_with_language() {
    let pre = |language: Option<&str>| TgSpanKind::Pre {
        language: language.map(str::to_owned),
    };
    assert_eq!(
        ru(vec![
            plain("до\n"),
            sp(pre(Some("rust")), "fn main() {}\n"),
            plain("после")
        ])
        .unwrap(),
        "до\n```rust\nfn main() {}\n```\nпосле"
    );
    // Блок посреди строки начинается с новой строки.
    assert_eq!(
        ru(vec![plain("до"), sp(pre(None), "x"), plain("после")]).unwrap(),
        "до\n```\nx\n```\nпосле"
    );
    // Ограда длиннее любой серии кавычек внутри; обязательный `*` и `_` внутри не экранируются.
    assert_eq!(
        one(pre(None), "```\na_b*\n```"),
        "````\n```\na_b*\n```\n````"
    );
    // Язык с лишними знаками не ломает строку-открытие.
    assert_eq!(one(pre(Some("c`++ x")), "x"), "```c++x\nx\n```");
}

#[test]
fn blockquote_prefixes_every_line() {
    assert_eq!(one(TgSpanKind::Blockquote, "раз\nдва"), "> раз\n> два");
    assert_eq!(one(TgSpanKind::Blockquote, "раз\n\nтри"), "> раз\n>\n> три");
    assert_eq!(
        ru(vec![
            plain("до "),
            sp(TgSpanKind::Blockquote, "цитата"),
            plain("после")
        ])
        .unwrap(),
        "до \n> цитата\nпосле"
    );
    // Внутри цитаты знак списка в начале строки экранируется.
    assert_eq!(
        one(TgSpanKind::Blockquote, "- не список"),
        "> \\- не список"
    );
}

#[test]
fn text_link_escapes_text_and_keeps_parentheses_balanced() {
    let link = |href: &str, text: &str| {
        one(
            TgSpanKind::TextLink {
                href: href.to_owned(),
            },
            text,
        )
    };
    assert_eq!(
        link("https://example.org/a_(b)", "статья"),
        "[статья](https://example.org/a_(b))"
    );
    // Непарная скобка в адресе — процентами, иначе ссылка закроется раньше.
    assert_eq!(
        link("https://example.org/a)b", "x"),
        "[x](https://example.org/a%29b)"
    );
    assert_eq!(
        link("https://example.org/a b<c>", "x"),
        "[x](https://example.org/a%20b%3Cc%3E)"
    );
    assert_eq!(
        link("https://e.org", "[скобки] *и* _знаки_"),
        "[\\[скобки\\] \\*и\\* \\_знаки\\_](https://e.org)"
    );
}

#[test]
fn plain_text_escaping() {
    assert_eq!(
        one(TgSpanKind::Plain, "a*b_c`d[e]f<g~h\\i"),
        "a\\*b\\_c\\`d\\[e\\]f\\<g\\~h\\\\i"
    );
    // Знаки, безвредные в середине строки, не трогаются.
    assert_eq!(
        one(TgSpanKind::Plain, "a > b - c # d + e 1. f"),
        "a > b - c # d + e 1. f"
    );
    assert_eq!(one(TgSpanKind::Plain, "цена 5$ & 10%"), "цена 5$ & 10%");
}

#[test]
fn line_start_markers_are_escaped() {
    let cases = [
        ("# заголовок", "\\# заголовок"),
        ("### три", "\\### три"),
        ("#", "\\#"),
        ("> цитата", "\\> цитата"),
        (">без пробела", "\\>без пробела"),
        ("- пункт", "\\- пункт"),
        ("+ пункт", "\\+ пункт"),
        ("-", "\\-"),
        ("12. пункт", "12\\. пункт"),
        ("3) пункт", "3\\) пункт"),
        ("---", "\\---"),
        ("===", "\\==="),
        ("  - с отступом", "  \\- с отступом"),
        // Не маркеры:
        ("#тег", "#тег"),
        ("####### семь", "####### семь"),
        ("-5 градусов", "-5 градусов"),
        ("2024 год", "2024 год"),
        ("1.5 литра", "1.5 литра"),
        ("-- тире", "-- тире"),
    ];
    for (input, expected) in cases {
        assert_eq!(one(TgSpanKind::Plain, input), expected, "{input}");
    }
    // Каждая строка текста, а не только первая.
    assert_eq!(
        one(TgSpanKind::Plain, "раз\n- два\n# три"),
        "раз\n\\- два\n\\# три"
    );
    // Сразу после предыдущего текста в той же строке — не начало строки.
    assert_eq!(ru(vec![plain("а"), plain("- б")]).unwrap(), "а- б");
    assert_eq!(ru(vec![plain("а\n"), plain("- б")]).unwrap(), "а\n\\- б");
}

#[test]
fn hashtag_stays_a_tag_and_escapes_do_not_create_tags() {
    let body = ru(vec![
        plain("Сегодня "),
        sp(TgSpanKind::Hashtag, "#Работа"),
        plain(" и "),
        sp(TgSpanKind::Hashtag, "#идея_1"),
        plain(" # не тег"),
    ])
    .unwrap();
    assert_eq!(parse_tags(&body), ["работа", "идея_1"]);
    // Код с # — не тег; ссылка с # — не тег.
    let body = ru(vec![
        sp(TgSpanKind::Code, "#нет"),
        plain(" "),
        sp(
            TgSpanKind::TextLink {
                href: "https://e.org/#раздел".to_owned(),
            },
            "ссылка",
        ),
        plain(" "),
        sp(TgSpanKind::Link, "https://e.org/#якорь"),
    ])
    .unwrap();
    assert_eq!(parse_tags(&body), Vec::<String>::new());
}

#[test]
fn result_is_normalized() {
    // Конец строки приведён к `\n`, хвостовые пробелы и ведущие пустые строки убраны.
    assert_eq!(ru(vec![plain("\r\n\r\nтекст  \r\n")]).unwrap(), "текст");
}

#[test]
fn rendered_text_keeps_real_world_mix() {
    let body = ru(vec![
        plain("Купить "),
        sp(TgSpanKind::Bold, "молоко"),
        plain(" и хлеб, см. "),
        sp(
            TgSpanKind::TextLink {
                href: "https://example.org/a".to_owned(),
            },
            "список",
        ),
        plain(" "),
        sp(TgSpanKind::Hashtag, "#покупки"),
    ])
    .unwrap();
    assert_eq!(
        body,
        "Купить **молоко** и хлеб, см. [список](https://example.org/a) #покупки"
    );
    assert_eq!(parse_tags(&body), ["покупки"]);
}
