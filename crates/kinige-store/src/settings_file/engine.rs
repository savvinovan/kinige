//! Поток файла настроек (ADR-0010): опрос раз в секунду, применение внешних правок, запись после правок из окна. Вся работа
//! с файлом идёт здесь, в одном потоке; правка из окна выполняется в нём же как задание.

use super::apply::KEY_FILE_HASH;
use super::io::{self, Sig};
use crate::Store;
use crate::settings::get_setting;
use kinige_domain::{SettingsFileState, ShortcutKind};
use serde_json::{Map, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Как часто поток смотрит на файл.
pub const POLL: Duration = Duration::from_secs(1);

/// Система (или проверка) не приняла сочетание из файла.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutRefusal {
    Invalid,
    Duplicate,
    Taken,
}

/// Принимает сочетание из файла: проверка, регистрация в системе, сохранение — как при смене в окне. Слой Tauri.
pub trait ShortcutApplier: Send + Sync {
    fn apply(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ShortcutRefusal>;
}

/// Параметры потока файла.
pub struct SettingsFileOptions {
    pub poll: Duration,
    /// Принимает сочетания после запуска. `None` — только запись в базу (тесты хранилища).
    pub shortcuts: Option<Arc<dyn ShortcutApplier>>,
    /// Что-то изменилось (применён файл, записан файл, правка окна): слой Tauri сообщает окнам. Не блокирует.
    pub on_change: Arc<dyn Fn() + Send + Sync>,
    /// Только для тестов: шаг между `rename` приложения и записью хэша (гонка «агент заменил файл»).
    #[doc(hidden)]
    pub after_rename: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl SettingsFileOptions {
    pub fn new(on_change: Arc<dyn Fn() + Send + Sync>) -> Self {
        SettingsFileOptions {
            poll: POLL,
            shortcuts: None,
            on_change,
            after_rename: None,
        }
    }
}

type Job = Box<dyn FnOnce(&mut Engine) + Send>;

/// Дескриптор потока файла. Поток живёт, пока жив дескриптор.
pub struct SettingsFile {
    jobs: Sender<Job>,
    status: Arc<Mutex<SettingsFileState>>,
}

impl SettingsFile {
    /// Убирает брошенные временные файлы, один раз приводит файл и базу в согласие (синхронно: файл применяется до
    /// регистрации сочетаний и до слушателя MCP) и запускает поток опроса.
    pub fn start(store: Store, data_dir: PathBuf, options: SettingsFileOptions) -> SettingsFile {
        io::remove_stale_temp(&data_dir);
        let path = io::file_path(&data_dir);
        let status = Arc::new(Mutex::new(SettingsFileState {
            path: path.to_string_lossy().into_owned(),
            ..SettingsFileState::default()
        }));
        let stored_hash = store
            .read(|conn| get_setting(conn, KEY_FILE_HASH).map_err(crate::error::read_err))
            .ok()
            .flatten();
        let mut engine = Engine {
            store,
            dir: data_dir,
            path,
            options,
            status: status.clone(),
            stored_hash,
            last_sig: None,
            pending_write: false,
            writable: false,
            base: None,
            initial: true,
            logged: None,
            write_failures: 0,
            retry_at: None,
        };
        guarded(&mut engine, Engine::reconcile);
        engine.initial = false;
        let (jobs, inbox) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("kinige-settings-file".into())
            .spawn(move || run(engine, inbox))
            .ok();
        SettingsFile { jobs, status }
    }

    /// Состояние файла для окна.
    pub fn state(&self) -> SettingsFileState {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Правка из окна: в потоке файла сначала применяется внешняя правка (если файл изменился), затем `change`, затем файл
    /// записывается. `None` — поток остановился или `change` упал с паникой.
    pub fn edit<T: Send + 'static, E: Send + 'static>(
        &self,
        change: impl FnOnce(&Store) -> Result<T, E> + Send + 'static,
    ) -> Option<Result<T, E>> {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |engine| {
            engine.reconcile();
            let result = change(&engine.store);
            if result.is_ok() {
                engine.after_edit();
            }
            let _ = reply.send(result);
        });
        self.jobs.send(job).ok()?;
        answer.recv().ok()
    }

    /// Перечитать файл сейчас, не дожидаясь опроса (тесты).
    #[doc(hidden)]
    pub fn sync_now(&self) {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |engine| {
            engine.reconcile();
            let _ = reply.send(());
        });
        if self.jobs.send(job).is_ok() {
            let _ = answer.recv();
        }
    }
}

fn run(mut engine: Engine, inbox: mpsc::Receiver<Job>) {
    loop {
        match inbox.recv_timeout(engine.options.poll) {
            Ok(job) => {
                let _ = catch_unwind(AssertUnwindSafe(|| job(&mut engine)));
            }
            Err(RecvTimeoutError::Timeout) => guarded(&mut engine, Engine::tick),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Паника в потоке файла не должна его останавливать: ошибка в лог (без данных), поток продолжает.
fn guarded(engine: &mut Engine, step: fn(&mut Engine)) {
    if catch_unwind(AssertUnwindSafe(|| step(engine))).is_err() {
        log::error!("паника в потоке файла настроек, шаг пропущен");
    }
}

pub(super) struct Engine {
    pub(super) store: Store,
    pub(super) dir: PathBuf,
    pub(super) path: PathBuf,
    pub(super) options: SettingsFileOptions,
    pub(super) status: Arc<Mutex<SettingsFileState>>,
    /// Хэш байтов, с которыми база и файл согласованы в последний раз (зеркало `app_setting`).
    pub(super) stored_hash: Option<String>,
    pub(super) last_sig: Option<Sig>,
    /// Запись файла отложена (файл небезопасен, не разобран или запись не удалась).
    pub(super) pending_write: bool,
    /// Файл можно перезаписывать: его нет или он безопасен и разобран.
    pub(super) writable: bool,
    /// Документ последнего удачного чтения: из него берутся чужие ключи при записи.
    pub(super) base: Option<Map<String, Value>>,
    /// Запуск приложения: окон ещё нет, сочетания идут прямо в базу, событий нет.
    pub(super) initial: bool,
    /// Последнее записанное в журнал процесса предупреждение (вид и категория): повтор не пишется.
    pub(super) logged: Option<String>,
    /// Подряд неудачных записей файла и когда пробовать снова (с 2^n секунд, не реже раза в минуту).
    pub(super) write_failures: u32,
    pub(super) retry_at: Option<Instant>,
}
