//! The MCP server socket (ADR-0009): the path, folder checks, a probe of an old socket, `bind`, `chmod 0600`, the frame limit.
//! It is not a network port: only the same user can connect, and that user can read the database file anyway (§4.1).

use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::{UnixListener, UnixStream};

/// The limit of one JSON-RPC line: 4 MiB (ADR-0009).
pub const FRAME_LIMIT: usize = 4 * 1024 * 1024;
/// `sun_path` on macOS is 104 bytes; a margin for the terminating zero and discrepancies.
const MAX_PATH_BYTES: usize = 100;
const SOCKET_NAME: &str = "kinige.sock";

/// Why the socket is not up. The code goes to the "AI agent access" window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    /// The socket folder is a symlink, foreign or has excess permissions.
    DirUnsafe,
    /// The old socket answers: another Kinige is already working on this data.
    SecondInstance,
    /// `bind` or `chmod` failed, or a foreign file lies in place of the socket.
    BindFailed,
}

impl SocketError {
    pub fn code(self) -> &'static str {
        match self {
            SocketError::DirUnsafe => "dir_unsafe",
            SocketError::SecondInstance => "second_instance",
            SocketError::BindFailed => "bind_failed",
        }
    }
}

/// The socket path: `<data directory>/mcp/kinige.sock`; longer than the `sun_path` limit -- `<tmp>/kinige-mcp/<hash>.sock`.
pub fn socket_path(data_dir: &Path, tmp_dir: &Path) -> PathBuf {
    let primary = data_dir.join("mcp").join(SOCKET_NAME);
    if primary.as_os_str().len() <= MAX_PATH_BYTES {
        return primary;
    }
    tmp_dir.join("kinige-mcp").join(format!(
        "{:016x}.sock",
        fnv1a(data_dir.as_os_str().as_encoded_bytes())
    ))
}

/// FNV-1a: the hash must be the same between launches (`DefaultHasher` does not promise that).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Our own uid without `libc`: a socket pair in the process, the `peer_cred` of the other end is us.
pub fn own_uid() -> io::Result<u32> {
    let (left, _right) = UnixStream::pair()?;
    Ok(left.peer_cred()?.uid())
}

/// The socket folder: not a symlink, ours, permissions 0700. If there is no folder, it is created with permissions 0700.
fn prepare_dir(dir: &Path, uid: u32) -> Result<(), SocketError> {
    if std::fs::symlink_metadata(dir).is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| SocketError::BindFailed)?;
    }
    let meta = std::fs::symlink_metadata(dir).map_err(|_| SocketError::DirUnsafe)?;
    let safe = meta.file_type().is_dir()
        && !meta.file_type().is_symlink()
        && meta.uid() == uid
        && meta.mode() & 0o777 == 0o700;
    if safe {
        Ok(())
    } else {
        Err(SocketError::DirUnsafe)
    }
}

/// Brings the listener up: folder checks, a probe of the old socket (it answered -- a second Kinige; it did not -- the file is removed), `bind`,
/// `chmod 0600`. If `chmod` failed, the socket file is removed. A foreign file in place of the socket is not touched.
pub fn bind(path: &Path, uid: u32) -> Result<UnixListener, SocketError> {
    bind_with(path, uid, |socket| {
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
    })
}

/// The same with a given permission setter: a test substitutes a failing one to check the removal of the socket file.
pub fn bind_with(
    path: &Path,
    uid: u32,
    chmod: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<UnixListener, SocketError> {
    let dir = path.parent().ok_or(SocketError::BindFailed)?;
    prepare_dir(dir, uid)?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return Err(SocketError::SecondInstance);
            }
            std::fs::remove_file(path).map_err(|_| SocketError::BindFailed)?;
        }
        Ok(_) => return Err(SocketError::BindFailed),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(SocketError::BindFailed),
    }
    let listener = UnixListener::bind(path).map_err(|_| SocketError::BindFailed)?;
    if chmod(path).is_err() {
        let _ = std::fs::remove_file(path);
        return Err(SocketError::BindFailed);
    }
    Ok(listener)
}

/// The socket file is in place (checked every 10 minutes: macOS may clean old files in `$TMPDIR`).
pub fn socket_present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
}

/// The reading half with a frame limit: `AsyncRwTransport` from `rmcp` reads a line with no bound (checked in 4a.0), so
/// the limit is set here. A line longer than [`FRAME_LIMIT`] is a read error, the service ends and the connection closes.
pub struct Limited<R> {
    inner: R,
    since_newline: usize,
    limit: usize,
}

impl<R> Limited<R> {
    pub fn new(inner: R, limit: usize) -> Limited<R> {
        Limited {
            inner,
            since_newline: 0,
            limit,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Limited<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &polled {
            let limit = self.limit;
            let mut run = self.since_newline;
            for byte in buf.filled().get(before..).unwrap_or_default() {
                if *byte == b'\n' {
                    run = 0;
                } else {
                    run += 1;
                    if run > limit {
                        // A read error leaves no data in the buffer (the `AsyncRead` contract).
                        buf.set_filled(before);
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "frame is longer than the limit",
                        )));
                    }
                }
            }
            self.since_newline = run;
        }
        polled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn short_data_dir_keeps_the_socket_inside_it_and_a_long_one_falls_back_to_tmp() {
        let tmp = Path::new("/var/folders/ab/T");
        let short = Path::new("/Users/me/Library/Application Support/me.savvinov.kinige");
        assert_eq!(
            socket_path(short, tmp),
            short.join("mcp").join("kinige.sock")
        );
        let long = PathBuf::from(format!("/Users/me/{}", "очень-длинная-папка/".repeat(8)));
        let fallback = socket_path(&long, tmp);
        assert!(fallback.starts_with("/var/folders/ab/T/kinige-mcp"));
        assert!(fallback.as_os_str().len() <= MAX_PATH_BYTES);
        assert_eq!(fallback, socket_path(&long, tmp), "stable between calls");
        assert_ne!(fallback, socket_path(&long.join("другая"), tmp));
    }

    #[tokio::test]
    async fn limited_reader_passes_long_data_split_into_lines_and_rejects_one_long_line() {
        let ok = format!("{}\n{}\n", "a".repeat(10), "b".repeat(10));
        let mut reader = Limited::new(ok.as_bytes(), 10);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await.unwrap();
        assert_eq!(out, ok.as_bytes());
        let too_long = "c".repeat(11);
        let mut reader = Limited::new(too_long.as_bytes(), 10);
        let error = reader.read_to_end(&mut Vec::new()).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
