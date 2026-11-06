//! Push and pull on Postgres (ADR-0001 §6.1, §A.3, §A.4). The queries read the facts, `rules::push::decide` and `rules::pull::plan`
//! decide, and the queries write.
//!
//! **`seq` is issued under the account lock, in the transaction of the write.** The first statement of a push is
//! `SELECT ... FROM accounts WHERE id = $1 FOR UPDATE`; the next push of the account waits for it until the first commits, so no
//! transaction can take `n + 1` while `n` is uncommitted and no reader sees a gap that fills in later. Pushes of other accounts take
//! other rows and do not wait. A pull reads in one snapshot (`REPEATABLE READ`), bounded by the head it read in that snapshot.

use super::{fixed, to_i64, to_u64};
use crate::error::ApiError;
use crate::rules::pull::{self, Cursor, Drop, Generation, Plan, page_budget};
use crate::rules::push::{Answer, Facts, OpRecord, PurgedRecord, Verdict, decide};
use crate::wire::Frame;
use deadpool_postgres::{Pool, Transaction};
use kinige_crypto::header::{Header, Kind};
use kinige_domain::sync::Snapshot;
use libsodium_rs::crypto_generichash::generichash;
use std::collections::BTreeMap;
use tokio_postgres::IsolationLevel;
use tokio_postgres::types::Json;

/// What the request says about itself, the same for every envelope of a batch.
#[derive(Debug, Clone)]
pub struct PushContext {
    pub now_ms: u64,
    pub protocol_ok: bool,
    pub cursor: Cursor,
}

/// A page of the log, or why there is none.
#[derive(Debug)]
pub struct PullOut {
    /// The current generation, the whole lineage with the cut times (the device keeps it, §6.3) and the account's drop epoch.
    pub generation: u64,
    pub lineage: Vec<Generation>,
    pub epoch: u64,
    pub plan: Plan,
    /// Empty unless the plan is `Page`.
    pub items: Vec<Pulled>,
    pub next: u64,
    pub head: u64,
}

#[derive(Debug)]
pub struct Pulled {
    pub seq: u64,
    pub entity: String,
    pub id: String,
    pub op: String,
    pub bytes: Vec<u8>,
}

/// One envelope of a push. Steps 3-7 of §A.4 are decided in `rules::push`, step 8 is here.
pub async fn push_one(
    pool: &Pool,
    account: &str,
    frame: &Frame,
    ctx: &PushContext,
) -> Result<Answer, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let head = lock_head(&tx, account).await?;
    let header = &frame.header;
    let bytes_hash = hash_bytes(&frame.bytes)?;
    let op = op_record(&tx, account, &header.op).await?;
    let purged = purged_record(&tx, account, header).await?;
    let known = known_groups(&tx, account, header).await?;
    let lineage = lineage(&tx).await?;
    let drops = if ctx.cursor.after > 0 {
        drops_of(&tx, account).await?
    } else {
        Vec::new()
    };
    let cursor_stale = !matches!(pull::plan(&lineage, &drops, &ctx.cursor), Plan::Page);
    let facts = Facts {
        header,
        bytes_hash,
        op: op.as_ref(),
        protocol_ok: ctx.protocol_ok,
        cursor_stale,
        now_ms: ctx.now_ms,
        purged: purged.as_ref(),
        known: &known,
    };
    match decide(&facts) {
        Verdict::Answer(answer) => Ok(answer),
        Verdict::Accept => {
            let seq = head.saturating_add(1);
            write(&tx, account, frame, seq, &bytes_hash, &known, ctx.now_ms).await?;
            tx.commit().await?;
            Ok(Answer::Ok {
                seq,
                purged_of: None,
            })
        }
    }
}

