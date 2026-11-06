//! The periodic work of the process: compaction of every account's log (ADR-0001 §6.1, §10.2) and the sweep of deleted blobs and
//! unfinished uploads (ADR-0020), on the server's clock.

use crate::blobs::sweep;
use crate::state::AppState;
use crate::store::compact;
use std::time::Duration;

/// Compacts every account and sweeps the blobs every `every_secs` seconds, forever. The first run is after one period, not at start.
pub async fn compact_loop(state: AppState, every_secs: u64) {
    let mut ticker = tokio::time::interval(Duration::from_secs(every_secs.max(1)));
    ticker.tick().await;
    loop {
        ticker.tick().await;
        match compact::all(&state.pool, &*state.clock).await {
            Ok(removed) => log::info!("compaction pass done, {removed} envelopes removed"),
            Err(_) => log::warn!("compaction pass failed"),
        }
        match sweep::run(&state.pool, &state.blobs, &state.config, &*state.clock).await {
            Ok(swept) => log::info!(
                "blob sweep done, {} objects removed, {} unfinished uploads removed, {} orphans adopted, {} retired journal entries removed",
                swept.removed,
                swept.uploads,
                swept.adopted,
                swept.journal
            ),
            Err(_) => log::warn!("blob sweep failed"),
        }
    }
}
