use super::*;

fn entry(time: &str, body: &str) -> MirrorEntry {
    MirrorEntry {
        time: time.to_string(),
        body_md: body.to_string(),
        reply: None,
        attachments: Vec::new(),
    }
}

fn reply_entry(time: &str, body: &str, reply: MirrorReply) -> MirrorEntry {
    MirrorEntry {
        reply: Some(reply),
        ..entry(time, body)
    }
}

#[test]
fn day_file_has_iso_date_and_24_hour_time_headings() {
    let text = render_day_file(
        "2026-10-21",
        &[
            entry("09:12", "Купить **молоко**\n- [ ] позвонить маме"),
            entry("21:05", "Вечером: код"),
        ],
    );
    assert_eq!(
        text,
        "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n\n## 21:05\n\nВечером: код\n"
    );
}

#[test]
fn a_reply_has_a_link_line_under_the_time_heading() {
    let original = MirrorReply::Original {
        date: "2023-10-31".into(),
        time: "12:05".into(),
        link: Some(relative_day_link(
            "Inbox",
            &day_file_segments("Inbox", "2023-10-31").unwrap(),
        )),
    };
    assert_eq!(
        render_day_file(
            "2026-10-31",
            &[reply_entry("09:12", "До сих пор так думаю", original)]
        ),
        "# 2026-10-31\n\n## 09:12\n\n↩ [2023-10-31 12:05](<../2023/2023-10-31.md>)\n\nДо сих пор так думаю\n"
    );
}

#[test]
fn a_reply_to_a_missing_or_trashed_entry_has_no_link() {
    let trashed = MirrorReply::Original {
        date: "2023-10-31".into(),
        time: "12:05".into(),
        link: None,
    };
    let text = render_day_file(
        "2026-10-31",
        &[
            reply_entry("09:12", "раз", MirrorReply::Gone),
            reply_entry("09:13", "два", trashed),
        ],
    );
    assert_eq!(
        text,
        "# 2026-10-31\n\n## 09:12\n\n↩\n\nраз\n\n## 09:13\n\n↩ 2023-10-31 12:05\n\nдва\n"
    );
}

#[test]
fn relative_links_cross_chats_and_keep_spaces_inside_angle_brackets() {
    let other = day_file_segments("Книги (2)", "2023-03-14").unwrap();
    assert_eq!(
        relative_day_link("Inbox", &other),
        "../../Книги (2)/2023/2023-03-14.md"
    );
    let link = relative_day_link("Inbox", &other);
    let line = reply_line(&MirrorReply::Original {
        date: "2023-03-14".into(),
        time: "08:00".into(),
        link: Some(link),
    });
    assert_eq!(
        line,
        "↩ [2023-03-14 08:00](<../../Книги (2)/2023/2023-03-14.md>)"
    );
}

#[test]
fn empty_day_has_only_the_date_heading() {
    assert_eq!(render_day_file("2026-10-21", &[]), "# 2026-10-21\n");
}

#[test]
fn atx_headings_inside_entries_are_demoted_below_the_entry_level() {
    let body = "# Заголовок\nтекст\n## Второй\n### Третий\n#### Четвёртый\n##### Пятый\n###### Шестой\n#не заголовок\n#";
    assert_eq!(
        demote_headings(body),
        "### Заголовок\nтекст\n#### Второй\n##### Третий\n###### Четвёртый\n###### Пятый\n###### Шестой\n#не заголовок\n###"
    );
}

#[test]
fn entry_cannot_fake_another_entry() {
    let text = render_day_file(
        "2026-10-21",
        &[entry("09:12", "## 10:00\nподделка\n# 2026-01-01")],
    );
    let headings: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("# ") || l.starts_with("## "))
        .collect();
    assert_eq!(headings, ["# 2026-10-21", "## 09:12"]);
}

#[test]
fn setext_headings_become_atx_below_the_entry_level() {
    assert_eq!(
        demote_headings("Название\n=======\nтекст"),
        "### Название\nтекст"
    );
    assert_eq!(
        demote_headings("Название\n---\nтекст"),
        "#### Название\nтекст"
    );
    let text = render_day_file("2026-10-21", &[entry("09:12", "Название\n===\nтекст")]);
    assert!(text.lines().all(|l| !l.starts_with("===")));
    assert_eq!(
        text.lines()
            .filter(|l| l.starts_with("# ") || l.starts_with("## "))
            .count(),
        2
    );
}

