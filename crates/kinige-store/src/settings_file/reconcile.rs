//! Согласование файла и базы (ADR-0010): чтение, применение внешней правки, правило «хэш совпал — побеждает база»,
//! запись файла из состояния.

use super::apply::{ApplyInput, KEY_FILE_HASH};
use super::engine::{Engine, ShortcutRefusal};
use super::io::{self, ReadOutcome};
use super::parse::{Parsed, parse};
use crate::Now;
use crate::agent_settings::put;
use crate::error::write_err;
use kinige_domain::{
    FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, SettingsFileState, ShortcutKind,
    UnsafeReason,
};
use sha2::{Digest, Sha256};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

impl Engine {
    pub(super) fn tick(&mut self) {
        // Отложенная запись что-то значит, только пока файл можно перезаписывать: битый, небезопасный или нечитаемый файл
        // перечитывается лишь когда он изменился (признак), а не на каждом опросе.
        let due = self.retry_at.is_none_or(|at| Instant::now() >= at);
        let retry_write = self.pending_write && self.writable && due;
        if io::signature(&self.path) == self.last_sig && !retry_write {
            return;
        }
        self.reconcile();
    }

    /// Запись файла не удалась: следующая попытка позже (2, 4, … 60 секунд), а не на каждом опросе.
    fn write_failed(&mut self) {
        self.pending_write = true;
        self.write_failures = self.write_failures.saturating_add(1);
        let wait = 2u64.saturating_pow(self.write_failures.min(6)).min(60);
        self.retry_at = Some(Instant::now() + Duration::from_secs(wait));
    }

    /// Предупреждение в журнал процесса один раз, пока тот же вид ошибки не отработал успешно (`forget`): повторные проходы
    /// молчат (иначе битый файл или занятая база давали бы строку на каждый опрос). Виды независимы: чередование двух
    /// ошибок строк не добавляет. Только категории, без данных.
    fn warn_once(&mut self, key: String, text: &str) {
        if self.logged.insert(key.clone()) {
            log::warn!("{text}: {key}");
        }
    }

    /// Вид ошибки (приставка ключа) отработал успешно: следующий сбой снова попадёт в журнал.
    fn forget(&mut self, prefix: &str) {
        self.logged.retain(|key| !key.starts_with(prefix));
    }

    /// Читает файл и приводит его и базу в согласие. Хэш совпал с последним синхронизированным — внешней правки нет, база
    /// выигрывает (и при каждом чтении, не только при старте); не совпал — файл применяется как внешняя правка.
    pub(super) fn reconcile(&mut self) {
        match io::read_file(&self.path, &self.dir) {
            ReadOutcome::Missing => {
                self.forget("read");
                self.forget("parse");
                self.forget("unsafe");
                self.last_sig = None;
                self.base = None;
                self.writable = true;
                let directory = io::unsafe_directory(&self.dir);
                self.update(|state| {
                    state.problem = None;
                    state.unsafe_reason = directory;
                    state.issues.clear();
                    state.unknown_keys.clear();
                    state.unknown_more = 0;
                });
                self.write_from_state();
            }
            ReadOutcome::Failed(error) => {
                self.warn_once(
                    format!("read {}", error.category()),
                    "файл настроек не прочитан",
                );
                self.last_sig = io::signature(&self.path);
                self.writable = false;
                self.base = None;
                self.update(|state| {
                    state.problem = Some(error.problem());
                    state.unsafe_reason = None;
                });
            }
            ReadOutcome::NotRegular => {
                self.warn_once("unsafe".into(), "файл настроек не обычный файл");
                self.last_sig = io::signature(&self.path);
                self.writable = false;
                self.base = None;
                self.update(|state| {
                    state.problem = None;
                    state.unsafe_reason = Some(UnsafeReason::NotRegular);
                });
            }
            ReadOutcome::Content {
                bytes,
                unsafe_reason,
                sig,
            } => {
                self.last_sig = sig;
                match parse(&bytes) {
                    Err(error) => {
                        self.warn_once(
                            format!("parse {}", error.category()),
                            "файл настроек не разобран",
                        );
                        self.writable = false;
                        self.base = None;
                        self.update(|state| {
                            state.problem = Some(error.problem());
                            state.unsafe_reason = unsafe_reason;
                            state.issues.clear();
                            state.unknown_keys.clear();
                            state.unknown_more = 0;
                        });
                    }
                    Ok(parsed) => {
                        self.forget("read");
                        self.forget("parse");
                        self.forget("unsafe");
                        self.writable = unsafe_reason.is_none();
                        self.base = Some(parsed.raw.clone());
                        let hash = sha256_hex(&bytes);
                        if self.stored_hash.as_deref() == Some(hash.as_str()) {
                            self.settle(parsed, unsafe_reason);
                        } else {
                            self.apply_external(parsed, hash, unsafe_reason);
                        }
                    }
                }
            }
        }
    }

