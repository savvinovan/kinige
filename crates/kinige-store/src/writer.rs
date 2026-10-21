//! Поток-писатель (ТЗ §3.2): все записи идут через одно соединение, которым владеет один поток.

use crate::StoreError;
use rusqlite::Connection;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Дескриптор потока-писателя. Поток завершается, когда уничтожены все дескрипторы.
#[derive(Clone)]
pub(crate) struct Writer {
    jobs: mpsc::Sender<Job>,
}

impl Writer {
    pub(crate) fn spawn(mut conn: Connection) -> Writer {
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("kinige-writer".into())
            .spawn(move || {
                for job in queue {
                    // Паника в задаче не должна останавливать писателя: транзакция откатится при размотке стека,
                    // вызывающий получит `Internal`.
                    let _ = catch_unwind(AssertUnwindSafe(|| job(&mut conn)));
                }
            })
            .expect("не удалось запустить поток-писатель");
        Writer { jobs }
    }

    /// Выполняет задачу на потоке-писателе и ждёт результат.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Connection) -> T + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |conn| {
                let _ = reply.send(job(conn));
            }))
            .map_err(|_| StoreError::Internal("поток-писатель остановлен".into()))?;
        result
            .recv()
            .map_err(|_| StoreError::Internal("задача писателя прервана".into()))
    }
}
