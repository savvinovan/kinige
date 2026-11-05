//! Spike S4b (KINIG-96): the account flows of ADR-0001 §8 against an in-memory account server, with the real keys, wraps, words and
//! envelopes of `kinige-crypto`. The server and the client here are the simulation; what they exchange is real bytes.

pub mod client;
pub mod leak;
pub mod link;
pub mod log;
pub mod server;
