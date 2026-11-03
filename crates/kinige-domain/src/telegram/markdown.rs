//! Telegram message -> entry Markdown (slice 4, task 4.2). No IO.
//!
//! Placeholders and the "Переслано от" ("Forwarded from") lines are written into the text in the interface language at
//! the moment of import: this is data, not interface (slice 4 specification, decision 7). The result goes through
//! [`normalize_body`], so it is fit for `new_message` as is.

use super::{TgMedia, TgMessage, TgSpan, TgSpanKind};
use crate::groups::normalize_body;
use crate::lang::Language;

/// Characters that are escaped everywhere in plain text.
const SPECIAL: [char; 8] = ['\\', '*', '_', '`', '[', ']', '<', '~'];

/// Entry text for a message. Order of lines: author (if given), forward, attachment, text.
///
/// `None` means nothing but spaces is left after assembly (no text and no attachment), or the text does not fit the
/// `body` group (limit 512 KiB; Telegram messages are an order of magnitude shorter, so in practice this is only an
/// empty message).
pub fn render_body(
    message: &TgMessage,
    author: Option<&str>,
    language: Language,
) -> Option<String> {
    let t = Strings::of(language);
    // Content is the attachment and the text. The author name and "Переслано" do not count as content: a message without it is empty.
    let mut content: Vec<String> = Vec::new();
    if let Some(media) = &message.media {
        media_lines(media, &t, &mut content);
    }
    let text = render_spans(&message.text);
    if !text.trim().is_empty() {
        content.push(text);
    }
    if content.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = Vec::new();
    if let Some(name) = author.map(str::trim).filter(|n| !n.is_empty()) {
        lines.push(format!("**{}**", escape(name, false)));
    }
    if message.forwarded {
        let source = message
            .forwarded_from
            .as_deref()
            .or(message.saved_from.as_deref())
            .map(str::trim)
            .filter(|n| !n.is_empty());
        lines.push(match source {
            Some(name) => format!("_{} {}_", t.forwarded_from, escape(name, false)),
            None => format!("_{}_", t.forwarded),
        });
    }
    lines.extend(content);
    normalize_body(&lines.join("\n")).ok()
}

/// Captions of placeholders and forwards in one language.
struct Strings {
    forwarded_from: &'static str,
    forwarded: &'static str,
    poll: &'static str,
    /// "... not imported" templates: `{}` is the caption (file name, emoji), if there is one.
    photo: &'static str,
    sticker: &'static str,
    animation: &'static str,
    video: &'static str,
    video_message: &'static str,
    voice: &'static str,
    audio: &'static str,
    file: &'static str,
    location: &'static str,
    venue: &'static str,
    contact: &'static str,
    other: &'static str,
}

impl Strings {
    fn of(language: Language) -> Strings {
        match language {
            Language::Ru => Strings {
                forwarded_from: "Переслано от",
                forwarded: "Переслано",
                poll: "Опрос",
                photo: "Фото не импортировано",
                sticker: "Стикер{} не импортирован",
                animation: "Анимация не импортирована",
                video: "Видео не импортировано",
                video_message: "Видеосообщение не импортировано",
                voice: "Голосовое сообщение не импортировано",
                audio: "Аудио не импортировано",
                file: "Файл{} не импортирован",
                location: "Геопозиция не импортирована",
                venue: "Место{} не импортировано",
                contact: "Контакт не импортирован",
                other: "Вложение не импортировано",
            },
            Language::En => Strings {
                forwarded_from: "Forwarded from",
                forwarded: "Forwarded",
                poll: "Poll",
                photo: "Photo not imported",
                sticker: "Sticker{} not imported",
                animation: "Animation not imported",
                video: "Video not imported",
                video_message: "Video message not imported",
                voice: "Voice message not imported",
                audio: "Audio not imported",
                file: "File{} not imported",
                location: "Location not imported",
                venue: "Place{} not imported",
                contact: "Contact not imported",
                other: "Attachment not imported",
            },
        }
    }
}

