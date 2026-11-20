//! The details of an attachment (slice 7b, ADR-0001 appendix §C.8): a display name, a description and a spoiler flag.
//!
//! A command takes changes, not the whole triple. Rust normalizes each field that was sent and writes a group only for a field whose
//! normalized value differs from the stored one; equal values write nothing. On a row written before 7b a missing group is added
//! (`kinige_domain::add_group`), and no code path writes a default-valued group to an existing row. The columns are the read
//! function's output for the new bytes. No revision and no `edited_at`: the entry's own groups are untouched.

use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use crate::send::mark_dirty;
use crate::view::load_view;
use crate::{Now, Store, StoreError};
use kinige_domain::add_group::rewrite_or_add_field;
use kinige_domain::attachment::{
    AttachmentDetailsChanges, AttachmentKind, AttachmentView, DESCRIPTION_GROUP, Details,
    NAME_GROUP, PRIVACY_GROUP, check_kind, normalize_description, normalize_name, read_details,
};
use kinige_domain::hlc::is_uuid_v4;
use kinige_domain::rewrite::{Groups, is_read_only};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{Value, json};
use std::sync::Arc;

impl Store {
    /// "Details" on a tile of the input field: a local write to `draft_attachment`, no HLC (the groups are made when the entry is sent).
    /// No such tile is `AttachmentNotFound`; a rule broken is `AttachmentDetailsInvalid`. Nothing is written on an error.
    pub fn set_draft_attachment_details(
        &self,
        attachment_id: &str,
        changes: &AttachmentDetailsChanges,
    ) -> Result<AttachmentView, StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        let (id, changes) = (attachment_id.to_string(), changes.clone());
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let (key, kind, filename, current) = load_draft(&tx, &id)?;
            let plan = plan_changes(kind, &filename, &current, &changes)?;
            let now = Details {
                display_name: plan.name.clone().unwrap_or(current.display_name),
                description: plan.description.clone().unwrap_or(current.description),
                spoiler: plan.spoiler.unwrap_or(current.spoiler),
            };
            tx.execute(
                "UPDATE draft_attachment SET display_name = ?1, description = ?2, spoiler = ?3 WHERE attachment_id = ?4",
                (&now.display_name, &now.description, now.spoiler, &id),
            )
            .map_err(write_err)?;
            let view = crate::attachment::draft_views(&tx, &key)?
                .into_iter()
                .find(|view| view.id == id)
                .ok_or(StoreError::AttachmentNotFound)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })
    }

    /// "Details" of a sent attachment (also "Hide as spoiler" from the context menu, which sends only the spoiler). One writer
    /// transaction: a changed field whose group exists is rewritten under a new HLC strictly above the old one; on an older row the
    /// group is added; the entry is re-indexed when the name or the description changed, and its day is marked for the Markdown copy.
    ///
    /// Errors, nothing written: `AttachmentNotFound` (not a UUID, no such attachment), `MessageInTrash`, `ReadOnly` (a group in an
    /// unsupported `sv`), `AttachmentDetailsInvalid`.
    pub fn set_attachment_details(
        &self,
        attachment_id: &str,
        changes: &AttachmentDetailsChanges,
        now: &Now,
    ) -> Result<AttachmentView, StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        let (id, changes, now) = (attachment_id.to_string(), changes.clone(), now.clone());
        let faults = Arc::clone(&self.inner.faults);
        let (view, listed) = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let target = load_target(&tx, &id)?;
            if is_read_only(&target.hlc_json)? {
                return Err(StoreError::ReadOnly);
            }
            let current = read_details(
                target.kind,
                &target.filename,
                &target.groups_raw,
                &target.hlc_json,
            );
            let plan = plan_changes(target.kind, &target.filename, &current, &changes)?;
            let listed = plan.name.is_some() || plan.description.is_some();
            if plan.is_empty() {
                let view = view_of(&tx, &target.message_id, &id)?;
                return Ok((view, false));
            }
            let (groups_raw, hlc_json, schema_version) = write_groups(&tx, &target, &plan, &now)?;
            let after = read_details(target.kind, &target.filename, &groups_raw, &hlc_json);
            tx.execute(
                "UPDATE attachment SET groups_raw = ?1, hlc_json = ?2, schema_version = ?3, display_name = ?4, \
                 description = ?5, spoiler = ?6 WHERE id = ?7",
                rusqlite::params![
                    groups_raw,
                    hlc_json,
                    schema_version,
                    after.display_name,
                    after.description,
                    after.spoiler,
                    id
                ],
            )
            .map_err(write_err)?;
            if listed {
                // The index and the copy depend on the name and the description only; hiding or showing a photo changes neither.
                crate::search::reindex(&tx, &target.message_id, &target.body_md).map_err(write_err)?;
                mark_dirty(&tx, &target.chat_id, &target.journal_date).map_err(write_err)?;
            }
            faults.run_before_details_commit();
            let view = view_of(&tx, &target.message_id, &id)?;
            tx.commit().map_err(write_err)?;
            Ok((view, listed))
        })?;
        if listed {
            self.wake_mirror();
        }
        Ok(view)
    }
}

/// The fields that changed, normalized: `None` is "leave", `Some(value)` is the new value.
#[derive(Debug, Default)]
struct Plan {
    name: Option<Option<String>>,
    description: Option<Option<String>>,
    spoiler: Option<bool>,
}

