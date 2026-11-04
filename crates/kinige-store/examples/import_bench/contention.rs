//! KINIG-47 measurements of `import_bench`: the tray's "recent chats" before and after the query change, and the window's reads
//! while an AI agent searches before and after the agent got its own read connection. Both compare the old and the new
//! arrangement inside one run, on the same database. Prints only numbers; no texts, names or paths.

use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AgentRequest, Now, Store};
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Chats the 100,000 imported entries are spread over for the recent chats measurement.
const CHATS: usize = 40;
/// Rounds of each measurement.
const ROUNDS: usize = 40;

/// The query "recent chats" had before KINIG-47.3: `MAX(created_at)` over every live entry of every chat.
const OLD_RECENT_SQL: &str = "SELECT c.id, c.title, c.icon FROM chat c \
     WHERE c.deleted_at IS NULL AND c.archived_at IS NULL AND c.id != ?1 \
     AND EXISTS (SELECT 1 FROM message m WHERE m.chat_id = c.id AND m.deleted_at IS NULL) \
     ORDER BY (SELECT MAX(m.created_at) FROM message m WHERE m.chat_id = c.id AND m.deleted_at IS NULL) DESC, \
     c.id LIMIT ?2";

/// "Recent chats" (the tray menu, five chats) in the old and the new form. The imported entries sit in Inbox only, so the bench first
/// spreads them over `CHATS` chats by rewriting the `chat_id` column of the table directly (the groups are not touched): the database
/// is a throw-away one, and only the shape of the data matters for a query.
pub fn recent_chats_bench(store: &Store, data: &Path) {
    let chats: Vec<String> = (0..CHATS)
        .map(|index| {
            store
                .create_chat(&format!("Чат {index}"), None, &Now::system())
                .unwrap()
                .id
        })
        .collect();
    let conn = rusqlite::Connection::open(data.join("kinige.db")).unwrap();
    for (index, chat) in chats.iter().enumerate() {
        conn.execute(
            "UPDATE message SET chat_id = ?1 WHERE chat_id = ?2 AND rowid % ?3 = ?4",
            rusqlite::params![chat, INBOX_ID, CHATS as i64 + 1, index as i64],
        )
        .unwrap();
    }
    let entries: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message WHERE chat_id != ?1",
            [INBOX_ID],
            |row| row.get(0),
        )
        .unwrap();

    let reader = rusqlite::Connection::open_with_flags(
        data.join("kinige.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let old = || -> Vec<String> {
        let mut statement = reader.prepare(OLD_RECENT_SQL).unwrap();
        statement
            .query_map(rusqlite::params![INBOX_ID, 5], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let new = || -> Vec<String> {
        store
            .recent_chats(5)
            .unwrap()
            .into_iter()
            .map(|chat| chat.id)
            .collect()
    };
    assert_eq!(
        old(),
        new(),
        "the old and the new query give the same five chats"
    );
    let before = sample(ROUNDS, || {
        old();
    });
    let after = sample(ROUNDS, || {
        new();
    });
    println!(
        "recent chats (tray menu, 5 of {CHATS} chats, {entries} entries outside Inbox), before (MAX(created_at) over every entry): {}",
        stats(&before)
    );
    println!(
        "recent chats (tray menu, 5 of {CHATS} chats, {entries} entries outside Inbox), after (last entry in feed order, message_feed): {}",
        stats(&after)
    );
}

/// The window's feed page and the tray's recent chats while an AI agent searches the whole journal in a loop, with the agent on the
/// window's connection (before KINIG-47.2) and on its own (after). The agent's own search time is printed too.
pub fn agent_contention_bench(store: &Store, words: &[String]) {
    store.set_agent_chats(vec![INBOX_ID.to_string()]).unwrap();
    store.set_agent_mode(AgentMode::Read).unwrap();
    store.set_agent_enabled(true).unwrap();
    let words: Vec<String> = words.iter().take(20).cloned().collect();
    if words.is_empty() {
        println!("agent contention: no words to search");
        return;
    }
    for (label, shared) in [
        ("before: the agent on the window's connection", true),
        ("after: the agent on its own connection", false),
    ] {
        store.set_agent_shares_reader_for_test(shared);
        let stop = AtomicBool::new(false);
        let (feed, recent, agent_times) = std::thread::scope(|scope| {
            let agent = scope.spawn(|| {
                let mut times = Vec::new();
                let mut next = 0usize;
                while !stop.load(Ordering::Relaxed) {
                    let word = &words[next % words.len()];
                    next += 1;
                    let arguments: Map<String, Value> =
                        json!({"query": word}).as_object().cloned().unwrap();
                    let t = Instant::now();
                    store
                        .agent_call(
                            &AgentRequest {
                                client: "bench",
                                tool: "search",
                                arguments: Some(&arguments),
                                inbox_title: "Inbox",
                            },
                            &Now::system(),
                        )
                        .unwrap();
                    times.push(super::ms(t));
                }
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                times
            });
            // Let the agent get going, then read the feed page and the tray list the way the window does.
            std::thread::sleep(Duration::from_millis(300));
            let feed = sample(ROUNDS * 5, || {
                store
                    .list_messages(INBOX_ID, None, 200, &Now::system())
                    .unwrap();
            });
            let recent = sample(ROUNDS, || {
                store.recent_chats(5).unwrap();
            });
            stop.store(true, Ordering::Relaxed);
            (feed, recent, agent.join().unwrap())
        });
        println!(
            "while an AI agent searches in a loop, {label}: feed page (200 entries) {}; recent chats {}; the agent's search {} ({} searches)",
            stats(&feed),
            stats(&recent),
            stats(&agent_times),
            agent_times.len()
        );
    }
    store.set_agent_shares_reader_for_test(false);
}

/// Times of `rounds` runs of `work`, in milliseconds, sorted.
fn sample(rounds: usize, mut work: impl FnMut()) -> Vec<f64> {
    let mut times: Vec<f64> = (0..rounds)
        .map(|_| {
            let t = Instant::now();
            work();
            super::ms(t)
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times
}

/// "p50 .. ms, p95 .. ms, max .. ms" of sorted times.
fn stats(times: &[f64]) -> String {
    if times.is_empty() {
        return "no samples".into();
    }
    format!(
        "p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        times[times.len() / 2],
        times[(times.len() * 95).div_ceil(100) - 1],
        times[times.len() - 1]
    )
}
