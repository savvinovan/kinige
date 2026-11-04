//! Rewriting an entry's `body` group inside an open transaction: a text edit (with its `edit` revision) and the removal of the
//! Telegram placeholder (without one). The rest of the entry actions are in `messages.rs`.

use crate::error::write_err;
use crate::hlc_state::next_hlc;
use crate::messages::{Loaded, apply};
use crate::send::mark_dirty;
use crate::{Now, StoreError};
use kinige_domain::append::{CurrentBody, rewrite_body};
use kinige_domain::history::edit_revision;
use kinige_domain::rewrite::Rewritten;
use rusqlite::Transaction;
use uuid::Uuid;

/// A text edit inside an open transaction: the `edit` revision (a snapshot before the edit), the new `body` group, tags, the copy mark.
/// The same text -- nothing is written.
pub(crate) fn edit_body(
    tx: &Transaction<'_>,
    loaded: &Loaded,
    body_md: &str,
    now: &Now,
) -> Result<(), StoreError> {
    replace_body(tx, loaded, body_md, now, true)
}

/// Removes the Telegram placeholder from an entry that has just got its file (slice 7, ADR-0014): the usual rewrite of `body` (new
/// HLC, `base`/`supersedes`, `edited_at`, tags, search, copy mark) but **no `edit` revision**, because the old text is the new text
/// plus one known line. The caller has checked that the entry is unedited since import.
pub(crate) fn replace_body_without_revision(
    tx: &Transaction<'_>,
    loaded: &Loaded,
    body_md: &str,
    now: &Now,
) -> Result<(), StoreError> {
    replace_body(tx, loaded, body_md, now, false)
}

fn replace_body(
    tx: &Transaction<'_>,
    loaded: &Loaded,
    body_md: &str,
    now: &Now,
    with_revision: bool,
) -> Result<(), StoreError> {
    if loaded.columns.body_md == body_md {
        return Ok(());
    }
    let hlc = next_hlc(tx, now.ms)?;
    let revision = if with_revision {
        Some(edit_revision(
            &Uuid::new_v4().to_string(),
            &loaded.id,
            &loaded.groups_raw,
            &loaded.hlc_json,
            now.ms,
            &hlc,
        )?)
    } else {
        None
    };
    let rewritten = rewrite_body(
        &CurrentBody {
            groups_raw: &loaded.groups_raw,
            hlc_json: &loaded.hlc_json,
            // The outbox appears in slice 11: then "confirmed" is both a column and an outbox row (ADR-0001 §3.4).
            body_confirmed: loaded.server_known,
        },
        body_md,
        now.ms,
        &hlc,
    )?;
    let groups = Rewritten {
        groups_raw: rewritten.groups_raw,
        hlc_json: rewritten.hlc_json,
        schema_version: rewritten.schema_version,
    };
    apply(tx, &loaded.id, &groups)?;
    if let Some(revision) = revision {
        tx.execute(
            "INSERT INTO message_revision (id, message_id, snapshot_json, saved_at, reason, hlc_json, server_known, \
             schema_version, groups_raw) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
            rusqlite::params![
                revision.id,
                revision.message_id,
                revision.snapshot_json,
                revision.saved_at,
                revision.reason,
                revision.hlc_json,
                revision.schema_version,
                revision.groups_raw
            ],
        )
        .map_err(write_err)?;
    }
    crate::tags::replace_tags(tx, &loaded.id, body_md).map_err(write_err)?;
    crate::search::reindex(tx, &loaded.id, body_md).map_err(write_err)?;
    mark_dirty(tx, &loaded.columns.chat_id, &loaded.columns.journal_date).map_err(write_err)?;
    Ok(())
}