impl Plan {
    fn is_empty(&self) -> bool {
        self.name.is_none() && self.description.is_none() && self.spoiler.is_none()
    }
}

/// Normalizes what was sent and keeps only what differs from `current`. What a kind takes is `check_kind`'s rule: the spoiler belongs to
/// an `image`, an `audio` takes nothing.
fn plan_changes(
    kind: AttachmentKind,
    original: &str,
    current: &Details,
    changes: &AttachmentDetailsChanges,
) -> Result<Plan, StoreError> {
    check_kind(kind, changes)?;
    let mut plan = Plan::default();
    if let Some(sent) = &changes.name {
        let name = match sent {
            Some(stem) => normalize_name(stem, original)?,
            None => None,
        };
        plan.name = (name != current.display_name).then_some(name);
    }
    if let Some(sent) = &changes.description {
        let text = match sent {
            Some(text) => normalize_description(text)?,
            None => None,
        };
        plan.description = (text != current.description).then_some(text);
    }
    if let Some(spoiler) = changes.spoiler {
        plan.spoiler = (spoiler != current.spoiler).then_some(spoiler);
    }
    Ok(plan)
}

/// Writes the planned groups into the bytes, one new HLC per group (each strictly above everything before it).
fn write_groups(
    tx: &Transaction<'_>,
    target: &Target,
    plan: &Plan,
    now: &Now,
) -> Result<(String, String, u32), StoreError> {
    let steps: [(&str, Option<(&str, Value)>); 3] = [
        (NAME_GROUP, plan.name.as_ref().map(|v| ("name", json!(v)))),
        (
            DESCRIPTION_GROUP,
            plan.description.as_ref().map(|v| ("description", json!(v))),
        ),
        (
            PRIVACY_GROUP,
            plan.spoiler.map(|flag| ("spoiler", json!(flag))),
        ),
    ];
    let (mut raw, mut hlcs, mut schema_version) = (
        target.groups_raw.clone(),
        target.hlc_json.clone(),
        target.schema_version,
    );
    for (group, field) in steps {
        let Some((key, value)) = field else {
            continue;
        };
        let hlc = next_hlc(tx, now.ms)?;
        let current = Groups {
            groups_raw: &raw,
            hlc_json: &hlcs,
        };
        let done = rewrite_or_add_field(&current, "attachment", group, &hlc, None, (key, value))?;
        (raw, hlcs, schema_version) = (done.groups_raw, done.hlc_json, done.schema_version);
    }
    Ok((raw, hlcs, schema_version))
}

/// A sent attachment and its entry, as the edit sees them.
struct Target {
    message_id: String,
    chat_id: String,
    journal_date: String,
    body_md: String,
    kind: AttachmentKind,
    filename: String,
    groups_raw: String,
    hlc_json: String,
    schema_version: u32,
}

fn load_target(tx: &Transaction<'_>, id: &str) -> Result<Target, StoreError> {
    let found = tx
        .query_row(
            "SELECT a.message_id, a.kind, a.filename, a.groups_raw, a.hlc_json, a.schema_version, a.deleted_at IS NOT NULL, \
             m.chat_id, m.journal_date, m.body_md, m.deleted_at IS NOT NULL \
             FROM attachment a LEFT JOIN message m ON m.id = a.message_id WHERE a.id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<bool>>(10)?,
                ))
            },
        )
        .optional()
        .map_err(read_err)?;
    let Some((
        message_id,
        kind,
        filename,
        groups_raw,
        hlc_json,
        schema_version,
        removed,
        chat,
        date,
        body,
        in_trash,
    )) = found
    else {
        return Err(StoreError::AttachmentNotFound);
    };
    let (Some(chat_id), Some(journal_date), Some(body_md)) = (chat, date, body) else {
        return Err(StoreError::AttachmentNotFound);
    };
    if removed {
        return Err(StoreError::AttachmentNotFound);
    }
    if in_trash == Some(true) {
        return Err(StoreError::MessageInTrash);
    }
    Ok(Target {
        message_id,
        chat_id,
        journal_date,
        body_md,
        kind: AttachmentKind::parse(&kind).ok_or(StoreError::AttachmentNotFound)?,
        filename,
        groups_raw,
        hlc_json,
        schema_version,
    })
}

/// A draft attachment's key, kind, original name and current details.
fn load_draft(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<(String, AttachmentKind, String, Details), StoreError> {
    let (key, kind, filename, display_name, description, spoiler): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
    ) = tx
        .query_row(
            "SELECT draft_key, kind, filename, display_name, description, spoiler FROM draft_attachment WHERE attachment_id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(read_err)?
        .ok_or(StoreError::AttachmentNotFound)?;
    let kind = AttachmentKind::parse(&kind).ok_or(StoreError::AttachmentNotFound)?;
    let details = Details {
        display_name,
        description,
        spoiler: spoiler != 0 && kind == AttachmentKind::Image,
    };
    Ok((key, kind, filename, details))
}

/// The attachment as the interface sees it, from the entry's view.
fn view_of(tx: &Transaction<'_>, message_id: &str, id: &str) -> Result<AttachmentView, StoreError> {
    load_view(tx, message_id)?
        .attachments
        .into_iter()
        .find(|view| view.id == id)
        .ok_or(StoreError::AttachmentNotFound)
}
