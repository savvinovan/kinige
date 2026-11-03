//! Reading lines (frames) with a limit: one JSON-RPC line is at most [`FRAME_LIMIT`] bytes (ADR-0009).

use std::io::{self, BufRead};

/// The limit of one frame: 4 MiB.
pub const FRAME_LIMIT: usize = 4 * 1024 * 1024;
/// How much of the beginning of a rejected line is kept to look for the `id`.
pub const PREFIX_KEEP: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// The line without the trailing newline is in the buffer.
    Line,
    /// The line is longer than the limit: read to the end and dropped; the buffer holds only its beginning (up to [`PREFIX_KEEP`] bytes).
    TooLong,
    /// The stream has ended.
    Eof,
}

/// Reads the next line into `buf` (it is cleared). Memory does not grow beyond `limit`: the tail of a long line is discarded.
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
            // End of stream: a last line without a newline is still a line.
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
            if buf.len().saturating_add(take) > limit {
                too_long = true;
                // Only the beginning of the line stays: the `id` for the error reply is looked up in it.
                buf.truncate(PREFIX_KEEP);
                buf.shrink_to(PREFIX_KEEP);
            } else {
                // `take` is not larger than the chunk length: the position of the newline or the whole length.
                buf.extend_from_slice(chunk.get(..take).unwrap_or_default());
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
        // Exactly the limit passes.
        assert_eq!(frames(&[b'y'; 10], 10)[0].0, Frame::Line);
        assert_eq!(frames(&[b'y'; 11], 10)[0].0, Frame::TooLong);
    }
}
