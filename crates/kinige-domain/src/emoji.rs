//! Проверка «это последовательность эмодзи» для списка «недавних» (слайс 3). Без внешних данных: правила UTS #51 (ED-20,
//! `emoji sequence`) для одного графемного кластера, ограниченные тем, что выдаёт пикер: символ эмодзи, при желании с
//! `VS16` или тоном кожи; флаг из двух региональных индикаторов; клавиша-кейкап; тег-флаг подобласти (🏴 + теги); цепочка
//! элементов через ZWJ.
//!
//! Таблица `Extended_Pictographic` — из `emoji-data.txt` (Unicode Emoji 16.0, диапазоны неназначенных кодов внутри блоков
//! эмодзи включены намеренно, как в самом файле: так будущие эмодзи проходят без обновления). Список написан вручную, а не
//! подключён из крейта: зависимость ради одной проверки не нужна. Таблица записана по памяти, поэтому сверена со всеми
//! последовательностями `emojibase-data` 17.0.0 (en, ru, с вариантами тонов кожи): отвергаются только «компоненты», которых
//! пикер не выдаёт, — одиночные региональные индикаторы и одиночные тона кожи (разовая проверка при разработке, в
//! репозиторий не входит).

/// Самая длинная последовательность в байтах: как у эмодзи чата.
pub const MAX_EMOJI_BYTES: usize = crate::chat::MAX_ICON_BYTES;

const ZWJ: char = '\u{200D}';
const VS16: char = '\u{FE0F}';
const KEYCAP: char = '\u{20E3}';
const TAG_FIRST: char = '\u{E0020}';
const TAG_LAST: char = '\u{E007E}';
const TAG_CANCEL: char = '\u{E007F}';
const BLACK_FLAG: char = '\u{1F3F4}';

/// Диапазоны `Extended_Pictographic` (включительно).
const EXTENDED_PICTOGRAPHIC: &[(u32, u32)] = &[
    (0x00A9, 0x00A9),
    (0x00AE, 0x00AE),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x2122, 0x2122),
    (0x2139, 0x2139),
    (0x2194, 0x2199),
    (0x21A9, 0x21AA),
    (0x231A, 0x231B),
    (0x2328, 0x2328),
    (0x2388, 0x2388),
    (0x23CF, 0x23CF),
    (0x23E9, 0x23F3),
    (0x23F8, 0x23FA),
    (0x24C2, 0x24C2),
    (0x25AA, 0x25AB),
    (0x25B6, 0x25B6),
    (0x25C0, 0x25C0),
    (0x25FB, 0x25FE),
    (0x2600, 0x2605),
    (0x2607, 0x2612),
    (0x2614, 0x2685),
    (0x2690, 0x2705),
    (0x2708, 0x2712),
    (0x2714, 0x2714),
    (0x2716, 0x2716),
    (0x271D, 0x271D),
    (0x2721, 0x2721),
    (0x2728, 0x2728),
    (0x2733, 0x2734),
    (0x2744, 0x2744),
    (0x2747, 0x2747),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2763, 0x2767),
    (0x2795, 0x2797),
    (0x27A1, 0x27A1),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2934, 0x2935),
    (0x2B05, 0x2B07),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0x3297, 0x3297),
    (0x3299, 0x3299),
    (0x1F000, 0x1F0FF),
    (0x1F10D, 0x1F10F),
    (0x1F12F, 0x1F12F),
    (0x1F16C, 0x1F171),
    (0x1F17E, 0x1F17F),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F1AD, 0x1F1E5),
    (0x1F201, 0x1F20F),
    (0x1F21A, 0x1F21A),
    (0x1F22F, 0x1F22F),
    (0x1F232, 0x1F23A),
    (0x1F23C, 0x1F23F),
    (0x1F249, 0x1F3FA),
    (0x1F400, 0x1F53D),
    (0x1F546, 0x1F64F),
    (0x1F680, 0x1F6FF),
    (0x1F774, 0x1F77F),
    (0x1F7D5, 0x1F7FF),
    (0x1F80C, 0x1F80F),
    (0x1F848, 0x1F84F),
    (0x1F85A, 0x1F85F),
    (0x1F888, 0x1F88F),
    (0x1F8AE, 0x1F8FF),
    (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945),
    (0x1F947, 0x1FAFF),
    (0x1FC00, 0x1FFFD),
];

