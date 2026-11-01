//! Канонический JSON по RFC 8785 (JCS) для групп полей (ADR-0001 §3.3).
//!
//! В группах только строки, булевы, `null`, объекты, массивы и целые в пределах ±2⁵³, поэтому правила JCS сводятся к
//! сортировке ключей по кодовым единицам UTF-16, минимальному экранированию строк и отсутствию пробелов. Числа с дробной
//! частью и вне ±2⁵³ не допускаются: их запись в JCS (ECMAScript) — отдельная задача, которой в группах нет.

use serde_json::Value;
use std::fmt;

/// Наибольшее безопасное целое: ±2⁵³ (ADR-0001 §3.3).
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_992;

/// Предел вложенности равен пределу разбора serde_json (`remaining_depth` = 128 в `de.rs`): что записали — то прочитаем.
/// Счёт у serde_json такой: контейнер, при входе в который остаток дошёл до 0, — ошибка, поэтому читается не больше
/// `MAX_DEPTH - 1` = 127 вложенных массивов и объектов, а 128-й — `TooDeep`. Значение глубже не пишется, и рекурсия записи
/// не переполняет стек от чужих данных (ADR-0001 §A: глубина значения группы ≤ 127 контейнеров, глубже — `malformed`).
pub const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JcsError {
    /// Число с дробной частью или вне ±2⁵³.
    UnsupportedNumber,
    /// Вложенность глубже [`MAX_DEPTH`].
    TooDeep,
}

impl fmt::Display for JcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JcsError::UnsupportedNumber => {
                f.write_str("в группе допустимы только целые числа в пределах ±2^53")
            }
            JcsError::TooDeep => f.write_str("в группе слишком глубокая вложенность"),
        }
    }
}

impl std::error::Error for JcsError {}

/// Каноническая запись значения группы. Пишется сразу в `String`, поэтому UTF-8 гарантирован типом, а не проверкой.
pub fn canonical_string(value: &Value) -> Result<String, JcsError> {
    let mut out = String::new();
    write_value(value, &mut out, 0)?;
    Ok(out)
}

/// Канонические байты значения группы (те же, что у [`canonical_string`]).
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, JcsError> {
    canonical_string(value).map(String::into_bytes)
}

