//! What the feed shows while a parent is missing (ADR-0001 §3.6). There are no foreign keys: a record may arrive before its parent.
//! This is derived display only: `placement` never changes and no envelope is made by it.

/// How long the children of a purged chat stay hidden after the device learned of the purge: long enough for the initiator's cascade
/// of `purged` envelopes to arrive (S4a, 2026-11-05). A child whose `purged` never comes -- the initiator had not seen the message
/// when it purged the chat -- is shown in Inbox after this, instead of staying hidden until the tombstone expires.
pub const PURGED_CHILD_GRACE_MS: u64 = 24 * 60 * 60 * 1000;

/// What a device knows about the chat a message points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatState {
    /// The chat exists and is not in the trash (Inbox always is).
    Live,
    /// The chat is in the trash: its messages are hidden together with it (decision 4).
    Trashed,
    /// The chat is purged (a tombstone). A message that is a child of the purge (`placement` older than the chat's deletion,
    /// [`super::purge::is_chat_child`]) is hidden until its own `purged` arrives; any other message was moved in later.
    Purged {
        message_is_child: bool,
        /// The device learned of the purge less than [`PURGED_CHILD_GRACE_MS`] ago.
        recent: bool,
        /// This device is the initiator and has the message's own `purged` in its outbox, not yet acked: it is about to purge the
        /// message, so it must not show it (S4a review).
        own_purge_pending: bool,
    },
    /// Neither a live chat nor a tombstone: the chat has not arrived yet, or its tombstone expired.
    Unknown,
}

/// Where a message is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// In the chat its `placement` names.
    InChat,
    /// In Inbox, with a mark.
    Inbox(InboxMark),
    /// Not shown.
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxMark {
    /// "Chat deleted": the chat's tombstone is alive and the message is not its child, or its `purged` did not come in time.
    ChatDeleted,
    /// "Chat unavailable": after a complete pull the chat is neither live nor in a tombstone.
    ChatUnavailable,
}

/// The slot of a live (not trashed) message. `reached_head` is whether pull has reached the head of the server log: only then can a
/// missing chat be called unavailable, before that it may still be on its way. An attachment or a revision without its message is
/// always hidden (it has no slot of its own).
pub fn message_slot(chat: ChatState, reached_head: bool) -> Slot {
    match chat {
        ChatState::Live => Slot::InChat,
        ChatState::Trashed => Slot::Hidden,
        ChatState::Purged {
            message_is_child: true,
            recent,
            own_purge_pending,
        } if recent || own_purge_pending || !reached_head => Slot::Hidden,
        ChatState::Purged { .. } => Slot::Inbox(InboxMark::ChatDeleted),
        ChatState::Unknown if reached_head => Slot::Inbox(InboxMark::ChatUnavailable),
        ChatState::Unknown => Slot::Hidden,
    }
}
