//! Telegram-сообщение → Markdown записи (слайс 4, задача 4.2). Без IO.
//!
//! Заглушки и строки «Переслано от» пишутся в текст на языке интерфейса в момент импорта: это данные, а не интерфейс
//! (спецификация слайса 4, решение 7). Результат проходит [`normalize_body`], поэтому годен для `new_message` как есть.

use super::{TgMedia, TgMessage, TgSpan, TgSpanKind};
use crate::groups::normalize_body;
use crate::lang::Language;

/// Знаки, которые в обычном тексте экранируются везде.
const SPECIAL: [char; 8] = ['\\', '*', '_', '`', '[', ']', '<', '~'];

/// Текст записи по сообщению. Порядок строк: автор (если передан), пересылка, вложение, текст.
///
/// `None` — после сборки не осталось ничего, кроме пробелов (нет текста и вложения), или текст не влезает в группу
/// `body` (предел 512 КиБ; сообщения Telegram на порядок короче, поэтому на практике это только пустое сообщение).
pub fn render_body(
    message: &TgMessage,
    author: Option<&str>,
    language: Language,
) -> Option<String> {
    let t = Strings::of(language);
    // Содержание — вложение и текст. Имя автора и «Переслано» содержанием не считаются: сообщение без него пустое.
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

/// Подписи заглушек и пересылки на одном языке.
struct Strings {
    forwarded_from: &'static str,
    forwarded: &'static str,
    poll: &'static str,
    /// Шаблоны «… не импортировано»: `{}` — подпись (имя файла, эмодзи), если она есть.
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
    // Подпись к шаблону: `" report.pdf"` или пусто. Экранируется как обычный текст.
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

/// Куски текста → Markdown. Блоки (`Pre`, `Blockquote`) всегда начинаются с новой строки и заканчиваются переводом строки.
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
            // Дословно: `#тег` остаётся тегом, адрес — ссылкой.
            TgSpanKind::Link
            | TgSpanKind::Email
            | TgSpanKind::Phone
            | TgSpanKind::Mention
            | TgSpanKind::Hashtag
            | TgSpanKind::Cashtag
            | TgSpanKind::BotCommand => out.push_str(&span.text),
            // Подчёркивание и спойлер в Markdown записи не выражаются, остальное — как обычный текст.
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

/// Текст в маркерах построчно: пробелы по краям строки остаются снаружи, пустые строки маркеров не получают.
fn wrap(text: &str, marker: &str) -> String {
    map_lines(text, |core| {
        format!("{marker}{}{marker}", escape(core, false))
    })
}

/// Код в строке: обратных кавычек вокруг на одну больше, чем самая длинная серия внутри.
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

/// Применяет `f` к содержимому каждой строки без пробелов по краям; пробелы и пустые строки сохраняются.
fn map_lines(text: &str, f: impl Fn(&str) -> String) -> String {
    text.split('\n')
        .map(|line| {
            let core = line.trim();
            if core.is_empty() {
                return line.to_string();
            }
            let start = line.len() - line.trim_start().len();
            let end = line.trim_end().len();
            format!("{}{}{}", &line[..start], f(core), &line[end..])
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

/// Ограждённый блок кода на отдельных строках; ограда длиннее любой серии кавычек внутри.
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

/// Адрес ссылки: пробелы, `<`, `>`, `\` и управляющие знаки — процентами; скобки остаются, пока они парные.
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

/// Обычный текст → Markdown без форматирования. `at_line_start`: текст стоит в начале строки записи (первая строка текста
/// и каждая следующая), поэтому знаки списка, цитаты и заголовка в начале строки тоже экранируются.
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

/// Байтовый индекс знака, который в начале строки превращает её в разметку: `#`, `>`, `-`, `+`, `N.`, `N)`, строки из
/// одних `-` или `=`. `*`, `_` и `~` экранируются везде, поэтому здесь не нужны.
fn line_marker(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let lead = line.len() - trimmed.len();
    let first = trimmed.chars().next()?;
    let after = |len: usize| trimmed[len..].chars().next();
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
            let mark = trimmed[digits..].chars().next();
            (digits <= 9 && matches!(mark, Some('.' | ')')) && boundary(after(digits + 1)))
                .then_some(lead + digits)
        }
        _ => None,
    }
}
