//! The line between a client and the server: every call can be lost on the way there (the server never sees it) or on the way back
//! (the server did it and the client never learns). A client that "crashes between its steps" is a client whose answer was lost, or
//! one that is rebuilt from its persisted store (`Client::restart`) between two calls.

use crate::server::Server;
use std::collections::VecDeque;

/// What happens to the next call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The request is lost: the server does not see it.
    LoseRequest,
    /// The server handles the request and its answer is lost.
    LoseReply,
}

/// The client got no answer. It cannot tell the two faults apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lost;

pub struct Link<'a> {
    server: &'a mut Server,
    faults: VecDeque<Option<Fault>>,
    /// Requests the client sent, lost ones included: the "Save" gate must keep this at zero.
    sent: usize,
}

impl<'a> Link<'a> {
    pub fn new(server: &'a mut Server) -> Link<'a> {
        Link {
            server,
            faults: VecDeque::new(),
            sent: 0,
        }
    }

    /// The faults of the next calls, in order; `None` is a call that works. Afterwards calls work.
    pub fn with_faults(
        server: &'a mut Server,
        faults: impl IntoIterator<Item = Option<Fault>>,
    ) -> Link<'a> {
        Link {
            server,
            faults: faults.into_iter().collect(),
            sent: 0,
        }
    }

    pub fn call<T>(&mut self, request: impl FnOnce(&mut Server) -> T) -> Result<T, Lost> {
        self.sent += 1;
        match self.faults.pop_front().flatten() {
            Some(Fault::LoseRequest) => Err(Lost),
            Some(Fault::LoseReply) => {
                let _ = request(self.server);
                Err(Lost)
            }
            None => Ok(request(self.server)),
        }
    }

    pub fn sent(&self) -> usize {
        self.sent
    }

    pub fn server(&mut self) -> &mut Server {
        self.server
    }
}
