//! Минимальный разбор сообщения JSON-RPC: мосту нужны только `id` и `method` верхнего уровня (ADR-0009). Остальное он не
//! читает и не хранит. Никакой паники на мусоре: любая неразобранная строка даёт пустой результат и пересылается как есть.

/// Что мост знает о сообщении.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// `id` дословно, как в строке (число или строка JSON); другие значения (`null`, объект) не считаются.
    pub id: Option<String>,
    /// `method`, если это простая строка без escape-последовательностей.
    pub method: Option<String>,
}

impl Scan {
    /// Запрос: есть и `method`, и `id`. Только запросы ждут ответа.
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }

    /// Ответ: есть `id` и нет `method` (в том числе ответ клиента на запрос сервера).
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.method.is_none()
    }
}

pub fn scan(line: &[u8]) -> Scan {
    let mut found = Scan::default();
    match scan_object(line, &mut found) {
        Some(()) => found,
        None => Scan::default(),
    }
}

/// Разбирает только начало слишком длинной строки: `id`, если он стоит раньше места обрыва. Нужен, чтобы ответить на
/// отвергнутый кадр ошибкой с настоящим `id`, а не с `null`.
pub fn scan_prefix(prefix: &[u8]) -> Scan {
    let mut found = Scan::default();
    let _ = scan_object(prefix, &mut found);
    found
}

/// Строка — целый JSON-объект верхнего уровня (по структуре; значения не проверяются).
pub fn is_object(line: &[u8]) -> bool {
    scan_object(line, &mut Scan::default()).is_some()
}

fn scan_object(line: &[u8], found: &mut Scan) -> Option<()> {
    let mut i = skip_space(line, 0);
    if *line.get(i)? != b'{' {
        return None;
    }
    i += 1;
    loop {
        i = skip_space(line, i);
        match *line.get(i)? {
            b'}' => return Some(()),
            b',' => {
                i += 1;
                continue;
            }
            b'"' => {}
            _ => return None,
        }
        let key_end = string_end(line, i)?;
        let key = line.get(i + 1..key_end - 1)?;
        i = skip_space(line, key_end);
        if *line.get(i)? != b':' {
            return None;
        }
        i = skip_space(line, i + 1);
        let end = value_end(line, i)?;
        let value = line.get(i..end)?;
        match key {
            b"id" => found.id = id_text(value),
            b"method" => found.method = method_text(value),
            _ => {}
        }
        i = end;
    }
}

fn id_text(value: &[u8]) -> Option<String> {
    let first = *value.first()?;
    let is_number = first == b'-' || first.is_ascii_digit();
    if first == b'"' || is_number {
        String::from_utf8(value.to_vec()).ok()
    } else {
        None
    }
}

fn method_text(value: &[u8]) -> Option<String> {
    let inner = value.strip_prefix(b"\"")?.strip_suffix(b"\"")?;
    if inner.contains(&b'\\') {
        return None;
    }
    String::from_utf8(inner.to_vec()).ok()
}

fn skip_space(line: &[u8], mut i: usize) -> usize {
    while matches!(line.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        i += 1;
    }
    i
}

/// Индекс сразу после закрывающей кавычки строки, которая начинается в `start`.
fn string_end(line: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    loop {
        match *line.get(i)? {
            b'"' => return Some(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
}

/// Индекс сразу после значения, которое начинается в `start`.
fn value_end(line: &[u8], start: usize) -> Option<usize> {
    match *line.get(start)? {
        b'"' => string_end(line, start),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut i = start;
            loop {
                match *line.get(i)? {
                    b'"' => i = string_end(line, i)?,
                    b'{' | b'[' => {
                        depth += 1;
                        i += 1;
                    }
                    b'}' | b']' => {
                        depth = depth.checked_sub(1)?;
                        i += 1;
                        if depth == 0 {
                            return Some(i);
                        }
                    }
                    _ => i += 1,
                }
            }
        }
        _ => {
            let mut i = start;
            while !matches!(
                line.get(i),
                None | Some(b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n')
            ) {
                i += 1;
            }
            (i > start).then_some(i)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Scan {
        scan(text.as_bytes())
    }

    #[test]
    fn reads_id_and_method_whatever_the_order_and_nesting() {
        let request = s(
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"id":99,"method":"x","a":["}",{"id":1}]}}"#,
        );
        assert_eq!(request.id.as_deref(), Some("7"));
        assert_eq!(request.method.as_deref(), Some("tools/call"));
        assert!(request.is_request());
        let late = s(r#"{"params":{"x":"\"id\":5"},"method":"initialize","id":"abc"}"#);
        assert_eq!(
            (late.id.as_deref(), late.method.as_deref()),
            (Some("\"abc\""), Some("initialize"))
        );
    }

    #[test]
    fn response_and_notification_are_told_apart() {
        let response = s(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#);
        assert!(response.is_response() && !response.is_request());
        let notification = s(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert!(!notification.is_request() && !notification.is_response());
        // Ответ клиента на запрос сервера — тоже ответ: в таблицу неотвеченных он не попадает.
        assert!(s(r#"{"id":"srv-1","result":{"roots":[]}}"#).is_response());
    }

    #[test]
    fn null_object_and_bool_ids_do_not_count() {
        for text in [
            r#"{"id":null,"method":"x"}"#,
            r#"{"id":{"a":1},"method":"x"}"#,
            r#"{"id":true,"method":"x"}"#,
        ] {
            assert_eq!(s(text).id, None, "{text}");
        }
    }

    #[test]
    fn a_cut_prefix_still_gives_an_id_that_precedes_the_cut() {
        let cut = br#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"text":"aaaa"#;
        assert_eq!(scan_prefix(cut).id.as_deref(), Some("42"));
        // Обычный разбор целой строки по такому обрубку ничего не даёт.
        assert_eq!(scan(cut), Scan::default());
        // id после обрыва достать нельзя.
        assert_eq!(scan_prefix(br#"{"params":{"text":"aaaa"#).id, None);
    }

    #[test]
    fn is_object_tells_json_objects_from_the_rest() {
        assert!(is_object(br#" {"a":1} "#));
        for text in [&b"{not json"[..], b"[1]", b"", b"plain", b"{\"a\":"] {
            assert!(!is_object(text), "{text:?}");
        }
    }

    #[test]
    fn garbage_never_panics_and_gives_nothing() {
        let samples: [&[u8]; 14] = [
            b"",
            b"{",
            b"}",
            b"{\"",
            b"{\"id\"",
            b"{\"id\":",
            b"{\"id\":1",
            b"[1,2]",
            b"\xff\xfe{\"id\":1}",
            b"{\"id\":\"\\",
            b"{\"a\":[[[[[[",
            b"{\"a\":}",
            b"null",
            b"{\"method\":\"a\\nb\",\"id\":1}",
        ];
        for sample in samples {
            let _ = scan(sample);
        }
        assert_eq!(scan(b"{\"id\":\"\\").id, None);
        assert_eq!(scan(b"\xff\xfe{\"id\":1}"), Scan::default());
        // method с escape-последовательностью не считается именем метода: такое сообщение — ответ по `id`.
        assert_eq!(scan(br#"{"method":"a\nb","id":1}"#).method, None);
        // Все префиксы настоящего сообщения тоже не вызывают паники.
        let full = br#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"search","arguments":{"query":"\"x\""}}}"#;
        for end in 0..full.len() {
            let _ = scan(&full[..end]);
        }
    }
}