fn is_extended_pictographic(c: char) -> bool {
    let code = c as u32;
    EXTENDED_PICTOGRAPHIC
        .binary_search_by(|&(first, last)| {
            if last < code {
                std::cmp::Ordering::Less
            } else if first > code {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn is_regional_indicator(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// `Emoji_Modifier`: тона кожи (Фицпатрик).
fn is_skin_tone(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}

/// Элемент цепочки: символ `Extended_Pictographic`, затем не более одного `VS16` или тона кожи. Возвращает, сколько
/// символов съедено (0 — элемента нет).
fn element_len(chars: &[char]) -> usize {
    match chars {
        [first, rest @ ..] if is_extended_pictographic(*first) => match rest.first() {
            Some(&c) if c == VS16 || is_skin_tone(c) => 2,
            _ => 1,
        },
        _ => 0,
    }
}

/// Строка — ровно одна последовательность эмодзи: пустая, с пробелами, с управляющими знаками, разметкой, текстом или
/// длиннее [`MAX_EMOJI_BYTES`] — нет.
pub fn is_emoji_sequence(text: &str) -> bool {
    if text.is_empty() || text.len() > MAX_EMOJI_BYTES {
        return false;
    }
    let chars: Vec<char> = text.chars().collect();
    match chars.as_slice() {
        // Кейкап: цифра, `#` или `*`, необязательный VS16, U+20E3.
        [key, rest @ ..] if key.is_ascii_digit() || *key == '#' || *key == '*' => {
            matches!(rest, [KEYCAP] | [VS16, KEYCAP])
        }
        // Флаг страны: два региональных индикатора.
        [a, b] if is_regional_indicator(*a) && is_regional_indicator(*b) => true,
        // Флаг подобласти: чёрный флаг, теги, отмена тегов.
        [flag, tags @ .., cancel] if *flag == BLACK_FLAG && *cancel == TAG_CANCEL => {
            !tags.is_empty() && tags.iter().all(|c| (TAG_FIRST..=TAG_LAST).contains(c))
        }
        // Цепочка элементов через ZWJ.
        all => {
            let mut rest = all;
            loop {
                let used = element_len(rest);
                if used == 0 {
                    return false;
                }
                rest = &rest[used..];
                match rest {
                    [] => return true,
                    [joiner, tail @ ..] if *joiner == ZWJ => rest = tail,
                    _ => return false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_without_overlaps() {
        for pair in EXTENDED_PICTOGRAPHIC.windows(2) {
            assert!(pair[0].1 < pair[1].0, "{pair:?}");
        }
        assert!(EXTENDED_PICTOGRAPHIC.iter().all(|(a, b)| a <= b));
    }

    #[test]
    fn emoji_of_every_kind_are_accepted() {
        for emoji in [
            "📚",
            "💬",
            "❤",
            "❤\u{FE0F}",
            "©",
            "☺\u{FE0F}",
            "👍\u{1F3FD}", // тон кожи
            "🇷🇺",          // флаг страны
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}", // 🏴 Шотландия
            "1\u{FE0F}\u{20E3}", // кейкап
            "#\u{20E3}",
            "👨\u{200D}👩\u{200D}👧\u{200D}👦", // семья
            "🏳\u{FE0F}\u{200D}🌈",              // радужный флаг
            "👩\u{1F3FB}\u{200D}❤\u{FE0F}\u{200D}👨\u{1F3FF}", // пара с разными тонами
            "🫩",                                // эмодзи новых версий внутри блока
        ] {
            assert!(is_emoji_sequence(emoji), "{emoji:?}");
        }
    }

    #[test]
    fn everything_else_is_refused() {
        let long = "📚".repeat(9); // 36 байт
        for bad in [
            "",
            " ",
            "📚 📚",
            "📚 ",
            "abc",
            "a",
            "1",
            "<b>abc",
            "<b>📚",
            "📚a",
            "a📚",
            "📚📚", // два эмодзи — не одна последовательность
            "\n",
            "📚\u{0007}",
            "\u{200D}",
            "📚\u{200D}",
            "\u{200D}📚",
            "📚\u{200D}\u{200D}📚",
            "\u{1F3FB}", // тон кожи без основы
            "\u{FE0F}",
            "🇷",
            "🇷🇺🇷",
            "\u{1F3F4}\u{E007F}",
            "\u{1F3F4}\u{E0067}",
            "1\u{20E3}\u{20E3}",
            "\u{20E3}",
            long.as_str(),
            "\u{E0067}",
            "\u{FFFD}",
        ] {
            assert!(!is_emoji_sequence(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_length_limit_fits_the_longest_picker_emoji_and_nothing_longer() {
        // 👨‍👩‍👧‍👦: четыре символа и три ZWJ — 25 байт.
        let family = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
        assert_eq!(family.len(), 25);
        assert!(is_emoji_sequence(family));
        // Самая длинная последовательность пикера — поцелуй с разными тонами кожи, 35 байт.
        let kiss = "👩\u{1F3FB}\u{200D}\u{2764}\u{FE0F}\u{200D}💋\u{200D}👨\u{1F3FF}";
        assert_eq!(kiss.len(), 35);
        assert!(is_emoji_sequence(kiss));
        // Та же цепочка семьи с тонами у всех — 41 байт: уже за пределом.
        let too_long = "👨\u{1F3FB}\u{200D}👩\u{1F3FB}\u{200D}👧\u{1F3FB}\u{200D}👦\u{1F3FB}";
        assert_eq!(too_long.len(), 41);
        assert!(too_long.len() > MAX_EMOJI_BYTES);
        assert!(!is_emoji_sequence(too_long));
    }
}
