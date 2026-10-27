//! Фоновая Markdown-копия (ТЗ §2.1): после фиксации записи отдельный поток целиком переписывает файл затронутого дня
//! `<папка>/<чат>/YYYY/YYYY-MM-DD.md` (временный файл + переименование). День без записей — файл удаляется.
//! Отметки (`mirror_dirty`) лежат в базе, поэтому переживают убийство процесса: оставшиеся обрабатываются при старте.
//! Корзина в копию не попадает (ТЗ §16).

use crate::settings::effective_mirror_dir;
use crate::time::local_hhmm;
use crate::writer::Writer;
use kinige_domain::MirrorError;
use kinige_domain::mirror::{MirrorEntry, day_file_segments, render_day_file};
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

/// Общее состояние между хранилищем и фоновым потоком.
pub(crate) struct MirrorShared {
    wake: Mutex<bool>,
    signal: Condvar,
    stopped: AtomicBool,
    error: Mutex<Option<MirrorError>>,
    retry: Duration,
}

impl MirrorShared {
    pub(crate) fn new(retry: Duration) -> Arc<MirrorShared> {
        Arc::new(MirrorShared {
            wake: Mutex::new(true), // первый проход — сразу при старте
            signal: Condvar::new(),
            stopped: AtomicBool::new(false),
            error: Mutex::new(None),
            retry,
        })
    }

    pub(crate) fn wake(&self) {
        if let Ok(mut flag) = self.wake.lock() {
            *flag = true;
            self.signal.notify_all();
        }
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub(crate) fn error(&self) -> Option<MirrorError> {
        self.error.lock().ok().and_then(|e| *e)
    }

    pub(crate) fn clear_error(&self) {
        self.set_error(None);
    }

    fn set_error(&self, error: Option<MirrorError>) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
    }

    /// Ждёт сигнала; после ошибки или с остатком отметок — не дольше `retry`.
    fn wait(&self, retry_soon: bool) {
        let Ok(mut flag) = self.wake.lock() else {
            return;
        };
        if !*flag {
            let timeout = if retry_soon {
                self.retry
            } else {
                Duration::from_secs(3600)
            };
            flag = match self.signal.wait_timeout(flag, timeout) {
                Ok((flag, _)) => flag,
                Err(_) => return,
            };
        }
        *flag = false;
    }
}

pub(crate) fn spawn(
    shared: Arc<MirrorShared>,
    reader: Connection,
    writer: Writer,
    default_dir: Option<PathBuf>,
) {
    thread::Builder::new()
        .name("kinige-mirror".into())
        .spawn(move || {
            loop {
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                let retry_soon = pass(&shared, &reader, &writer, default_dir.as_deref());
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                shared.wait(retry_soon);
            }
        })
        .expect("не удалось запустить поток Markdown-копии");
}

/// Один проход по отметкам. Возвращает `true`, если нужен повтор (ошибка или остались отметки).
fn pass(
    shared: &MirrorShared,
    reader: &Connection,
    writer: &Writer,
    default_dir: Option<&Path>,
) -> bool {
    let dir = match effective_mirror_dir(reader, default_dir) {
        Ok((Some(dir), _)) => dir,
        // Папки нет (не выбрана и по умолчанию её нет): копия выключена, отметки ждут.
        Ok((None, _)) => {
            shared.set_error(None);
            return false;
        }
        Err(_) => {
            eprintln!("kinige: копия: не удалось прочитать настройку папки, повтор позже");
            return true;
        }
    };
    // Пока писать нечего, папки не касаемся: на macOS первое обращение к «Документам» вызывает запрос доступа,
    // и он должен появиться вместе с первой записью, а не при запуске пустого приложения.
    match dirty_batch(reader) {
        Ok(batch) if batch.is_empty() => {
            shared.set_error(None);
            return false;
        }
        Ok(_) => {}
        Err(_) => {
            // Не «писать нечего»: чтение отметок не удалось, отметки целы — повтор по таймеру.
            eprintln!("kinige: копия: не удалось прочитать отметки дней, повтор позже");
            return true;
        }
    }
    if !prepare_root(&dir) {
        shared.set_error(Some(MirrorError::FolderUnavailable));
        return true;
    }
    loop {
        if shared.stopped.load(Ordering::SeqCst) {
            return false;
        }
        let Ok(batch) = dirty_batch(reader) else {
            eprintln!("kinige: копия: не удалось прочитать отметки дней, повтор позже");
            return true;
        };
        if batch.is_empty() {
            shared.set_error(None);
            return false;
        }
        // Папки чатов по их нынешним названиям; чат уже сверен с папкой, где лежат его файлы, — раз за проход.
        let folders = crate::chats::folder_map(reader).unwrap_or_default();
        let mut reconciled: HashSet<String> = HashSet::new();
        for (chat_id, date, seq) in batch {
            let folder = folders
                .get(&chat_id)
                .cloned()
                .unwrap_or_else(|| chat_id.clone());
            if reconciled.insert(chat_id.clone())
                && !reconcile_folder(reader, writer, &dir, &chat_id, &folder)
            {
                return true;
            }
            if let Err(error) = write_day(reader, &dir, &chat_id, &folder, &date) {
                shared.set_error(Some(error));
                return true;
            }
            // Отметка снимается, только если за время записи её не обновили: новая правка того же дня оставит отметку.
            let cleared = writer.run(move |conn| {
                conn.execute(
                    "DELETE FROM mirror_dirty WHERE chat_id = ?1 AND journal_date = ?2 AND seq = ?3",
                    (chat_id, date, seq),
                )
            });
            if !matches!(cleared, Ok(Ok(_))) {
                return true;
            }
        }
    }
}

