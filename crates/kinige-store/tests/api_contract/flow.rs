//! The scenarios of the contract: the steps of a fixture file run on a real `Store` in an empty directory. The web side runs the
//! same steps on `MockApi`. The step list is in `fixtures/api-contract/README.md`.

use super::common::{T0, moscow, open_without_mirror};
use super::{fixture, int, list, opt_text, text};
use kinige_domain::SearchFilters;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, PageAt, Store, StoreError};
use serde_json::{Value, json};
use std::collections::HashMap;

#[test]
fn replies_from_the_first_send_to_the_purge() {
    run_file("reply-flow");
}

#[test]
fn the_size_of_a_feed_page() {
    run_file("page-limit");
}

#[test]
fn attachment_and_voice_errors() {
    run_file("attachment-errors");
}

#[test]
fn the_recent_emoji_list() {
    run_file("recent-emoji");
}

/// Every scenario of the file, each on its own store.
fn run_file(name: &str) {
    for scenario in list(&fixture(name), "scenarios") {
        let dir = tempfile::tempdir().unwrap();
        let mut world = World {
            store: open_without_mirror(dir.path()),
            labels: HashMap::from([("inbox".to_string(), INBOX_ID.to_string())]),
            chat_of: HashMap::new(),
        };
        for step in list(scenario, "steps") {
            world.step(step).unwrap_or_else(|failure| {
                panic!("{}: {failure} in {step}", text(scenario, "name"));
            });
        }
    }
}

/// The store, the ids that labels stand for, and the chat each entry was sent to.
struct World {
    store: Store,
    labels: HashMap<String, String>,
    chat_of: HashMap<String, String>,
}

impl World {
    /// Runs one step; `Err` says what differed from the fixture.
    fn step(&mut self, step: &Value) -> Result<(), String> {
        let outcome = self.act(step)?;
        let expected = opt_text(step, "error");
        match (outcome, expected) {
            (Ok(()), None) => Ok(()),
            (Err(got), Some(code)) if got == code => Ok(()),
            (Err(got), expected) => Err(format!("failed with {got}, expected {expected:?}")),
            (Ok(()), Some(code)) => Err(format!("succeeded, expected the error {code}")),
        }
    }