    /// Внешняя правка: применяется всё годное. Небезопасный файл применяется без `agent_access.*` и `mirror.folder`.
    fn apply_external(
        &mut self,
        parsed: Parsed,
        hash: String,
        unsafe_reason: Option<UnsafeReason>,
    ) {
        let unknown = parsed.unknown.clone();
        let unknown_more = parsed.unknown_more as u32;
        let input = ApplyInput {
            parsed,
            trusted: unsafe_reason.is_none(),
            data_dir: self.dir.clone(),
            hash: hash.clone(),
            at_ms: Now::system().ms,
        };
        let outcome = match self.store.apply_settings_file(input) {
            Ok(outcome) => outcome,
            Err(error) => {
                // Ничего не применено: хэш не сдвинут, следующий опрос попробует снова.
                self.warn_once(
                    format!("apply {}", error.category()),
                    "файл настроек не применён",
                );
                self.last_sig = None;
                return;
            }
        };
        self.stored_hash = Some(hash);
        self.forget("apply");
        if outcome.mirror {
            // Папка копии сменилась: писатель копии просыпается и пишет дни в новую папку сразу.
            self.store.mirror_dir_changed();
        }
        // Правка из файла новее отложенной правки окна: побеждает последняя.
        self.pending_write = false;
        let mut issues = outcome.issues;
        let mut shortcuts_changed = false;
        for (kind, accelerator) in outcome.shortcuts {
            match self.apply_shortcut(kind, &accelerator) {
                Ok(()) => shortcuts_changed = true,
                Err(refusal) => issues.push(KeyIssue {
                    key: format!("shortcuts.{}", shortcut_name(kind)),
                    kind: match refusal {
                        ShortcutRefusal::Invalid => KeyIssueKind::ShortcutInvalid,
                        ShortcutRefusal::Duplicate => KeyIssueKind::ShortcutDuplicate,
                        ShortcutRefusal::Taken => KeyIssueKind::ShortcutTaken,
                    },
                }),
            }
        }
        let now = Now::system().ms;
        self.update(|state| {
            state.problem = None;
            state.unsafe_reason = unsafe_reason;
            state.issues = issues;
            state.unknown_keys = unknown;
            state.unknown_more = unknown_more;
            let stamp = |flag: bool, slot: &mut Option<i64>| {
                if flag {
                    *slot = Some(now);
                }
            };
            stamp(outcome.appearance, &mut state.changed.appearance);
            stamp(shortcuts_changed, &mut state.changed.shortcuts);
            stamp(outcome.mirror, &mut state.changed.mirror);
            stamp(outcome.access, &mut state.changed.access);
        });
        self.notify();
    }

    fn apply_shortcut(&self, kind: ShortcutKind, accelerator: &str) -> Result<(), ShortcutRefusal> {
        use crate::StoreError;
        match (&self.options.shortcuts, self.initial) {
            (Some(applier), false) => applier.apply(&self.store, kind, accelerator),
            // Запуск: сочетания регистрируются позже (`register_saved`); сейчас только проверка и сохранение.
            _ => self
                .store
                .save_shortcut(kind, accelerator)
                .map_err(|error| match error {
                    StoreError::ShortcutDuplicate => ShortcutRefusal::Duplicate,
                    _ => ShortcutRefusal::Invalid,
                }),
        }
    }