/// One page, in one snapshot. `page_bytes` stops the page once that many bytes are in it (the first envelope always goes).
pub async fn pull(
    pool: &Pool,
    account: &str,
    cursor: &Cursor,
    limit: usize,
    page_bytes: usize,
) -> Result<PullOut, ApiError> {
    let mut client = pool.get().await?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    let row = tx
        .query_opt(
            "SELECT seq_head, epoch FROM accounts WHERE id = $1",
            &[&account],
        )
        .await?
        .ok_or(ApiError::Auth)?;
    let head = to_u64(row.try_get("seq_head")?);
    let epoch = to_u64(row.try_get("epoch")?);
    let lineage = lineage(&tx).await?;
    let drops = drops_of(&tx, account).await?;
    let plan = pull::plan(&lineage, &drops, cursor);
    let generation = lineage.last().map_or(0, |g| g.id);
    let mut out = PullOut {
        generation,
        lineage: lineage.clone(),
        epoch,
        plan,
        items: Vec::new(),
        next: head.max(cursor.after),
        head,
    };
    if plan != Plan::Page {
        return Ok(out);
    }
    let (limit, page_bytes) = page_budget(limit, page_bytes);
    out.items = page(&tx, account, cursor.after, head, limit, page_bytes).await?;
    if let Some(last) = out.items.last().map(|item| item.seq) {
        let more = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM envelopes WHERE account_id = $1 AND seq > $2 AND seq <= $3)",
                &[&account, &to_i64(last, "seq")?, &to_i64(head, "seq")?],
            )
            .await?;
        if more.try_get::<_, bool>(0)? {
            out.next = last;
        }
    }
    Ok(out)
}

/// The head `seq` of the account, with the account row locked until the end of the transaction. `Auth` if the account is gone.
async fn lock_head(tx: &Transaction<'_>, account: &str) -> Result<u64, ApiError> {
    let row = tx
        .query_opt(
            "SELECT seq_head FROM accounts WHERE id = $1 FOR UPDATE",
            &[&account],
        )
        .await?
        .ok_or(ApiError::Auth)?;
    Ok(to_u64(row.try_get("seq_head")?))
}

async fn op_record(
    tx: &Transaction<'_>,
    account: &str,
    op: &str,
) -> Result<Option<OpRecord>, ApiError> {
    let row = tx
        .query_opt(
            "SELECT seq, bytes_hash FROM ops WHERE account_id = $1 AND op_id = $2",
            &[&account, &op],
        )
        .await?;
    row.map(|row| {
        Ok(OpRecord {
            seq: to_u64(row.try_get("seq")?),
            bytes_hash: fixed(&row, "bytes_hash")?,
        })
    })
    .transpose()
}

async fn purged_record(
    tx: &Transaction<'_>,
    account: &str,
    header: &Header,
) -> Result<Option<PurgedRecord>, ApiError> {
    let row = tx
        .query_opt(
            "SELECT seq, purged_of FROM purged_entities WHERE account_id = $1 AND entity = $2 AND entity_id = $3",
            &[&account, &header.entity, &header.id],
        )
        .await?;
    row.map(|row| {
        let Json(purged_of): Json<Snapshot> = row.try_get("purged_of")?;
        Ok(PurgedRecord {
            seq: to_u64(row.try_get("seq")?),
            purged_of,
        })
    })
    .transpose()
}

async fn known_groups(
    tx: &Transaction<'_>,
    account: &str,
    header: &Header,
) -> Result<Snapshot, ApiError> {
    let rows = tx
        .query(
            "SELECT grp, hlc FROM entity_known WHERE account_id = $1 AND entity = $2 AND entity_id = $3",
            &[&account, &header.entity, &header.id],
        )
        .await?;
    rows.iter()
        .map(|row| Ok((row.try_get("grp")?, row.try_get("hlc")?)))
        .collect()
}

pub(super) async fn lineage(tx: &Transaction<'_>) -> Result<Vec<Generation>, ApiError> {
    let rows = tx
        .query("SELECT generation, cut_ms FROM lineage ORDER BY pos", &[])
        .await?;
    rows.iter()
        .map(|row| {
            Ok(Generation {
                id: to_u64(row.try_get("generation")?),
                cut_ms: to_u64(row.try_get("cut_ms")?),
            })
        })
        .collect()
}

pub(super) async fn drops_of(tx: &Transaction<'_>, account: &str) -> Result<Vec<Drop>, ApiError> {
    let rows = tx
        .query(
            "SELECT generation, epoch, seq, compacted_ms FROM drops WHERE account_id = $1",
            &[&account],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(Drop {
                generation: to_u64(row.try_get("generation")?),
                epoch: to_u64(row.try_get("epoch")?),
                seq: to_u64(row.try_get("seq")?),
                compacted_ms: to_u64(row.try_get("compacted_ms")?),
            })
        })
        .collect()
}

pub(super) async fn current_generation(tx: &Transaction<'_>) -> Result<u64, ApiError> {
    Ok(lineage(tx).await?.last().map_or(0, |g| g.id))
}

