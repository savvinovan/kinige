//! The world a schedule runs in: the server, two or three clients, a hostile network and a virtual clock with skew per device.
//! A schedule is a list of steps; nothing is random inside the run, so the same schedule (and the seed that produced it) always
//! gives the same result.

use crate::backend::{self, SyncServer};
use crate::client::{Client, FeedSlot, OpError};
use crate::envelope::{Entity, Envelope, Key};
use crate::oracle::Oracle;
use crate::server::{PullResponse, PushResponse, Server};
use crate::switches::Switches;
use std::sync::Arc;

/// The start of the virtual clock, ms (2027-01-15).
pub const T0: u64 = 1_800_000_000_000;

/// How long a pull that reached the head still licenses a push: the length of one sync cycle.
pub const CYCLE_MS: u64 = 60_000;

/// What happens to a push on the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushFault {
    Ok,
    /// The request never reaches the server.
    DropRequest,
    /// The server stores the envelopes, the client never hears: the same as a restart between "sent" and "ack saved".
    DropResponse,
    /// The request is delivered twice.
    Duplicate,
    /// The connection drops mid-push: `n` envelopes are processed and answered, one more is processed without an answer.
    CutAfter(u8),
    /// The batch arrives in reverse order.
    Reorder,
}

/// What happens to a pull page on the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullFault {
    Ok,
    Drop,
    Duplicate,
    /// An older answer arrives again (a delayed duplicate).
    Replay,
    /// The page's envelopes are applied in another order (a server that reorders, §3.5).
    Shuffle(u8),
}