fn media_lines(media: &TgMedia, t: &Strings, lines: &mut Vec<String>) {
    // Caption for the template: `" report.pdf"` or empty. Escaped as plain text.
    let with = |template: &str, label: Option<&str>| {
        let label = label
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| format!(" {}", escape(l, false)))
            .unwrap_or_default();
        format!("_[{}]_", template.replacen("{}", &label, 1))
    };
    let plain = |template: &str| with(template, None);
    match media {
        TgMedia::Poll { question, answers } => {
            lines.push(format!(
                "**{}:** {}",
                t.poll,
                escape(question.trim(), false)
            ));
            lines.extend(
                answers
                    .iter()
                    .map(|answer| format!("- {}", escape(answer.trim(), false))),
            );
        }
        TgMedia::Photo => lines.push(plain(t.photo)),
        TgMedia::Sticker { emoji } => lines.push(with(t.sticker, emoji.as_deref())),
        TgMedia::Animation => lines.push(plain(t.animation)),
        TgMedia::VideoFile => lines.push(plain(t.video)),
        TgMedia::VideoMessage => lines.push(plain(t.video_message)),
        TgMedia::VoiceMessage => lines.push(plain(t.voice)),
        TgMedia::AudioFile => lines.push(plain(t.audio)),
        TgMedia::File { name } => lines.push(with(t.file, name.as_deref())),
        TgMedia::Location => lines.push(plain(t.location)),
        TgMedia::Venue { name } => lines.push(with(t.venue, Some(name))),
        TgMedia::Contact => lines.push(plain(t.contact)),
        TgMedia::Other(_) => lines.push(plain(t.other)),
    }
}

/// Text pieces -> Markdown. Blocks (`Pre`, `Blockquote`) always start on a new line and end with a line break.
fn render_spans(spans: &[TgSpan]) -> String {
    let mut out = String::new();
    for span in spans {
        if span.text.is_empty() {
            continue;
        }
        let at_start = out.is_empty() || out.ends_with('\n');
        match &span.kind {
            TgSpanKind::Bold => out.push_str(&wrap(&span.text, "**")),
            TgSpanKind::Italic => out.push_str(&wrap(&span.text, "_")),
            TgSpanKind::Strike => out.push_str(&wrap(&span.text, "~~")),
            TgSpanKind::Code => out.push_str(&code_inline(&span.text)),
            TgSpanKind::Pre { language } => {
                ensure_new_line(&mut out);
                out.push_str(&fenced(&span.text, language.as_deref()));
            }
            TgSpanKind::Blockquote => {
                ensure_new_line(&mut out);
                for line in escape(&span.text, true).split('\n') {
                    out.push_str(if line.is_empty() { ">" } else { "> " });
                    out.push_str(line);
                    out.push('\n');
                }
            }
            TgSpanKind::TextLink { href } => {
                if span.text.trim().is_empty() {
                    out.push_str(&span.text);
                } else {
                    out.push_str(&format!(
                        "[{}]({})",
                        escape(&span.text, false),
                        destination(href)
                    ));
                }
            }
            // Verbatim: `#tag` stays a tag, an address stays a link.
            TgSpanKind::Link
            | TgSpanKind::Email
            | TgSpanKind::Phone
            | TgSpanKind::Mention
            | TgSpanKind::Hashtag
            | TgSpanKind::Cashtag
            | TgSpanKind::BotCommand => out.push_str(&span.text),
            // Underline and spoiler cannot be expressed in the entry Markdown; the rest is plain text.
            _ => out.push_str(&escape(&span.text, at_start)),
        }
    }
    out
}

