//! Compaction on Postgres (ADR-0001 §6.1, §10.2): `rules::compact::plan` decides from the log's open metadata, this applies the plan
//! under the account lock, so a push of the same account cannot interleave with it. A compaction that removes a `purged` is a drop:
//! the account's epoch grows by one and the drop is stored with the generation it was made in and the time (ADR-0017).

use super::envelopes::{current_generation, drops_of};
use super::{to_i64, to_u64};
use crate::error::ApiError;
use crate::rules::compact::{Meta, plan};
use deadpool_postgres::Pool;
use kinige_crypto::header::Kind;
use std::collections::BTreeMap;
use tokio_postgres::types::Json;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Envelopes removed from the log.
    pub removed: usize,
    /// Whether a `purged` was dropped, so the epoch grew.
    pub dropped: bool,
}

/// Compacts one account at `now_ms`.
pub async fn account(pool: &Pool, account: &str, now_ms: u64) -> Result<Stats, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT epoch FROM accounts WHERE id = $1 FOR UPDATE",
            &[&account],
        )
        .await?
        .ok_or(ApiError::Auth)?;
    let epoch = to_u64(row.try_get("epoch")?);
    let rows = tx
        .query(
            "SELECT seq, at_ms, entity, entity_id, kind, groups FROM envelopes WHERE account_id = $1 ORDER BY seq",
            &[&account],
        )
        .await?;
    let metas: Vec<Meta> = rows
        .iter()
        .map(|row| {
            let Json(groups): Json<BTreeMap<String, String>> = row.try_get("groups")?;
            let kind: String = row.try_get("kind")?;
            Ok(Meta {
                seq: to_u64(row.try_get("seq")?),
                at_ms: to_u64(row.try_get("at_ms")?),
                entity: row.try_get("entity")?,
                id: row.try_get("entity_id")?,
                kind: if kind == "purged" {
                    Kind::Purged
                } else {
                    Kind::Upsert
                },
                groups,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let plan = plan(&metas, now_ms);
    if plan.remove.is_empty() {
        return Ok(Stats::default());
    }
    let dropped = plan.dropped_max > 0;
    if dropped {
        let generation = current_generation(&tx).await?;
        let new_epoch = epoch.saturating_add(1);
        tx.execute(
            "UPDATE accounts SET epoch = $2 WHERE id = $1",
            &[&account, &to_i64(new_epoch, "epoch")?],
        )
        .await?;
        tx.execute(
            "INSERT INTO drops (account_id, generation, epoch, seq, compacted_ms) VALUES ($1, $2, $3, $4, $5)",
            &[
                &account,
                &to_i64(generation, "generation")?,
                &to_i64(new_epoch, "epoch")?,
                &to_i64(plan.dropped_max, "seq")?,
                &to_i64(now_ms, "clock")?,
            ],
        )
        .await?;
    }
    for (entity, id) in &plan.forgotten {
        for table in ["purged_entities", "entity_known"] {
            tx.execute(
                &format!(
                    "DELETE FROM {table} WHERE account_id = $1 AND entity = $2 AND entity_id = $3"
                ),
                &[&account, entity, id],
            )
            .await?;
        }
    }
    let seqs = plan
        .remove
        .iter()
        .map(|seq| to_i64(*seq, "seq"))
        .collect::<Result<Vec<i64>, ApiError>>()?;
    tx.execute(
        "DELETE FROM envelopes WHERE account_id = $1 AND seq = ANY($2)",
        &[&account, &seqs],
    )
    .await?;
    tx.commit().await?;
    log::info!("compaction done");
    Ok(Stats {
        removed: plan.remove.len(),
        dropped,
    })
}

/// Compacts every account at `now_ms`; returns the number of envelopes removed. One account's failure does not stop the others.
pub async fn all(pool: &Pool, now_ms: u64) -> Result<usize, ApiError> {
    let client = pool.get().await?;
    let rows = client.query("SELECT id FROM accounts", &[]).await?;
    drop(client);
    let mut removed = 0;
    for row in rows {
        let id: String = row.try_get("id")?;
        match account(pool, &id, now_ms).await {
            Ok(stats) => removed += stats.removed,
            Err(_) => log::warn!("compaction of one account failed"),
        }
    }
    Ok(removed)
}

/// The drops of an account, for the tests and for S4c-2's dump.
pub async fn drops(pool: &Pool, account: &str) -> Result<Vec<crate::rules::pull::Drop>, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    drops_of(&tx, account).await
}
