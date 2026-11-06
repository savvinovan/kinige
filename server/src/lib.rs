//! The Kinige sync server core (spike S4c-1, KINIG-98; ADR-0001 §3.5, §6, §8, appendix §A.3-§A.6; ADR-0019): accounts, sessions,
//! push, pull and compaction on axum and Postgres.
//!
//! Every decision the server makes is a plain function in [`rules`] with unit tests and no database; the [`store`] only stores and
//! locks. `seq` is issued under the account's row lock, in the transaction of the write (ADR-0001 §10.2, §B item 23). The server
//! reads the open header of an envelope and never anything inside it (§10).
//!
//! Logging is event kinds and error categories only. Request bodies, tokens, keys and envelope bytes never reach the log (§8.8).

pub mod api;
pub mod clock;
pub mod config;
pub mod dto;
pub mod error;
pub mod logging;
pub mod maintenance;
pub mod rate;
pub mod rules;
pub mod state;
pub mod store;
pub mod wire;

#[cfg(feature = "testing")]
pub mod testing;

pub use api::router;
pub use clock::{Clock, ManualClock, SystemClock};
pub use config::Config;
pub use maintenance::compact_loop;
pub use state::AppState;
