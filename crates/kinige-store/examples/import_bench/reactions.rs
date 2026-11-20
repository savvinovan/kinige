//! Slice 5a (KINIG-138): reactions on a tenth of the entries, then the cost of opening Inbox with them. Prints only numbers and times.

use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};
use std::path::Path;
use std::time::Instant;

/// Puts a reaction on every tenth entry (by row), then opens Inbox 100 times (the first page of 200 entries and the "On this day" card) and
/// prints p50, p95 and max. The target of the slice is p95 under 300 ms with reactions on 10 % of the entries.
pub fn reaction_bench(store: &Store, data: &Path) {
    let ids: Vec<String> = {
        let conn = rusqlite::Connection::open(data.join("kinige.db")).unwrap();
        let mut statement = conn
            .prepare("SELECT id FROM message WHERE rowid % 10 = 0")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let t = Instant::now();
    for id in &ids {
        store
            .set_message_reaction(id, "\u{2764}\u{FE0F}", true, &Now::system())
            .unwrap();
    }
    println!(
        "reactions put on {} entries (a tenth): {:.1} s",
        ids.len(),
        t.elapsed().as_secs_f64()
    );
    let mut times = Vec::new();
    let mut with_reactions = 0usize;
    for _ in 0..100 {
        let t = Instant::now();
        let page = store
            .list_messages(INBOX_ID, None, 200, &Now::system())
            .unwrap();
        let _card = store.get_on_this_day(&Now::system()).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        with_reactions = page
            .messages
            .iter()
            .filter(|m| !m.reactions.is_empty())
            .count();
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "opening Inbox with reactions on a tenth of the entries (page of 200, {with_reactions} with reactions, + the card, 100 opens): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        times[times.len() / 2],
        times[(times.len() * 95).div_ceil(100) - 1],
        times[times.len() - 1]
    );
}
