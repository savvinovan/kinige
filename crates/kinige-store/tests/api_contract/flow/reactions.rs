//! The steps of the reaction scenarios (slice 5a): put or remove a reaction at a given minute, and check what the feed shows.

use super::{World, check, code, code_text};
use crate::common::{MINUTE, T0, moscow};
use crate::{int, list, spec, text};
use kinige_store::PageAt;
use serde_json::Value;

impl World {
    /// `react`: `msg`, `emoji`, `on`, `minute`? (after the start of the scenario; later reactions sort after earlier ones).
    pub(super) fn react(&self, step: &Value) -> Result<Result<(), String>, String> {
        let at = moscow(T0 + step["minute"].as_i64().unwrap_or(0) * MINUTE);
        let on = step["on"].as_bool().ok_or("`on` is not a boolean")?;
        let done = self.store.set_message_reaction(
            &self.id(text(step, "msg")),
            &spec(&step["emoji"]),
            on,
            &at,
        );
        Ok(done.map(|_| ()).map_err(code))
    }

    /// `reactCycle`: `msg`, `first` (a code point), `count`: each of `count` consecutive characters is put and removed.
    pub(super) fn react_cycle(&self, step: &Value) -> Result<Result<(), String>, String> {
        let message = self.id(text(step, "msg"));
        let first = u32::try_from(int(step, "first")).map_err(|e| e.to_string())?;
        for offset in 0..u32::try_from(int(step, "count")).map_err(|e| e.to_string())? {
            let emoji = char::from_u32(first + offset)
                .ok_or("not a character")?
                .to_string();
            for on in [true, false] {
                let at = moscow(T0 + i64::from(offset) * MINUTE);
                if let Err(error) = self.store.set_message_reaction(&message, &emoji, on, &at) {
                    return Ok(Err(code(error)));
                }
            }
        }
        Ok(Ok(()))
    }

    /// `reactions`: `msg`, `list` (the live emoji in the order put), `keysHad`.
    pub(super) fn check_reactions(&self, step: &Value) -> Result<Result<(), String>, String> {
        let message = self.id(text(step, "msg"));
        let chat = self
            .chat_of
            .get(&message)
            .ok_or("the entry was not sent by a step")?;
        let page = self
            .store
            .list_messages_page(chat, PageAt::Latest, 200, None, &moscow(T0))
            .map_err(code_text)?;
        let view = page
            .messages
            .iter()
            .find(|m| m.id == message)
            .ok_or("the entry is not in the feed")?;
        let got: Vec<String> = view.reactions.iter().map(|r| r.emoji.clone()).collect();
        let expected: Vec<String> = list(step, "list").iter().map(spec).collect();
        check(got, expected, "reactions")?;
        check(
            view.reaction_keys_had.len() as i64,
            int(step, "keysHad"),
            "keys had",
        )?;
        Ok(Ok(()))
    }
}
