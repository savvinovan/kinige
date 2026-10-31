//! Файл настроек на диске (ADR-0010): безопасное чтение по дескриптору, признак для опроса, атомарная запись, уборка
//! брошенных временных файлов. Все проверки владельца и прав — только под `cfg(unix)`.

use super::parse::MAX_BYTES;
use kinige_domain::UnsafeReason;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

pub const FILE_NAME: &str = "settings.json";
const TMP_PREFIX: &str = ".settings.json.";
const TMP_SUFFIX: &str = ".tmp";

/// Признак изменения файла для опроса раз в секунду: любое отличие — повод перечитать.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sig {
    mtime_ns: u128,
    len: u64,
    mode: u32,
    inode: u64,
    symlink: bool,
    /// У ссылки — время изменения и размер цели: правка цели опросом видна (сама ссылка при этом не меняется).
    target: Option<(u128, u64)>,
}

/// Признак файла по пути (без перехода по ссылке); `None` — файла нет или он не виден.
pub fn signature(path: &Path) -> Option<Sig> {
    let meta = fs::symlink_metadata(path).ok()?;
    Some(Sig {
        mtime_ns: nanos(&meta),
        len: meta.len(),
        mode: mode_of(&meta),
        inode: inode_of(&meta),
        symlink: meta.file_type().is_symlink(),
        target: meta
            .file_type()
            .is_symlink()
            .then(|| fs::metadata(path).ok())
            .flatten()
            .map(|target| (nanos(&target), target.len())),
    })
}

fn nanos(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(unix)]
fn mode_of(meta: &fs::Metadata) -> u32 {
    std::os::unix::fs::MetadataExt::mode(meta)
}
#[cfg(not(unix))]
fn mode_of(_: &fs::Metadata) -> u32 {
    0
}
#[cfg(unix)]
fn inode_of(meta: &fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::ino(meta)
}
#[cfg(not(unix))]
fn inode_of(_: &fs::Metadata) -> u64 {
    0
}

/// Что нашлось на диске.
#[derive(Debug)]
pub enum ReadOutcome {
    Missing,
    /// Не читается или слишком велик (причина — в ошибке файла).
    Failed(super::parse::FileError),
    /// Не обычный файл: содержимое не читалось.
    NotRegular,
    /// Содержимое и вывод проверок. `unsafe_reason` — первая найденная причина.
    Content {
        bytes: Vec<u8>,
        unsafe_reason: Option<UnsafeReason>,
        sig: Option<Sig>,
    },
}

/// Читает файл настроек. Файл открывается один раз (`O_NOFOLLOW | O_NONBLOCK`), владелец, права и тип проверяются по
/// дескриптору, а не по пути: между проверкой и чтением файл подменить нельзя. Ссылка читается по цели, но помечается
/// небезопасной; канал и устройство не читаются вовсе. Читается не больше `MAX_BYTES + 1` байт.
pub fn read_file(path: &Path, dir: &Path) -> ReadOutcome {
    read_file_with(path, dir, &|| ())
}

/// То же с шагом `between` после чтения и до снятия признака: в нём тесты подменяют файл. Признак снимается до `open` и
/// после чтения; не совпали — файл менялся во время чтения, признак пустой, и следующий опрос прочитает файл заново.
#[doc(hidden)]
pub fn read_file_with(path: &Path, dir: &Path, between: &dyn Fn()) -> ReadOutcome {
    let sig_before = signature(path);
    let (file, via_symlink) = match open(path) {
        Ok(opened) => opened,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return ReadOutcome::Missing,
        Err(error) => return ReadOutcome::Failed(super::parse::FileError::Io(error.kind())),
    };
    let Ok(meta) = file.metadata() else {
        return ReadOutcome::Failed(super::parse::FileError::Io(io::ErrorKind::Other));
    };
    if !meta.is_file() {
        return ReadOutcome::NotRegular;
    }
    if meta.len() > MAX_BYTES {
        return ReadOutcome::Failed(super::parse::FileError::TooLarge);
    }
    let mut bytes = Vec::new();
    if let Err(error) = file.take(MAX_BYTES + 1).read_to_end(&mut bytes) {
        return ReadOutcome::Failed(super::parse::FileError::Io(error.kind()));
    }
    if bytes.len() as u64 > MAX_BYTES {
        return ReadOutcome::Failed(super::parse::FileError::TooLarge);
    }
    let unsafe_reason = if via_symlink {
        Some(UnsafeReason::Symlink)
    } else {
        unsafe_file(&meta).or_else(|| unsafe_directory(dir))
    };
    between();
    let sig_after = signature(path);
    ReadOutcome::Content {
        bytes,
        unsafe_reason,
        sig: if sig_before == sig_after {
            sig_after
        } else {
            None
        },
    }
}

#[cfg(unix)]
fn open(path: &Path) -> io::Result<(File, bool)> {
    use std::os::unix::fs::OpenOptionsExt;
    let flags = libc::O_NONBLOCK;
    match OpenOptions::new()
        .read(true)
        .custom_flags(flags | libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => Ok((file, false)),
        // Ссылка: `O_NOFOLLOW` даёт ELOOP. Читаем по цели (без доверия к ней) и помечаем.
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(path)
            .map(|file| (file, true)),
        Err(error) => Err(error),
    }
}
#[cfg(not(unix))]
fn open(path: &Path) -> io::Result<(File, bool)> {
    File::open(path).map(|file| (file, false))
}

#[cfg(unix)]
fn unsafe_file(meta: &fs::Metadata) -> Option<UnsafeReason> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: `geteuid` ничего не читает и не пишет, ошибок не возвращает.
    if meta.uid() != unsafe { libc::geteuid() } {
        return Some(UnsafeReason::Owner);
    }
    (meta.mode() & 0o022 != 0).then_some(UnsafeReason::Permissions)
}
#[cfg(not(unix))]
fn unsafe_file(_: &fs::Metadata) -> Option<UnsafeReason> {
    None
}

/// Каталог данных: не ссылка, свой, не доступен на запись группе и всем.
#[cfg(unix)]
pub fn unsafe_directory(dir: &Path) -> Option<UnsafeReason> {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return Some(UnsafeReason::Directory);
    };
    // SAFETY: см. `unsafe_file`.
    let own = meta.uid() == unsafe { libc::geteuid() };
    (!meta.is_dir() || !own || meta.mode() & 0o022 != 0).then_some(UnsafeReason::Directory)
}
#[cfg(not(unix))]
pub fn unsafe_directory(_: &Path) -> Option<UnsafeReason> {
    None
}

/// Атомарная запись: временный файл рядом (`O_EXCL`, 0600), `fsync`, `rename`, `fsync` каталога. Читатель видит файл
/// целиком старым или целиком новым.
pub fn write_atomic(path: &Path, dir: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = dir.join(format!(
        "{TMP_PREFIX}{}{TMP_SUFFIX}",
        Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = create_private(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        if let Ok(directory) = File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(unix)]
fn create_private(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
#[cfg(not(unix))]
fn create_private(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// Убирает брошенные временные файлы (обрыв между созданием и `rename`).
pub fn remove_stale_temp(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(TMP_PREFIX) && name.ends_with(TMP_SUFFIX) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Путь файла настроек в каталоге данных.
pub fn file_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}