fn ensure_new_line(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

/// Text in markers line by line: spaces at the line edges stay outside, empty lines get no markers.
fn wrap(text: &str, marker: &str) -> String {
    map_lines(text, |core| {
        format!("{marker}{}{marker}", escape(core, false))
    })
}

/// Inline code: one more backtick around than the longest run inside.
fn code_inline(text: &str) -> String {
    map_lines(text, |core| {
        let fence = "`".repeat(longest_run(core, '`') + 1);
        let pad = if core.starts_with('`') || core.ends_with('`') {
            " "
        } else {
            ""
        };
        format!("{fence}{pad}{core}{pad}{fence}")
    })
}

/// Applies `f` to the content of each line without edge spaces; spaces and empty lines are kept.
fn map_lines(text: &str, f: impl Fn(&str) -> String) -> String {
    text.split('\n')
        .map(|line| {
            let core = line.trim();
            if core.is_empty() {
                return line.to_string();
            }
            let start = line.len() - line.trim_start().len();
            let end = line.trim_end().len();
            // The bounds come from `trim_*`, so they lie on character boundaries.
            let head = line.get(..start).unwrap_or_default();
            let tail = line.get(end..).unwrap_or_default();
            format!("{head}{}{tail}", f(core))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn longest_run(text: &str, target: char) -> usize {
    let (mut best, mut current) = (0, 0);
    for c in text.chars() {
        current = if c == target { current + 1 } else { 0 };
        best = best.max(current);
    }
    best
}

/// A fenced code block on separate lines; the fence is longer than any run of backticks inside.
fn fenced(text: &str, language: Option<&str>) -> String {
    let fence = "`".repeat((longest_run(text, '`') + 1).max(3));
    let info: String = language
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '+' | '-' | '_' | '#' | '.'))
        .collect();
    let body = text.trim_matches('\n').trim_end();
    format!("{fence}{info}\n{body}\n{fence}\n")
}

/// Link destination: spaces, `<`, `>`, `\` and control characters are percent-encoded; brackets stay while they are paired.
fn destination(href: &str) -> String {
    let balanced = {
        let mut depth = 0i32;
        let mut ok = true;
        for c in href.chars() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    ok &= depth >= 0;
                }
                _ => {}
            }
        }
        ok && depth == 0
    };
    let mut out = String::with_capacity(href.len());
    for c in href.trim().chars() {
        if c.is_whitespace()
            || c.is_control()
            || matches!(c, '<' | '>' | '\\')
            || (!balanced && matches!(c, '(' | ')'))
        {
            let mut buf = [0u8; 4];
            for byte in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Plain text -> Markdown without formatting. `at_line_start`: the text stands at the start of an entry line (the first
/// line of the text and every following one), so list, quote and heading marks at the line start are escaped too.
fn escape(text: &str, at_line_start: bool) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let marker = if index > 0 || at_line_start {
            line_marker(line)
        } else {
            None
        };
        for (i, c) in line.char_indices() {
            if SPECIAL.contains(&c) || marker == Some(i) {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

/// Byte index of a character that turns the line into markup at its start: `#`, `>`, `-`, `+`, `N.`, `N)`, lines made
/// of only `-` or `=`. `*`, `_` and `~` are escaped everywhere, so they are not needed here.
fn line_marker(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let lead = line.len() - trimmed.len();
    let first = trimmed.chars().next()?;
    let after = |len: usize| trimmed.get(len..).and_then(|rest| rest.chars().next());
    let boundary = |c: Option<char>| c.is_none_or(|c| c == ' ' || c == '\t');
    match first {
        '#' => {
            let run = trimmed.chars().take_while(|c| *c == '#').count();
            (run <= 6 && boundary(after(run))).then_some(lead)
        }
        '>' => Some(lead),
        '-' | '+' if boundary(after(1)) => Some(lead),
        '-' | '=' if trimmed.chars().all(|c| c == first) => Some(lead),
        '0'..='9' => {
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            let mark = after(digits);
            (digits <= 9 && matches!(mark, Some('.' | ')')) && boundary(after(digits + 1)))
                .then_some(lead + digits)
        }
        _ => None,
    }
}