#[test]
fn thematic_break_and_lists_are_not_mistaken_for_setext() {
    assert_eq!(
        demote_headings("абзац\n\n---\nещё"),
        "абзац\n\n---\nещё",
        "после пустой строки это разделитель"
    );
    assert_eq!(demote_headings("- пункт\n---"), "- пункт\n---");
    assert_eq!(demote_headings("1. пункт\n---"), "1. пункт\n---");
    assert_eq!(demote_headings("> цитата\n==="), "> цитата\n===");
}

#[test]
fn fenced_code_is_left_untouched() {
    let body = "до\n```sh\n# комментарий\nТитул\n===\n```\n# после";
    assert_eq!(
        demote_headings(body),
        "до\n```sh\n# комментарий\nТитул\n===\n```\n### после"
    );
    let tilde = "~~~\n# внутри\n~~~\n# снаружи";
    assert_eq!(demote_headings(tilde), "~~~\n# внутри\n~~~\n### снаружи");
}

#[test]
fn fence_closes_only_with_a_long_enough_matching_marker() {
    let body = "````\n```\n# всё ещё внутри\n````\n# снаружи";
    assert_eq!(
        demote_headings(body),
        "````\n```\n# всё ещё внутри\n````\n### снаружи"
    );
    // An unclosed fence up to the end of the entry: headings inside are left alone (as a Markdown renderer does).
    assert_eq!(demote_headings("```\n# внутри"), "```\n# внутри");
}

#[test]
fn indented_code_is_not_a_heading() {
    assert_eq!(demote_headings("    # код"), "    # код");
    assert_eq!(demote_headings("   # заголовок"), "   ### заголовок");
}

#[test]
fn paths_use_chat_folder_year_and_date() {
    assert_eq!(
        day_file_segments(INBOX_FOLDER, "2026-10-21"),
        Some([
            "Inbox".to_string(),
            "2026".to_string(),
            "2026-10-21.md".to_string()
        ])
    );
}

#[test]
fn paths_reject_anything_but_a_plain_date_and_folder() {
    for date in ["", "../../x", "2026-10-21/../..", "2026-1-1"] {
        assert_eq!(day_file_segments("Inbox", date), None, "{date}");
    }
    for folder in ["", "..", ".", "a/b", "a\\b"] {
        assert_eq!(day_file_segments(folder, "2026-10-21"), None, "{folder}");
    }
}

fn image(name: &str, file: &str) -> MirrorAttachment {
    MirrorAttachment {
        is_image: true,
        name: name.to_string(),
        file: file.to_string(),
    }
}

#[test]
fn attachments_are_linked_into_the_day_folder_above_the_text() {
    let with_files = MirrorEntry {
        attachments: vec![
            image("IMG_0001.jpg", "IMG_0001.jpg"),
            MirrorAttachment {
                is_image: false,
                name: "menu [v2].pdf".to_string(),
                file: "menu [v2].pdf".to_string(),
            },
        ],
        ..entry("09:12", "Lunch")
    };
    assert_eq!(
        render_day_file("2026-11-03", &[with_files]),
        "# 2026-11-03\n\n## 09:12\n\n![IMG_0001.jpg](<2026-11-03/IMG_0001.jpg>)\n\n[menu \\[v2\\].pdf](<2026-11-03/menu [v2].pdf>)\n\nLunch\n"
    );
}

#[test]
fn an_entry_without_text_ends_after_its_last_attachment() {
    let photo_only = MirrorEntry {
        attachments: vec![image("a.png", "a.png")],
        ..entry("09:12", "")
    };
    assert_eq!(
        render_day_file("2026-11-03", &[photo_only, entry("09:13", "next")]),
        "# 2026-11-03\n\n## 09:12\n\n![a.png](<2026-11-03/a.png>)\n\n## 09:13\n\nnext\n"
    );
}