fn dirty_batch(reader: &Connection) -> rusqlite::Result<Vec<(String, String, i64)>> {
    let mut statement = reader.prepare(
        "SELECT chat_id, journal_date, seq FROM mirror_dirty ORDER BY seq, chat_id LIMIT 50",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect()
}

/// Корневая папка существует (создаётся при необходимости) и это каталог.
fn prepare_root(dir: &Path) -> bool {
    fs::create_dir_all(dir).is_ok() && dir.is_dir()
}

/// Папка чата в копии совпадает с папкой, где лежат его файлы: при переименовании (или смене владельца простого имени)
/// старая папка переезжает целиком. Ошибка переименования не останавливает копию: дни чата всё равно записываются в
/// новую папку. `false` — не удалось записать сведения о папке, нужен повтор.
fn reconcile_folder(
    reader: &Connection,
    writer: &Writer,
    root: &Path,
    chat_id: &str,
    folder: &str,
) -> bool {
    let known: Option<String> = reader
        .query_row(
            "SELECT folder FROM mirror_chat WHERE chat_id = ?1",
            [chat_id],
            |row| row.get(0),
        )
        .optional()
        .unwrap_or(None);
    if known.as_deref() == Some(folder) {
        return true;
    }
    if let Some(previous) = known.as_deref() {
        let (from, to) = (root.join(previous), root.join(folder));
        if from.is_dir() {
            // Имена, отличающиеся регистром, на macOS — одна папка: переезд через промежуточное имя.
            let same_folder = previous.to_lowercase() == folder.to_lowercase();
            if same_folder {
                let hop = root.join(format!(".{folder}.rename"));
                if fs::rename(&from, &hop).is_ok() && fs::rename(&hop, &to).is_err() {
                    let _ = fs::rename(&hop, &from);
                }
            } else if !to.exists() {
                let _ = fs::rename(&from, &to);
            }
        }
    }
    let (chat_id, folder) = (chat_id.to_string(), folder.to_string());
    matches!(
        writer.run(move |conn| conn.execute(
            "INSERT INTO mirror_chat (chat_id, folder) VALUES (?1, ?2) \
             ON CONFLICT(chat_id) DO UPDATE SET folder = excluded.folder",
            (chat_id, folder),
        )),
        Ok(Ok(_))
    )
}

/// Перезаписывает файл одного дня одного чата или удаляет его, если записей не осталось.
fn write_day(
    reader: &Connection,
    root: &Path,
    chat_id: &str,
    folder: &str,
    journal_date: &str,
) -> Result<(), MirrorError> {
    let Some(segments) = day_file_segments(folder, journal_date) else {
        // Недопустимая дата или имя папки: файла быть не может, отметку снимаем (иначе она не сойдёт никогда).
        return Ok(());
    };
    let entries =
        day_entries(reader, chat_id, journal_date).map_err(|_| MirrorError::WriteFailed)?;
    let [chat_dir, year_dir, file_name] = segments;
    let year_path = root.join(&chat_dir).join(&year_dir);
    let file_path = year_path.join(&file_name);

    if entries.is_empty() {
        match fs::remove_file(&file_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(MirrorError::WriteFailed),
        }
        // Пустые папки года и чата убираем, если они опустели; корень копии не трогаем.
        let _ = fs::remove_dir(&year_path);
        let _ = fs::remove_dir(root.join(&chat_dir));
        return Ok(());
    }

    let text = render_day_file(journal_date, &entries);
    write_atomically(&year_path, &file_name, text.as_bytes()).map_err(|_| MirrorError::WriteFailed)
}

/// Временный файл рядом с целевым → запись → `fsync` → переименование: читатель видит либо старый, либо новый файл целиком.
fn write_atomically(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{name}.tmp"));
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, dir.join(name))?;
        // Запись в каталоге (само переименование) тоже сбрасывается на диск: иначе после потери питания файл мог бы
        // оказаться под прежним именем. На Windows каталог открыть нельзя, там ошибка игнорируется.
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn day_entries(
    reader: &Connection,
    chat_id: &str,
    journal_date: &str,
) -> rusqlite::Result<Vec<MirrorEntry>> {
    let mut statement = reader.prepare(
        "SELECT body_md, created_at, created_tz FROM message \
         WHERE chat_id = ?1 AND journal_date = ?2 AND deleted_at IS NULL ORDER BY created_at, id",
    )?;
    statement
        .query_map((chat_id, journal_date), |row| {
            let created_at: i64 = row.get(1)?;
            let tz: String = row.get(2)?;
            Ok(MirrorEntry {
                time: local_hhmm(created_at, &tz),
                body_md: row.get(0)?,
            })
        })?
        .collect()
}
