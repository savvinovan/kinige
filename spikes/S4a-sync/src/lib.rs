//! Spike S4a (KINIG-94): a simulation of the sync protocol of ADR-0001, with 2 or 3 clients and an in-memory server, under delayed,
//! duplicated, reordered and dropped delivery. The rules under test are the real ones in `kinige_domain::sync`; this crate is the
//! server, the clients, the network, the clock, the oracle and the generators around them.

pub mod backend;
pub mod client;
pub mod directed;
pub mod envelope;
#[cfg(feature = "real-server")]
pub mod http_server;
pub mod oracle;
pub mod props;
#[cfg(feature = "real-envelope")]
pub mod real;
pub mod schedules;
pub mod server;
pub mod switches;
pub mod world;