/// `depth` — сколько контейнеров уже открыто вокруг `value`.
fn write_value(value: &Value, out: &mut String, depth: usize) -> Result<(), JcsError> {
    if matches!(value, Value::Array(_) | Value::Object(_)) && depth + 1 >= MAX_DEPTH {
        return Err(JcsError::TooDeep);
    }
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => {
            let integer = number
                .as_i64()
                .filter(|n| n.unsigned_abs() <= MAX_SAFE_INTEGER as u64)
                .ok_or(JcsError::UnsupportedNumber)?;
            out.push_str(&integer.to_string());
        }
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out, depth + 1)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            // Порядок ключей — по кодовым единицам UTF-16, а не по байтам UTF-8 (они расходятся за пределами BMP).
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(item, out, depth + 1)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Минимальное экранирование: `"` и `\`, управляющие символы короткими формами `\b \t \n \f \r`, остальные — `\u00xx`
/// строчными буквами; всё прочее (в том числе U+007F и символы вне ASCII) — как есть, в UTF-8.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn canon(value: Value) -> String {
        String::from_utf8(canonical_bytes(&value).unwrap()).unwrap()
    }

    #[test]
    fn keys_are_sorted_by_utf16_code_units_rfc8785_section_3_2_3() {
        // Пример из RFC 8785, п. 3.2.3: порядок — U+000D, "1", U+0080, "ö", "€", U+1F600 (суррогатная пара D83D DE00), U+FB33.
        // По байтам UTF-8 U+FB33 шёл бы раньше U+1F600; по UTF-16 — позже.
        let value = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\u{1f600}": "Emoji: Grinning Face",
            "\u{80}": "Control",
            "\u{f6}": "Latin Small Letter O With Diaeresis"
        });
        let text = canon(value);
        let positions: Vec<usize> = [
            "\\r",
            "1",
            "\u{80}",
            "\u{f6}",
            "\u{20ac}",
            "\u{1f600}",
            "\u{fb33}",
        ]
        .iter()
        .map(|key| {
            text.find(&format!("\"{key}\":"))
                .unwrap_or_else(|| panic!("нет ключа {key:?} в {text:?}"))
        })
        .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{text:?}");
    }

    #[test]
    fn no_whitespace_and_nested_objects_are_sorted() {
        assert_eq!(
            canon(json!({"b": [1, 2, {"z": null, "a": true}], "a": false})),
            r#"{"a":false,"b":[1,2,{"a":true,"z":null}]}"#
        );
    }

    #[test]
    fn string_escaping_is_minimal() {
        assert_eq!(canon(json!("a\"b\\c")), r#""a\"b\\c""#);
        assert_eq!(canon(json!("\u{8}\t\n\u{c}\r")), r#""\b\t\n\f\r""#);
        assert_eq!(canon(json!("\u{0}\u{1f}")), r#""\u0000\u001f""#);
        // Не экранируются: «/», DEL, символы вне ASCII, эмодзи.
        assert_eq!(canon(json!("/\u{7f}Привет😀")), "\"/\u{7f}Привет😀\"");
    }

    #[test]
    fn integers_are_written_plainly() {
        assert_eq!(
            canon(json!([
                0,
                -1,
                1_760_000_000_000_i64,
                9_007_199_254_740_992_i64,
                -9_007_199_254_740_992_i64
            ])),
            "[0,-1,1760000000000,9007199254740992,-9007199254740992]"
        );
    }

    #[test]
    fn floats_and_huge_integers_are_rejected() {
        assert_eq!(
            canonical_bytes(&json!(1.5)),
            Err(JcsError::UnsupportedNumber)
        );
        assert_eq!(
            canonical_bytes(&json!(9_007_199_254_740_993_i64)),
            Err(JcsError::UnsupportedNumber)
        );
        assert_eq!(
            canonical_bytes(&json!(u64::MAX)),
            Err(JcsError::UnsupportedNumber)
        );
    }

    #[test]
    fn nesting_beyond_the_limit_is_an_error_not_a_stack_overflow() {
        let nested = |levels: usize| {
            let mut value = json!(1);
            for _ in 0..levels {
                value = Value::Array(vec![value]);
            }
            value
        };
        assert!(canonical_string(&nested(MAX_DEPTH - 1)).is_ok());
        assert_eq!(canonical_string(&nested(MAX_DEPTH)), Err(JcsError::TooDeep));
        assert_eq!(
            canonical_string(&nested(MAX_DEPTH + 50)),
            Err(JcsError::TooDeep)
        );
    }

    #[test]
    fn what_is_written_is_read_back_by_serde_json_at_every_depth() {
        // Предел совпадает с пределом разбора serde_json: записанное читается, а не записанное и не читалось бы.
        for levels in (MAX_DEPTH - 5)..=(MAX_DEPTH + 5) {
            let arrays = format!("{}1{}", "[".repeat(levels), "]".repeat(levels));
            let objects = format!("{}1{}", "{\"a\":".repeat(levels), "}".repeat(levels));
            for text in [arrays, objects] {
                let parsed: Result<Value, _> = serde_json::from_str(&text);
                let written = parsed.as_ref().ok().map(canonical_string);
                match written {
                    Some(result) => assert_eq!(result.as_deref(), Ok(text.as_str()), "{levels}"),
                    None => assert!(
                        levels >= MAX_DEPTH,
                        "{levels}: serde_json отказал раньше предела"
                    ),
                }
            }
        }
    }

    #[test]
    fn output_is_independent_of_insertion_order() {
        let a = json!({"x": 1, "y": 2});
        let b: Value = serde_json::from_str(r#"{"y":2,"x":1}"#).unwrap();
        assert_eq!(canonical_bytes(&a).unwrap(), canonical_bytes(&b).unwrap());
    }
}