/// Step 8: the envelope, its `op_id`, the entity's index, and the new head, all in the transaction that holds the account lock.
async fn write(
    tx: &Transaction<'_>,
    account: &str,
    frame: &Frame,
    seq: u64,
    bytes_hash: &[u8; 32],
    known: &Snapshot,
    now_ms: u64,
) -> Result<(), ApiError> {
    let header = &frame.header;
    let seq_db = to_i64(seq, "seq")?;
    let at = to_i64(now_ms, "clock")?;
    let len = i32::try_from(frame.bytes.len()).map_err(|_| ApiError::TooLarge)?;
    let groups: BTreeMap<&str, &str> = header
        .groups
        .iter()
        .map(|(group, stamp)| (group.as_str(), stamp.h.as_str()))
        .collect();
    let kind = match header.kind {
        Kind::Upsert => "upsert",
        Kind::Purged => "purged",
    };
    tx.execute(
        "INSERT INTO envelopes (account_id, seq, op_id, entity, entity_id, kind, groups, at_ms, len, bytes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            &account,
            &seq_db,
            &header.op,
            &header.entity,
            &header.id,
            &kind,
            &Json(&groups),
            &at,
            &len,
            &frame.bytes,
        ],
    )
    .await?;
    tx.execute(
        "INSERT INTO ops (account_id, op_id, seq, bytes_hash) VALUES ($1, $2, $3, $4)",
        &[&account, &header.op, &seq_db, &bytes_hash.as_slice()],
    )
    .await?;
    match header.kind {
        Kind::Upsert => {
            for (group, stamp) in &header.groups {
                if known.get(group).is_none_or(|held| stamp.h > *held) {
                    tx.execute(
                        "INSERT INTO entity_known (account_id, entity, entity_id, grp, hlc) VALUES ($1, $2, $3, $4, $5) \
                         ON CONFLICT (account_id, entity, entity_id, grp) DO UPDATE SET hlc = EXCLUDED.hlc",
                        &[&account, &header.entity, &header.id, group, &stamp.h],
                    )
                    .await?;
                }
            }
        }
        Kind::Purged => {
            tx.execute(
                "INSERT INTO purged_entities (account_id, entity, entity_id, seq, purged_of) VALUES ($1, $2, $3, $4, $5)",
                &[&account, &header.entity, &header.id, &seq_db, &Json(&header.purged_of)],
            )
            .await?;
            // A purge of an attachment names its blob in the open header (§5.2: the id is in the encrypted `core`, so the purging
            // device copies it there). The accepted purge marks the blob deleted, in the same transaction; the object leaves the
            // store after the backup retention (ADR-0020). Only the pusher's own account's blob can be named.
            if let Some(blob) = &header.blob {
                tx.execute(
                    "UPDATE blobs SET deleted_ms = $3 WHERE account_id = $1 AND blob_id = $2 AND deleted_ms IS NULL",
                    &[&account, blob, &at],
                )
                .await?;
            }
        }
    }
    tx.execute(
        "UPDATE accounts SET seq_head = $2 WHERE id = $1",
        &[&account, &seq_db],
    )
    .await?;
    Ok(())
}

async fn page(
    tx: &Transaction<'_>,
    account: &str,
    after: u64,
    head: u64,
    limit: usize,
    page_bytes: usize,
) -> Result<Vec<Pulled>, ApiError> {
    let rows = tx
        .query(
            "SELECT seq, entity, entity_id, op_id, bytes FROM ( \
                 SELECT seq, entity, entity_id, op_id, bytes, sum(len) OVER (ORDER BY seq) - len AS before \
                 FROM (SELECT * FROM envelopes WHERE account_id = $1 AND seq > $2 AND seq <= $3 ORDER BY seq LIMIT $4) w \
             ) t WHERE before < $5 ORDER BY seq",
            &[
                &account,
                &to_i64(after, "after")?,
                &to_i64(head, "head")?,
                &i64::try_from(limit).unwrap_or(i64::MAX),
                &i64::try_from(page_bytes).unwrap_or(i64::MAX),
            ],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(Pulled {
                seq: to_u64(row.try_get("seq")?),
                entity: row.try_get("entity")?,
                id: row.try_get("entity_id")?,
                op: row.try_get("op_id")?,
                bytes: row.try_get("bytes")?,
            })
        })
        .collect()
}

/// `crypto_generichash` of the envelope bytes (32 bytes): what the server remembers of an `op_id`'s bytes.
fn hash_bytes(bytes: &[u8]) -> Result<[u8; 32], ApiError> {
    let hash = generichash(bytes, None, 32).map_err(|_| ApiError::Internal)?;
    <[u8; 32]>::try_from(hash.as_slice()).map_err(|_| ApiError::Internal)
}
