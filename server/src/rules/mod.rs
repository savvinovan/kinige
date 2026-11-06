//! The decisions of the server as plain functions with no IO (spec: "SQL only stores and locks"): the push check order
//! (ADR-0001 §A.4), the drop-missed rule with generations and a lineage (§6.3), and the compaction plan (§10.2).

pub mod compact;
pub mod pull;
pub mod push;