    /// The action of a step: the outer `Err` is a failed check, the inner one the error code the store returned.
    fn act(&mut self, step: &Value) -> Result<Result<(), String>, String> {
        let now = moscow(T0);
        let id = |world: &Self, key: &str| world.id(text(step, key));
        match text(step, "op") {
            "chat" => {
                let chat = self.store.create_chat(text(step, "as"), None, &now);
                Ok(chat
                    .map(|chat| self.label(step, "as", chat.id))
                    .map_err(code))
            }
            "send" => {
                let (chat, reply) = (
                    id(self, "chat"),
                    opt_text(step, "replyTo").map(|r| self.id(r)),
                );
                let sent = self
                    .store
                    .send_reply(&chat, text(step, "body"), reply.as_deref(), &now);
                Ok(sent
                    .map(|view| self.sent(step, &chat, view.id))
                    .map_err(code))
            }
            "seed" => {
                let chat = id(self, "chat");
                for n in 0..int(step, "count") {
                    let at = moscow(T0 + n);
                    self.store
                        .send_message(&chat, &format!("entry {n}"), &at)
                        .map_err(code_text)?;
                }
                Ok(Ok(()))
            }
            "page" => {
                let chat = id(self, "chat");
                let limit = step["limit"]
                    .as_u64()
                    .map_or(kinige_store::DEFAULT_PAGE, |n| n as usize);
                let page = self
                    .store
                    .list_messages(&chat, None, limit, &now)
                    .map_err(code_text)?;
                check(page.messages.len() as i64, int(step, "count"), "page size")?;
                Ok(Ok(()))
            }
            "search" => {
                let filters = SearchFilters {
                    chat_id: None,
                    favorite: false,
                    tasks: false,
                    from: opt_text(step, "from").map(str::to_string),
                    to: opt_text(step, "to").map(str::to_string),
                };
                // The default search page is private to `src-tauri`; steps that count give a limit.
                let limit = step["limit"].as_u64().map_or(50, |n| n as usize);
                let found = match self
                    .store
                    .search(text(step, "query"), &filters, None, limit)
                {
                    Ok(page) => page,
                    Err(error) => return Ok(Err(code(error))),
                };
                check(found.hits.len() as i64, int(step, "count"), "hits")?;
                Ok(Ok(()))
            }
            "saveDraft" => {
                let (chat, reply) = (
                    id(self, "chat"),
                    opt_text(step, "replyTo").map(|r| self.id(r)),
                );
                let saved =
                    self.store
                        .save_chat_draft(&chat, text(step, "body"), reply.as_deref(), &now);
                Ok(saved.map_err(code))
            }
            "draft" => self.check_draft(step, &now),
            "feedQuote" => self.check_feed_quote(step, &now),
            "move" => {
                let (message, to) = (id(self, "msg"), id(self, "to"));
                Ok(self
                    .store
                    .move_message(&message, &to, &now)
                    .map(|_| ())
                    .map_err(code))
            }
            "delete" => Ok(self
                .store
                .delete_message(&id(self, "msg"), &now)
                .map_err(code)),
            "restore" => Ok(self
                .store
                .restore_message(&id(self, "msg"), &now)
                .map_err(code)),
            "purge" => Ok(self
                .store
                .purge_message(&id(self, "msg"), &now)
                .map_err(code)),
            "paste" => {
                let (chat, bytes) = (
                    id(self, "chat"),
                    vec![7u8; step["bytes"].as_u64().map_or(16, |n| n as usize)],
                );
                let tile = self.store.attach_bytes(&chat, None, &bytes, &now);
                Ok(tile
                    .map(|view| self.label(step, "as", view.id))
                    .map_err(code))
            }
            "removeTile" => {
                let (chat, tile) = (id(self, "chat"), id(self, "tile"));
                Ok(self
                    .store
                    .remove_draft_attachment(&chat, &tile)
                    .map_err(code))
            }
            "tiles" => {
                let chat = id(self, "chat");
                let draft = self.store.chat_draft(&chat, &now).map_err(code_text)?;
                check(draft.attachments.len() as i64, int(step, "count"), "tiles")?;
                Ok(Ok(()))
            }
            "sendWith" => {
                let chat = id(self, "chat");
                let ids: Vec<String> = list(step, "ids")
                    .iter()
                    .map(|t| self.id(t.as_str().unwrap()))
                    .collect();
                let reply = opt_text(step, "replyTo").map(|r| self.id(r));
                let sent =
                    self.store
                        .send_entry(&chat, text(step, "body"), reply.as_deref(), &ids, &now);
                Ok(sent
                    .map(|view| self.sent(step, &chat, view.id))
                    .map_err(code))
            }
            "voice" => {
                let (chat, reply) = (
                    id(self, "chat"),
                    opt_text(step, "replyTo").map(|r| self.id(r)),
                );
                let len = step["bytes"].as_u64().map_or(4096, |n| n as usize);
                let mut bytes = vec![0u8, 0, 0, 0x20];
                bytes.extend_from_slice(b"ftypM4A ");
                bytes.resize(len.max(12), 0x5a);
                let sent = self.store.send_voice(
                    &chat,
                    reply.as_deref(),
                    &bytes,
                    int(step, "durationMs"),
                    &now,
                );
                Ok(sent
                    .map(|view| self.sent(step, &chat, view.id))
                    .map_err(code))
            }
            "recent" => {
                let emoji = text(step, "emoji");
                let list = match self.store.add_recent_emoji(emoji) {
                    Ok(list) => list,
                    Err(error) => return Ok(Err(code(error))),
                };
                check_recent(step, &list)?;
                Ok(Ok(()))
            }
            "recentMany" => {
                let mut list = Vec::new();
                for emoji in self::list(step, "emojis") {
                    list = self
                        .store
                        .add_recent_emoji(emoji.as_str().unwrap())
                        .map_err(code_text)?;
                }
                check_recent(step, &list)?;
                Ok(Ok(()))
            }
            other => Err(format!("unknown step {other}")),
        }
    }

