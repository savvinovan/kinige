//! Slice 5a demo data (KINIG-138): synthetic reactions on today's Inbox entries and on entries of past years, one of them put a year after its entry, with
//! dates counted from the run's `journal_date` (chrono, the 04:00 boundary). Only with `KINIGE_SEED_REACTIONS=1`, so earlier scenarios keep their data.
//!
//! The entry "Планы на выходные" gets ten reactions (the limit), "Купить билеты на субботу и позвонить маме" two, "Идея для приложения" one, and the entry of
//! "On this day" of the newest past year a late one (🥲, a year later) while the one of three years ago gets a heart.

use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};

const MINUTE: i64 = 60_000;

pub fn seed_reactions(store: &Store, now: &Now) {
    let at = |minutes_ago: i64| Now::at(now.ms - minutes_ago * MINUTE, &now.tz);
    let page = store
        .list_messages(INBOX_ID, None, 200, now)
        .expect("the Inbox feed");
    let find = |start: &str| {
        page.messages
            .iter()
            .find(|m| m.body_md.starts_with(start))
            .map(|m| m.id.clone())
            .unwrap_or_else(|| panic!("no entry starting with {start}"))
    };
    let put = |id: &str, emoji: &str, minutes_ago: i64| {
        store
            .set_message_reaction(id, emoji, true, &at(minutes_ago))
            .expect("reaction");
    };

    let tickets = find("Купить билеты");
    put(&tickets, "\u{2764}\u{FE0F}", 80);
    put(&tickets, "🔥", 78);
    let idea = find("Идея для приложения");
    put(&idea, "😂", 100);
    let plans = find("### Планы на выходные");
    for (i, emoji) in ["😀", "😁", "😂", "🤣", "😃", "😄", "😅", "😆", "😉", "😊"]
        .iter()
        .enumerate()
    {
        put(&plans, emoji, 100 - i as i64);
    }

    // An entry in the trash with its reactions: the trash shows them and does not let them be changed.
    let gone = store
        .send_message(INBOX_ID, "Запись с реакциями, которую я удалил", &at(70))
        .expect("entry")
        .id;
    put(&gone, "🔥", 69);
    put(&gone, "\u{2764}\u{FE0F}", 68);
    store.delete_message(&gone, &at(30)).expect("trash");

    // The "On this day" card: entries of past years. The newest year's first entry gets a reaction put now, which is a year or more after it.
    if let Some(card) = store.get_on_this_day(now).expect("the card") {
        let entries: Vec<_> = card
            .years
            .iter()
            .flat_map(|year| year.entries.iter())
            .collect();
        if let Some(entry) = entries.first() {
            put(&entry.message.id, "🥲", 5);
        }
        if let Some(entry) = entries.get(2) {
            put(&entry.message.id, "\u{2764}\u{FE0F}", 4);
        }
    }
}
