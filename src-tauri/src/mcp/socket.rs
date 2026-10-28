//! Сокет сервера MCP (ADR-0009): путь, проверки папки, проба старого сокета, `bind`, `chmod 0600`, предел кадра.
//! Это не сетевой порт: подключиться может только тот же пользователь, а он и так читает файл базы (§4.1).

use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::{UnixListener, UnixStream};

/// Предел одной строки JSON-RPC: 4 МиБ (ADR-0009).
pub const FRAME_LIMIT: usize = 4 * 1024 * 1024;
/// `sun_path` на macOS — 104 байта; запас под завершающий ноль и разночтения.
const MAX_PATH_BYTES: usize = 100;
const SOCKET_NAME: &str = "kinige.sock";

/// Почему сокет не поднят. Код уходит в окно «Доступ ИИ-агентов».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    /// Папка сокета — симлинк, чужая или с лишними правами.
    DirUnsafe,
    /// Старый сокет отвечает: на этих данных уже работает другой Kinige.
    SecondInstance,
    /// `bind` или `chmod` не удался, либо на месте сокета лежит чужой файл.
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

/// Путь сокета: `<каталог данных>/mcp/kinige.sock`; длиннее предела `sun_path` — `<tmp>/kinige-mcp/<хэш>.sock`.
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

/// FNV-1a: хэш должен совпадать между запусками (`DefaultHasher` этого не обещает).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Свой uid без `libc`: пара сокетов в процессе, `peer_cred` второго конца — это мы.
pub fn own_uid() -> io::Result<u32> {
    let (left, _right) = UnixStream::pair()?;
    Ok(left.peer_cred()?.uid())
}

/// Папка сокета: не симлинк, наша, права 0700. Нет папки — создаётся с правами 0700.
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

/// Поднимает слушатель: проверки папки, проба старого сокета (ответил — второй Kinige; нет — файл удаляется), `bind`,
/// `chmod 0600`. Не удался `chmod` — файл сокета удаляется. Чужой файл на месте сокета не трогается.
pub fn bind(path: &Path, uid: u32) -> Result<UnixListener, SocketError> {
    bind_with(path, uid, |socket| {
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
    })
}

/// То же с заданной установкой прав: тест подставляет неудачную, чтобы проверить удаление файла сокета.
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

/// Файл сокета на месте (проверка раз в 10 минут: macOS может вычищать старые файлы в `$TMPDIR`).
pub fn socket_present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
}

/// Читающая половина с пределом кадра: `AsyncRwTransport` из `rmcp` читает строку без границы (проверено в 4a.0), поэтому
/// предел ставится здесь. Строка длиннее [`FRAME_LIMIT`] — ошибка чтения, сервис завершается и соединение закрывается.
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
            for byte in &buf.filled()[before..] {
                if *byte == b'\n' {
                    run = 0;
                } else {
                    run += 1;
                    if run > limit {
                        // Ошибка чтения не оставляет данных в буфере (контракт `AsyncRead`).
                        buf.set_filled(before);
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "кадр длиннее предела",
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
        assert_eq!(fallback, socket_path(&long, tmp), "стабилен между вызовами");
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
