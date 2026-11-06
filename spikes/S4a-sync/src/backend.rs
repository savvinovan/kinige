//! The server behind a trait (S4c-1, KINIG-98). The world, the oracle and the properties talk to a [`SyncServer`]; one implementation is the
//! in-memory server of `server.rs` (the default, with the switches of the negative controls), the other (feature `real-server`,
//! `http_server.rs`) is the real `kinige-server` router on Postgres, called in process, so HTTP parsing, SQL and the real envelope bytes are
//! exercised together with the protocol. Since S4c-2 both can take a backup and be restored from it: the in-memory one by cloning, the real
//! one by `kinige_server::backup` into a clean database, the code path the operator uses.
//!
//! Which one a [`World`](crate::world::World) gets is decided by [`with_backend`]: a test installs a factory for the current thread and
//! every world built inside the closure uses it. Nothing else in the simulation knows the difference.

use crate::envelope::{Envelope, Key};
use crate::server::{PullResponse, PushResponse, Server};
use crate::switches::Switches;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Where the pushing or pulling device stands (appendix §A.3, `stale`; ADR-0017): its generation, its `seq`, the drop epoch it last saw
/// and the cut time that came with the `GenerationChanged` answer which put it into its generation (0 if none did).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub generation: u64,
    pub seq: u64,
    pub known_epoch: u64,
    pub cut_ms: u64,
}

/// A backup the server took (S4c-2). Opaque to the world; a server restores only a backup of its own kind.
pub struct Backup(pub Box<dyn Any>);

pub trait SyncServer {
    /// The current generation.
    fn generation(&self) -> u64;
    /// `cursor` is the pusher's; the in-memory server ignores it, the real server answers `stale` when it is behind a drop it missed.
    fn push(&mut self, env: &Arc<Envelope>, now_ms: u64, cursor: Cursor) -> PushResponse;
    fn pull(&mut self, cursor: Cursor, limit: usize) -> PullResponse;
    fn compact(&mut self, now_ms: u64);
    fn head(&self) -> u64;
    fn issued(&self) -> u64;
    fn log_len(&self) -> usize;
    /// No envelope is waiting for its delayed commit (only the `no_seq_lock` switch of the in-memory server).
    fn settled(&self) -> bool;
    fn seq_of(&self, op_id: &str) -> Option<u64>;
    fn all_envelopes(&self) -> Vec<Arc<Envelope>>;
    /// Every entity for which a `purged` was ever accepted (not reduced by compaction): for the property checks.
    fn purged_ever(&self) -> Vec<Key>;
    /// How many compactions have dropped a `purged`.
    fn drops_count(&self) -> usize;
    /// A backup of the server as it is at `now_ms`, the cut time of the generation a restore from it makes.
    fn backup(&mut self, now_ms: u64) -> Backup;
    /// Restores from `backup` (taken by this kind of server): a new generation appended to the lineage, cut at the backup's time.
    /// `nonce` names the new generation for the in-memory server; the real one draws its own.
    fn restore(&mut self, backup: &Backup, nonce: u64, now_ms: u64);
    /// The in-memory server, for the scenarios that look inside it (lineage, drops).
    fn as_memory(&self) -> Option<&Server> {
        None
    }
    fn as_memory_mut(&mut self) -> Option<&mut Server> {
        None
    }
}

impl SyncServer for Server {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn push(&mut self, env: &Arc<Envelope>, now_ms: u64, _cursor: Cursor) -> PushResponse {
        Server::push(self, env, now_ms)
    }

    fn pull(&mut self, cursor: Cursor, limit: usize) -> PullResponse {
        Server::pull(self, cursor, limit)
    }

    fn compact(&mut self, now_ms: u64) {
        Server::compact(self, now_ms);
    }

    fn head(&self) -> u64 {
        Server::head(self)
    }

    fn issued(&self) -> u64 {
        Server::issued(self)
    }

    fn log_len(&self) -> usize {
        Server::log_len(self)
    }

    fn settled(&self) -> bool {
        Server::settled(self)
    }

    fn seq_of(&self, op_id: &str) -> Option<u64> {
        Server::seq_of(self, op_id)
    }

    fn all_envelopes(&self) -> Vec<Arc<Envelope>> {
        Server::all_envelopes(self)
    }

    fn purged_ever(&self) -> Vec<Key> {
        self.purged_ever.clone()
    }

    fn drops_count(&self) -> usize {
        Server::drops_count(self)
    }

    fn backup(&mut self, now_ms: u64) -> Backup {
        Backup(Box::new(Server::backup(self, now_ms)))
    }

    fn restore(&mut self, backup: &Backup, nonce: u64, _now_ms: u64) {
        if let Some(dump) = backup.0.downcast_ref::<Server>() {
            Server::restore(self, dump, nonce);
        }
    }

    fn as_memory(&self) -> Option<&Server> {
        Some(self)
    }

    fn as_memory_mut(&mut self) -> Option<&mut Server> {
        Some(self)
    }
}

type Factory = Rc<dyn Fn() -> Box<dyn SyncServer>>;

thread_local! {
    static FACTORY: RefCell<Option<Factory>> = const { RefCell::new(None) };
}

/// Runs `run` with every [`World`](crate::world::World) built on this thread inside it using the server `factory` makes. The factory is
/// removed when `run` ends, also when it panics.
pub fn with_backend<R>(
    factory: impl Fn() -> Box<dyn SyncServer> + 'static,
    run: impl FnOnce() -> R,
) -> R {
    struct Restore(Option<Factory>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FACTORY.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = FACTORY.with(|slot| slot.borrow_mut().replace(Rc::new(factory)));
    let _restore = Restore(previous);
    run()
}

/// The server a new world uses: the installed factory's, or the in-memory one with the given switches.
pub(crate) fn new_server(switches: Switches) -> Box<dyn SyncServer> {
    match FACTORY.with(|slot| slot.borrow().clone()) {
        Some(factory) => factory(),
        None => Box::new(Server::new(switches)),
    }
}
