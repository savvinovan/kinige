//! Чтение строк (кадров) с пределом: одна строка JSON-RPC — не больше [`FRAME_LIMIT`] байт (ADR-0009).

use std::io::{self, BufRead};

/// Предел одного кадра: 4 МиБ.
pub const FRAME_LIMIT: usize = 4 * 1024 * 1024;
/// Сколько начала отвергнутой строки сохраняется для поиска `id`.
pub const PREFIX_KEEP: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// Строка без завершающего перевода строки лежит в буфере.
    Line,
    /// Строка длиннее предела: прочитана до конца и выброшена; в буфере — только её начало (до [`PREFIX_KEEP`] байт).
    TooLong,
    /// Поток закончился.
    Eof,
}

/// Читает следующую строку в `buf` (он очищается). Память не растёт сверх `limit`: хвост длинной строки отбрасывается.
pub fn read_frame<R: BufRead>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    limit: usize,
) -> io::Result<Frame> {
    buf.clear();
    let mut too_long = false;
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if chunk.is_empty() {
            // Конец потока: последняя строка без перевода строки тоже строка.
            return Ok(match (too_long, buf.is_empty()) {
                (true, _) => Frame::TooLong,
                (false, true) => Frame::Eof,
                (false, false) => Frame::Line,
            });
        }
        let (take, found) = match chunk.iter().position(|b| *b == b'\n') {
            Some(at) => (at, true),
            None => (chunk.len(), false),
        };
        if !too_long {
            if buf.len() + take > limit {
                too_long = true;
                // Остаётся только начало строки: по нему ищется `id` для ответа-ошибки.
                buf.truncate(PREFIX_KEEP);
                buf.shrink_to(PREFIX_KEEP);
            } else {
                buf.extend_from_slice(&chunk[..take]);
            }
        }
        reader.consume(take + usize::from(found));
        if found {
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(if too_long {
                Frame::TooLong
            } else {
                Frame::Line
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frames(input: &[u8], limit: usize) -> Vec<(Frame, Vec<u8>)> {
        let mut reader = io::BufReader::with_capacity(7, Cursor::new(input.to_vec()));
        let mut out = Vec::new();
        let mut buf = Vec::new();
        loop {
            let frame = read_frame(&mut reader, &mut buf, limit).unwrap();
            if frame == Frame::Eof {
                return out;
            }
            out.push((frame, buf.clone()));
        }
    }

    #[test]
    fn splits_lines_across_small_reads_and_keeps_a_last_line_without_newline() {
        let got = frames(b"{\"a\":1}\r\n\n{\"b\":2}\nlast", 100);
        let lines: Vec<&[u8]> = got.iter().map(|(_, line)| line.as_slice()).collect();
        assert_eq!(lines, [&b"{\"a\":1}"[..], b"", b"{\"b\":2}", b"last"]);
    }

    #[test]
    fn a_line_over_the_limit_is_dropped_and_the_next_one_survives() {
        let mut input = vec![b'x'; 50];
        input.extend_from_slice(b"\nok\n");
        let got = frames(&input, 10);
        assert_eq!(got[0].0, Frame::TooLong);
        assert!(got[0].1.len() <= 10 && got[0].1.iter().all(|b| *b == b'x'));
        assert_eq!(got[1], (Frame::Line, b"ok".to_vec()));
        // Ровно предел проходит.
        assert_eq!(frames(&[b'y'; 10], 10)[0].0, Frame::Line);
        assert_eq!(frames(&[b'y'; 11], 10)[0].0, Frame::TooLong);
    }
}