    /// Хэш совпал: внешней правки нет. Если безопасный и разобранный файл отличается от состояния (правка окна не дошла
    /// до файла: обрыв, небезопасное состояние), он переписывается из базы.
    fn settle(&mut self, parsed: Parsed, unsafe_reason: Option<UnsafeReason>) {
        let in_sync = match self.store.effective_state() {
            Ok(state) => {
                self.forget("state");
                state.matches(&parsed)
            }
            Err(error) => {
                self.warn_once(
                    format!("state {}", error.category()),
                    "состояние настроек не прочитано",
                );
                true
            }
        };
        let rewrite = unsafe_reason.is_none() && (self.pending_write || !in_sync);
        if !rewrite {
            self.update(|state| {
                state.problem = None;
                state.unsafe_reason = unsafe_reason;
                // Ошибки разбора пересчитываются, ошибки состояния (папка, чаты, система) остаются до следующей правки.
                state.issues.retain(|issue| !is_parse_level(issue.kind));
                for issue in parsed.issues {
                    if !state.issues.contains(&issue) {
                        state.issues.push(issue);
                    }
                }
                state.unknown_keys = parsed.unknown;
                state.unknown_more = parsed.unknown_more as u32;
            });
            return;
        }
        self.write_from_state();
    }

    /// После правки окна: файл переписывается из базы, если его можно перезаписывать, иначе запись откладывается.
    pub(super) fn after_edit(&mut self) {
        if self.writable {
            self.write_from_state();
        } else {
            self.pending_write = true;
        }
        self.notify();
    }

    /// Файл из действующего состояния: временный файл, `fsync`, `rename`; хэш записанных байтов — после `rename`.
    fn write_from_state(&mut self) {
        let state = match self.store.effective_state() {
            Ok(state) => state,
            Err(error) => {
                self.warn_once(
                    format!("write-state {}", error.category()),
                    "файл настроек не записан",
                );
                self.write_failed();
                return;
            }
        };
        self.forget("write-state");
        let bytes = state.render(self.base.as_ref());
        let hash = sha256_hex(&bytes);
        if let Err(error) = io::write_atomic(&self.path, &self.dir, &bytes) {
            self.warn_once(
                format!("write-io {:?}", error.kind()),
                "файл настроек не записан",
            );
            self.write_failed();
            self.update(|state| {
                state.problem = Some(FileProblem {
                    kind: FileProblemKind::Io,
                    line: None,
                    column: None,
                });
            });
            return;
        }
        self.forget("write-io");
        // Признак — сразу после `rename`, до шага хэша: если файл подменят позже, признак изменится и опрос его заметит.
        self.last_sig = io::signature(&self.path);
        if let Some(step) = &self.options.after_rename {
            step();
        }
        let stored = hash.clone();
        let saved = self
            .store
            .write(move |conn| put(conn, KEY_FILE_HASH, &stored).map_err(write_err));
        if let Err(error) = saved {
            self.warn_once(
                format!("hash {}", error.category()),
                "хэш файла настроек не записан",
            );
        } else {
            self.stored_hash = Some(hash.clone());
            self.forget("hash");
        }
        self.pending_write = false;
        self.write_failures = 0;
        self.retry_at = None;
        // Если на диске уже не то, что записано (агент успел заменить файл), он будет прочитан и применён следующим опросом.
        // Читается тем же безопасным чтением (без перехода по ссылке, канал и устройство не читаются).
        let same = matches!(
            io::read_file(&self.path, &self.dir),
            ReadOutcome::Content { bytes, .. } if sha256_hex(&bytes) == hash
        );
        if !same {
            self.last_sig = None;
        }
        let (unknown, unknown_more) = parse(&bytes)
            .map(|parsed| (parsed.unknown, parsed.unknown_more as u32))
            .unwrap_or_default();
        // Каталог данных мог оказаться небезопасным при самом файле в порядке: причина остаётся видна.
        let directory = io::unsafe_directory(&self.dir);
        self.update(|state| {
            state.problem = None;
            state.unsafe_reason = directory;
            state.issues.clear();
            state.unknown_keys = unknown;
            state.unknown_more = unknown_more;
        });
    }

    fn update(&self, change: impl FnOnce(&mut SettingsFileState)) {
        let mut state = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut state);
    }

    fn notify(&self) {
        if !self.initial {
            (self.options.on_change)();
        }
    }
}

fn is_parse_level(kind: KeyIssueKind) -> bool {
    matches!(
        kind,
        KeyIssueKind::Type | KeyIssueKind::Value | KeyIssueKind::ShortcutInvalid
    )
}

fn shortcut_name(kind: ShortcutKind) -> &'static str {
    match kind {
        ShortcutKind::Capture => "capture",
        ShortcutKind::Clipboard => "clipboard",
    }
}

/// SHA-256 в hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
