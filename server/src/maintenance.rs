//! The periodic work of the process: compaction of every account's log (ADR-0001 §6.1, §10.2) on the server's clock.

use crate::state::AppState;
use crate::store::compact;
use std::time::Duration;

/// Compacts every account every `every_secs` seconds, forever. The first run is after one period, not at start.
pub async fn compact_loop(state: AppState, every_secs: u64) {
    let mut ticker = tokio::time::interval(Duration::from_secs(every_secs.max(1)));
    ticker.tick().await;
    loop {
        ticker.tick().await;
        match compact::all(&state.pool, state.clock.now_ms()).await {
            Ok(removed) => log::info!("compaction pass done, {removed} envelopes removed"),
            Err(_) => log::warn!("compaction pass failed"),
        }
    }
}
