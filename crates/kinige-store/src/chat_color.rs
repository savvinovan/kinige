//! "Colour" in the chat menu (KINIG-59): an explicit pick rewrites the `meta` group (`meta.color`), the way a rename or an emoji
//! change does. Chats that were never picked store `null` and show the automatic colour computed on read, so nothing is
//! written for them. The Markdown copy does not carry the colour.

use crate::chats::{apply_chat, begin, load_chat, summary};
use crate::error::write_err;
use crate::hlc_state::next_hlc;
use crate::{Now, Store, StoreError};
use kinige_domain::ChatColor;
use kinige_domain::ChatSummary;
use kinige_domain::chat::{is_inbox, set_color};

impl Store {
    /// Colours the chat's letter tile. Inbox is always grey and cannot be recoloured (`InboxProtected`); an unknown chat is
    /// `ChatNotFound`; a chat written by a newer version is `ReadOnly`. The same stored value writes nothing (no new stamp).
    pub fn set_chat_color(
        &self,
        id: &str,
        color: ChatColor,
        now: &Now,
    ) -> Result<ChatSummary, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            if is_inbox(&id) {
                return Err(StoreError::InboxProtected);
            }
            let tx = begin(conn)?;
            let loaded = load_chat(&tx, &id)?;
            loaded.ensure_writable()?;
            if loaded.columns.color.as_deref() != Some(color.as_str()) {
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = set_color(&loaded.groups(), color, &hlc)?;
                apply_chat(&tx, &id, &rewritten, now.ms)?;
            }
            let summary = summary(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(summary)
        })
    }
}
