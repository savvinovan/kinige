//! The writer thread (PRD §3.2): all writes go through one connection owned by one thread.

use crate::StoreError;
use rusqlite::Connection;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// A handle of the writer thread. The thread ends when all handles are dropped.
#[derive(Clone)]
pub(crate) struct Writer {
    jobs: mpsc::Sender<Job>,
}

impl Writer {
    pub(crate) fn spawn(mut conn: Connection) -> Result<Writer, StoreError> {
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("kinige-writer".into())
            .spawn(move || {
                for job in queue {
                    // A panic in a task must not stop the writer: the transaction rolls back on stack unwinding,
                    // the caller gets `Internal`.
                    let _ = catch_unwind(AssertUnwindSafe(|| job(&mut conn)));
                }
            })
            .map_err(|_| StoreError::Internal("could not start the writer thread".into()))?;
        Ok(Writer { jobs })
    }

    /// Runs a task on the writer thread and waits for the result.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Connection) -> T + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |conn| {
                let _ = reply.send(job(conn));
            }))
            .map_err(|_| StoreError::Internal("the writer thread has stopped".into()))?;
        result
            .recv()
            .map_err(|_| StoreError::Internal("the writer task was interrupted".into()))
    }
}