    /// The draft above the field: its text and the state of the reply line.
    fn check_draft(&self, step: &Value, now: &Now) -> Result<Result<(), String>, String> {
        let draft = self
            .store
            .chat_draft(&self.id(text(step, "chat")), now)
            .map_err(code_text)?;
        check(draft.body, text(step, "body").to_string(), "draft text")?;
        let state = draft.reply_to.map(|quote| state_of(&quote));
        check(
            state,
            opt_text(step, "quote").map(str::to_string),
            "draft quote",
        )?;
        Ok(Ok(()))
    }

    /// The quote under a reply, read from the feed of the chat the reply was sent to.
    fn check_feed_quote(&self, step: &Value, now: &Now) -> Result<Result<(), String>, String> {
        let message = self.id(text(step, "msg"));
        let chat = self
            .chat_of
            .get(&message)
            .ok_or("the entry was not sent by a step")?;
        let page = self
            .store
            .list_messages_page(chat, PageAt::Latest, 200, None, now)
            .map_err(code_text)?;
        let quote = page
            .messages
            .iter()
            .find(|m| m.id == message)
            .and_then(|m| m.reply_to.as_ref())
            .ok_or("the entry has no quote")?;
        check(
            state_of(quote),
            text(step, "state").to_string(),
            "quote state",
        )?;
        let expected_chat = opt_text(step, "quoteChat").map(|label| self.id(label));
        check(quote.chat_id.clone(), expected_chat, "quote chat")?;
        Ok(Ok(()))
    }

    /// The id a label stands for; an unknown label stands for itself.
    fn id(&self, label: &str) -> String {
        self.labels
            .get(label)
            .cloned()
            .unwrap_or_else(|| label.to_string())
    }

    /// Remembers the id under the step's `as` label, if it has one.
    fn label(&mut self, step: &Value, key: &str, id: String) {
        if let Some(label) = opt_text(step, key) {
            self.labels.insert(label.to_string(), id);
        }
    }

    /// Remembers a sent entry: its label and its chat.
    fn sent(&mut self, step: &Value, chat: &str, id: String) {
        self.chat_of.insert(id.clone(), chat.to_string());
        self.label(step, "as", id);
    }
}

/// The recent list after a step: its length and its ends, or the whole list.
fn check_recent(step: &Value, list: &[String]) -> Result<(), String> {
    if let Some(expected) = step["list"].as_array() {
        let expected: Vec<&str> = expected.iter().map(|e| e.as_str().unwrap()).collect();
        check(
            list.iter().map(String::as_str).collect::<Vec<_>>(),
            expected,
            "recent list",
        )?;
    }
    if let Some(count) = step["count"].as_i64() {
        check(list.len() as i64, count, "recent count")?;
    }
    if let Some(first) = opt_text(step, "first") {
        check(list.first().map(String::as_str), Some(first), "first emoji")?;
    }
    if let Some(last) = opt_text(step, "last") {
        check(list.last().map(String::as_str), Some(last), "last emoji")?;
    }
    Ok(())
}

fn check<T: PartialEq + std::fmt::Debug>(got: T, expected: T, what: &str) -> Result<(), String> {
    if got == expected {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, expected {expected:?}"))
    }
}

/// The wire name of the state of a quote.
fn state_of(quote: &kinige_domain::ReplyTo) -> String {
    json!(quote.state).as_str().unwrap().to_string()
}

/// The wire name of a store error.
fn code(error: StoreError) -> String {
    json!(error.code()).as_str().unwrap().to_string()
}

/// The same, as a failed check: an error in a step that must not fail.
fn code_text(error: StoreError) -> String {
    format!("unexpected error {}", code(error))
}