/// What the user does; indexes pick the n-th record the device has, modulo how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    CreateChat,
    RenameChat(usize),
    DeleteChat(usize),
    RestoreChat(usize),
    CreateMessage(usize),
    Edit(usize),
    Favorite(usize),
    Move(usize, usize),
    DeleteMessage(usize),
    RestoreMessage(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Local(usize, Op),
    Push(usize, PushFault),
    Pull(usize, PullFault),
    /// The 30-day purge pass.
    Purge(usize),
    Restart(usize),
    /// Virtual time moves forward, ms.
    Advance(u64),
    /// Server compaction of envelopes older than the retention period.
    Compact,
    /// The server takes a backup.
    Backup,
    /// The server is restored from the last backup: a new generation.
    RestoreServer,
    /// A database without an account binds to it.
    Bind(usize),
    /// Every device except `skip` (a device index; one past the last device skips nobody) pulls to the head and pushes everything,
    /// over a healthy network. How the generator makes one device stay away while the others carry on.
    SyncAll {
        skip: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Schedule {
    pub clients: usize,
    /// Skew of each device's physical clock against the virtual time, ms.
    pub skews: Vec<i64>,
    /// Devices that start without an account (their data is local until `Bind`).
    pub unbound: Vec<bool>,
    pub compact_outbox: bool,
    pub page: usize,
    pub batch: usize,
    pub steps: Vec<Step>,
}

pub struct World {
    pub now: u64,
    /// The in-memory server, or the real one when the thread installed it with `backend::with_backend` (S4c-1).
    pub server: Box<dyn SyncServer>,
    pub clients: Vec<Client>,
    pub oracle: Oracle,
    pub violations: Vec<String>,
    page: usize,
    batch: usize,
    last_page: Vec<Option<PullResponse>>,
    backup: Option<Server>,
    /// A server restore happened (for the property of §6.3).
    pub restored: bool,
    /// How many restores there have been: the source of the random numbers that name new generations.
    restores: u64,
}

impl World {
    pub fn new(schedule: &Schedule, switches: Switches) -> World {
        let n = schedule.clients;
        let clients = (0..n)
            .map(|i| {
                let skew = schedule.skews.get(i).copied().unwrap_or(0);
                let bound = !schedule.unbound.get(i).copied().unwrap_or(false);
                Client::new(i, skew, bound, schedule.compact_outbox, switches, T0)
            })
            .collect();
        World {
            now: T0,
            server: backend::new_server(switches),
            clients,
            oracle: Oracle::default(),
            violations: Vec::new(),
            page: schedule.page.max(1),
            batch: schedule.batch.max(1),
            last_page: vec![None; n],
            backup: None,
            restored: false,
            restores: 0,
        }
    }

    /// The in-memory server, for the scenarios that look inside it (lineage, drops, backups).
    pub fn memory(&self) -> &Server {
        self.server
            .as_memory()
            .unwrap_or_else(|| panic!("this scenario needs the in-memory server"))
    }

    pub fn memory_mut(&mut self) -> &mut Server {
        self.server
            .as_memory_mut()
            .unwrap_or_else(|| panic!("this scenario needs the in-memory server"))
    }

    pub fn run(&mut self, steps: &[Step]) {
        for step in steps {
            self.step(*step);
        }
    }

    pub fn step(&mut self, step: Step) {
        for client in &mut self.clients {
            client.now_ms = self.now;
        }
        match step {
            Step::Local(c, op) => self.local(c, op),
            Step::Push(c, fault) => self.push(c, fault),
            Step::Pull(c, fault) => self.pull(c, fault),
            Step::Purge(c) => {
                // The purge pass runs at the end of a sync cycle, after its pull: on a complete view (a model assumption, see README).
                self.pull_to_head(c);
                if let Some(client) = self.clients.get_mut(c) {
                    client.purge_due(self.now);
                }
            }
            Step::Restart(c) => {
                if let Some(client) = self.clients.get_mut(c) {
                    client.restart(self.now);
                }
            }
            Step::Advance(ms) => {
                self.now += ms;
                if ms >= CYCLE_MS {
                    // A pull-then-push cycle takes seconds: a pull that reached the head a minute ago licenses no push (§3.5).
                    for client in &mut self.clients {
                        client.cycle_ready = false;
                    }
                }
            }
            Step::Compact => self.server.compact(self.now),
            // A server that cannot restore (the real one, until S4c-2) has no backup to take: both steps are skipped.
            Step::Backup => {
                if let Some(memory) = self.server.as_memory() {
                    self.backup = Some(memory.backup());
                }
            }
            Step::RestoreServer => {
                if let (Some(backup), Some(memory)) =
                    (self.backup.clone(), self.server.as_memory_mut())
                {
                    // A random number from outside the server, as a real restore draws one; it cannot repeat in a run.
                    self.restores += 1;
                    let nonce = self.restores.wrapping_mul(0x9e37_79b9_7f4a_7c15);
                    memory.restore(&backup, nonce);
                    self.restored = true;
                }
            }
            Step::Bind(c) => {
                let generation = self.server.generation();
                if let Some(client) = self.clients.get_mut(c) {
                    client.begin_binding(generation);
                }
            }
            Step::SyncAll { skip } => {
                for c in (0..self.clients.len()).filter(|c| *c != skip) {
                    self.sync_device(c);
                }
            }
        }
        self.sync_clocks();
        self.drain_writes();
    }

    /// The devices read the shared virtual time from here (the handlers of network answers do not take it as an argument).
    fn sync_clocks(&mut self) {
        for client in &mut self.clients {
            client.now_ms = self.now;
        }
    }

    /// One device pulls to the head and pushes everything, with no faults.
    fn sync_device(&mut self, c: usize) {
        for _ in 0..200 {
            self.pull_to_head(c);
            if !self.clients.get(c).is_some_and(|cl| cl.has_pending_work()) {
                break;
            }
            self.push(c, PushFault::Ok);
        }
    }

    fn pull_to_head(&mut self, c: usize) {
        for _ in 0..200 {
            let before = self.client_mark(c);
            self.pull(c, PullFault::Ok);
            if self.client_mark(c) == before
                && self.clients.get(c).is_some_and(|cl| {
                    cl.cursor_seq >= self.server.head() && (cl.reached_head || !cl.can_pull())
                })
            {
                break;
            }
        }
    }

    fn local(&mut self, c: usize, op: Op) {
        let now = self.now;
        let Some(client) = self.clients.get_mut(c) else {
            return;
        };
        let chats = client.live_keys(Entity::Chat);
        let messages = client.live_keys(Entity::Message);
        let pick =
            |keys: &[Key], i: usize| keys.get(i.checked_rem(keys.len()).unwrap_or(0)).cloned();
        let result: Result<(), OpError> = match op {
            Op::CreateChat => {
                client.create_chat(now);
                Ok(())
            }
            Op::RenameChat(i) => pick(&chats, i).map_or(Ok(()), |k| client.rename_chat(&k, now)),
            Op::DeleteChat(i) => {
                pick(&chats, i).map_or(Ok(()), |k| client.set_deleted(&k, true, now))
            }
            Op::RestoreChat(i) => {
                pick(&chats, i).map_or(Ok(()), |k| client.set_deleted(&k, false, now))
            }
            Op::CreateMessage(i) => {
                if let Some(chat) = pick(&chats, i) {
                    client.create_message(&chat, now);
                }
                Ok(())
            }
            Op::Edit(i) => pick(&messages, i).map_or(Ok(()), |k| client.edit_body(&k, now)),
            Op::Favorite(i) => {
                pick(&messages, i).map_or(Ok(()), |k| client.toggle_favorite(&k, now))
            }
            Op::Move(i, j) => match (pick(&messages, i), pick(&chats, j)) {
                (Some(message), Some(chat)) => client.move_message(&message, &chat, now),
                _ => Ok(()),
            },
            Op::DeleteMessage(i) => {
                pick(&messages, i).map_or(Ok(()), |k| client.set_deleted(&k, true, now))
            }
            Op::RestoreMessage(i) => {
                pick(&messages, i).map_or(Ok(()), |k| client.set_deleted(&k, false, now))
            }
        };
        if let Err(OpError::HlcNotAbove) = result {
            self.violations.push(format!(
                "device {c}: a write could not get an HLC above the version it replaces ({op:?})"
            ));
        }
    }

    fn push(&mut self, c: usize, fault: PushFault) {
        let now = self.now;
        let (batch, cursor) = match self.clients.get_mut(c) {
            Some(client) => (client.next_batch(self.batch), client.pull_request()),
            None => return,
        };
        let mut batch: Vec<Arc<Envelope>> = batch;
        if fault == PushFault::Reorder {
            batch.reverse();
        }
        for (i, env) in batch.iter().enumerate() {
            let (deliver, hear) = match fault {
                PushFault::DropRequest => (false, false),
                PushFault::DropResponse => (true, false),
                PushFault::CutAfter(n) => (i <= usize::from(n), i < usize::from(n)),
                _ => (true, true),
            };
            if !deliver {
                continue;
            }
            let response = self.server.push(env, now, cursor);
            if fault == PushFault::Duplicate {
                let again = self.server.push(env, now, cursor);
                self.answer(c, env, again, hear);
            }
            self.answer(c, env, response, hear);
        }
    }

    fn answer(&mut self, c: usize, env: &Envelope, response: PushResponse, hear: bool) {
        if !hear {
            return;
        }
        if let Some(client) = self.clients.get_mut(c) {
            client.on_push_response(&env.header.op_id, response);
        }
    }

    fn pull(&mut self, c: usize, fault: PullFault) {
        let now = self.now;
        let Some(client) = self.clients.get(c) else {
            return;
        };
        if !client.can_pull() {
            return;
        }
        let (generation, after, known_epoch) = client.pull_request();
        let response = self.server.pull(generation, after, known_epoch, self.page);
        let delivered = match fault {
            PullFault::Drop => None,
            PullFault::Replay => self
                .last_page
                .get(c)
                .cloned()
                .flatten()
                .or(Some(response.clone())),
            _ => Some(response.clone()),
        };
        if let Some(slot) = self.last_page.get_mut(c) {
            *slot = Some(response);
        }
        let Some(delivered) = delivered else {
            return;
        };
        let copies = if fault == PullFault::Duplicate { 2 } else { 1 };
        for _ in 0..copies {
            let shuffle = match fault {
                PullFault::Shuffle(k) => Some(k),
                _ => None,
            };
            if let Some(client) = self.clients.get_mut(c) {
                client.on_pull_response(delivered.clone(), now, |items| {
                    if let Some(k) = shuffle {
                        items.reverse();
                        let len = items.len().max(1);
                        items.rotate_left(usize::from(k) % len);
                    }
                });
            }
        }
        self.check_feed(c);
    }

    /// What the device shows must never refer to a chat it does not have (§3.6).
    fn check_feed(&mut self, c: usize) {
        let Some(client) = self.clients.get(c) else {
            return;
        };
        for item in client.feed() {
            if let FeedSlot::Orphan(chat) = &item.slot {
                self.violations.push(format!("device {c}: message {} is shown in chat {chat}, which this device does not have", item.message));
            }
        }
    }

    fn drain_writes(&mut self) {
        for client in &mut self.clients {
            for write in client.writes.drain(..) {
                if let Some(problem) = self.oracle.record(write) {
                    self.violations.push(problem);
                }
            }
        }
    }

    /// Everyone pulls to the head and pushes everything, over a healthy network, until nothing changes (spec: the network quiesces).
    pub fn quiesce(&mut self) -> Result<(), String> {
        self.sync_clocks();
        for c in 0..self.clients.len() {
            let generation = self.server.generation();
            if let Some(client) = self.clients.get_mut(c) {
                client.begin_binding(generation);
            }
        }
        let mut previous = String::new();
        for _round in 0..60 {
            for c in 0..self.clients.len() {
                self.sync_device(c);
                self.drain_writes();
            }
            let mark = self.world_mark();
            let calm = mark == previous
                && self.server.settled()
                && self
                    .clients
                    .iter()
                    .all(|c| !c.has_pending_work() && c.resync.is_none());
            if calm {
                return Ok(());
            }
            previous = mark;
        }
        Err("the network did not quiesce in 60 rounds".to_string())
    }

    fn client_mark(&self, c: usize) -> String {
        self.clients
            .get(c)
            .map(|cl| {
                format!(
                    "{}:{}:{}",
                    cl.cursor_generation,
                    cl.cursor_seq,
                    cl.records.len()
                )
            })
            .unwrap_or_default()
    }

    fn world_mark(&self) -> String {
        let mut out = format!(
            "{}:{}:{}",
            self.server.generation(),
            self.server.issued(),
            self.server.log_len()
        );
        for client in &self.clients {
            out.push_str(&format!(
                "|{}:{}:{:?}",
                client.cursor_seq,
                client.outbox.len(),
                client.state()
            ));
        }
        out
    }
}
