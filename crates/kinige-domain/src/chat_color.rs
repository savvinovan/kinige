//! The colour of a chat's letter tile (KINIG-59). `meta.color` holds one of seven names or `null`; `null` means "automatic":
//! a colour computed from the chat id on every read, so nothing is written for chats that never got a pick (ADR-0001 details,
//! the `meta.color` convention). No IO.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::chat::is_inbox;

/// The colours the automatic rule picks from, in the order that decides the colour: reordering recolours every chat. Grey is
/// not here: it is only chosen by hand (and is the fixed colour of Inbox).
const AUTOMATIC: [ChatColor; 6] = [
    ChatColor::Blue,
    ChatColor::Teal,
    ChatColor::Green,
    ChatColor::Orange,
    ChatColor::Pink,
    ChatColor::Violet,
];

const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

/// A tile colour. The names are the `color.tile.*` roles of the design system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ChatColor.ts")]
pub enum ChatColor {
    Blue,
    Teal,
    Green,
    Orange,
    Pink,
    Violet,
    Grey,
}

impl ChatColor {
    /// The name as stored in `meta.color`.
    pub fn as_str(self) -> &'static str {
        match self {
            ChatColor::Blue => "blue",
            ChatColor::Teal => "teal",
            ChatColor::Green => "green",
            ChatColor::Orange => "orange",
            ChatColor::Pink => "pink",
            ChatColor::Violet => "violet",
            ChatColor::Grey => "grey",
        }
    }

    /// A stored name; anything else (another client's value, garbage) is `None`.
    pub fn parse(name: &str) -> Option<ChatColor> {
        match name {
            "blue" => Some(ChatColor::Blue),
            "teal" => Some(ChatColor::Teal),
            "green" => Some(ChatColor::Green),
            "orange" => Some(ChatColor::Orange),
            "pink" => Some(ChatColor::Pink),
            "violet" => Some(ChatColor::Violet),
            "grey" => Some(ChatColor::Grey),
            _ => None,
        }
    }
}

/// The automatic colour: 32-bit FNV-1a over the id bytes modulo the six hues. For ASCII ids (UUIDs, `inbox`) it equals the
/// frontend's former `tintForId`, so chats keep the colour they had before colours were stored.
pub fn automatic_color(chat_id: &str) -> ChatColor {
    let hash = chat_id.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    let index = usize::try_from(hash).unwrap_or_default() % AUTOMATIC.len();
    AUTOMATIC.get(index).copied().unwrap_or(ChatColor::Blue)
}

/// The colour shown for a chat: Inbox is always grey; an explicit pick (one of the seven names) wins; `null`, a missing field
/// or a value we do not know show the automatic colour. The stored value itself is never changed here.
pub fn resolve_color(chat_id: &str, stored: Option<&str>) -> ChatColor {
    if is_inbox(chat_id) {
        return ChatColor::Grey;
    }
    stored
        .and_then(ChatColor::parse)
        .unwrap_or_else(|| automatic_color(chat_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::INBOX_ID;

    /// The same pairs as in `web/src/api/mockColor.test.ts`, taken from the former TypeScript function `tintForId`.
    const VECTORS: [(&str, ChatColor); 10] = [
        ("", ChatColor::Teal),
        ("a", ChatColor::Pink),
        ("0b4f7c2e-5d7e-4f38-9a41-6e0f3f8f2c11", ChatColor::Green),
        ("3f1c9a52-8b6e-4d07-a2c4-17e5d9b0c3a8", ChatColor::Teal),
        ("7a2d5e90-1c3b-4f68-8e47-b0a9d2c61f35", ChatColor::Orange),
        ("c8e04b17-9a5d-4c32-b6f1-52d7a83e0c94", ChatColor::Pink),
        ("1d6f8a3b-2e47-4b90-9c5a-8f0e7d34b261", ChatColor::Teal),
        ("e5a79c04-6d18-4a2f-b3e0-94c1f5d72a86", ChatColor::Teal),
        ("92b3d7e1-4f05-4c8a-a6d9-0e1b58c3f470", ChatColor::Violet),
        ("5c0e1f86-a7d3-4429-8b12-d6e9304a7fc5", ChatColor::Teal),
    ];

    #[test]
    fn automatic_color_matches_the_frontend_vectors() {
        for (id, expected) in VECTORS {
            assert_eq!(automatic_color(id), expected, "id {id:?}");
        }
    }

    #[test]
    fn automatic_color_is_never_grey_and_covers_the_six_hues() {
        let seen: std::collections::BTreeSet<&str> = (0..200)
            .map(|index| automatic_color(&format!("chat-{index}")).as_str())
            .collect();
        assert_eq!(seen.len(), 6);
        assert!(!seen.contains("grey"));
    }

    #[test]
    fn inbox_is_always_grey_whatever_is_stored() {
        assert_eq!(resolve_color(INBOX_ID, None), ChatColor::Grey);
        assert_eq!(resolve_color(INBOX_ID, Some("pink")), ChatColor::Grey);
        assert_eq!(resolve_color(INBOX_ID, Some("garbage")), ChatColor::Grey);
    }

    #[test]
    fn an_explicit_pick_wins_over_the_automatic_colour() {
        let id = "0b4f7c2e-5d7e-4f38-9a41-6e0f3f8f2c11";
        for color in [
            ChatColor::Blue,
            ChatColor::Teal,
            ChatColor::Green,
            ChatColor::Orange,
            ChatColor::Pink,
            ChatColor::Violet,
            ChatColor::Grey,
        ] {
            assert_eq!(resolve_color(id, Some(color.as_str())), color);
        }
    }

    #[test]
    fn null_and_unknown_values_show_the_automatic_colour() {
        let id = "0b4f7c2e-5d7e-4f38-9a41-6e0f3f8f2c11";
        let automatic = automatic_color(id);
        for stored in [
            None,
            Some(""),
            Some("red"),
            Some("#fff"),
            Some("BLUE"),
            Some(" blue"),
        ] {
            assert_eq!(resolve_color(id, stored), automatic, "stored {stored:?}");
        }
    }

    #[test]
    fn names_round_trip_through_serde_and_parse() {
        for name in ["blue", "teal", "green", "orange", "pink", "violet", "grey"] {
            let color = ChatColor::parse(name);
            assert_eq!(color.map(ChatColor::as_str), Some(name));
            let json = serde_json::to_string(&color).unwrap_or_default();
            assert_eq!(json, format!("\"{name}\""));
        }
        assert!(serde_json::from_str::<ChatColor>("\"red\"").is_err());
    }
}
