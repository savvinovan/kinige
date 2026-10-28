//! Замеры раскладок FTS5 на сгенерированном корпусе.
use crate::corpus::{self, Msg, FREQUENT, RARE};
use crate::norm::Norm;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::time::Instant;

const TOKENIZE: &str = "unicode61 remove_diacritics 0";

fn open(path: &Path) -> Connection {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.pragma_update(None, "synchronous", "NORMAL").unwrap();
    c
}

fn file_size(path: &Path) -> u64 {
    let main = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    main + std::fs::metadata(format!("{}-wal", path.display())).map(|m| m.len()).unwrap_or(0)
}

fn ms(t: Instant) -> f64 { t.elapsed().as_secs_f64() * 1000.0 }

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 * p).ceil() as usize).clamp(1, v.len()) - 1]
}

pub fn run(n: usize) {
    let dir = PathBuf::from(std::env::var("S5_DIR").unwrap_or_else(|_| "out".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let t = Instant::now();
    let msgs = corpus::generate(n);
    let text_bytes: usize = msgs.iter().map(|m| m.body.len()).sum();
    println!("корпус: {n} записей, {:.1} МБ текста, генерация {:.0} мс", text_bytes as f64 / 1e6, ms(t));
    let norm = Norm::new();
    let t = Instant::now();
    let stems: Vec<String> = msgs.iter().map(|m| norm.index_text(&m.body, false)).collect();
    let stem_ms = ms(t);
    let t = Instant::now();
    let both: Vec<String> = msgs.iter().map(|m| norm.index_text(&m.body, true)).collect();
    println!("index_text: только стеммы {:.0} мс ({:.1} мкс/запись), со словами {:.0} мс; объём {:.1} / {:.1} МБ",
        stem_ms, stem_ms * 1000.0 / n as f64, ms(t),
        stems.iter().map(String::len).sum::<usize>() as f64 / 1e6, both.iter().map(String::len).sum::<usize>() as f64 / 1e6);
    let lower: Vec<String> = msgs.iter().map(|m| Norm::words(&m.body).join(" ")).collect();

    let base = dir.join("base.db");
    for ext in ["", "-wal", "-shm"] { let _ = std::fs::remove_file(format!("{}{ext}", base.display())); }
    {
        let mut c = open(&base);
        c.execute_batch("CREATE TABLE message (id TEXT PRIMARY KEY, chat_id TEXT NOT NULL, body_md TEXT NOT NULL, journal_date TEXT NOT NULL, created_at INTEGER NOT NULL, deleted_at INTEGER);
            CREATE INDEX message_order ON message(journal_date, created_at, id);").unwrap();
        let tx = c.transaction().unwrap();
        for m in &msgs {
            tx.execute("INSERT INTO message VALUES (?1,?2,?3,?4,?5,NULL)", params![m.id, m.chat_id, m.body, m.journal_date, m.created_at]).unwrap();
        }
        tx.commit().unwrap();
        c.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();
    }
    let base_size = file_size(&base);
    println!("база без индекса поиска: {:.1} МБ\n", base_size as f64 / 1e6);

    let variants: [(&str, &[String]); 5] = [
        ("v1 contentless+map, стеммы", &stems),
        ("v1s contentless+map, стеммы+слова", &both),
        ("v2 external content, стеммы", &stems),
        ("v3 обычная, message_id UNINDEXED, стеммы", &stems),
        ("t3 trigram contentless, нижний регистр", &lower),
    ];
    println!("| вариант | размер индекса, МБ | к тексту | вставка пачкой, мкс/зап | вставка по одной, мкс | удаление по одной, мкс |");
    println!("|---|---|---|---|---|---|");
    let mut ready: Vec<(usize, PathBuf)> = Vec::new();
    for (vi, (name, texts)) in variants.iter().enumerate() {
        let path = dir.join(format!("var{vi}.db"));
        for ext in ["", "-wal", "-shm"] { let _ = std::fs::remove_file(format!("{}{ext}", path.display())); }
        std::fs::copy(&base, &path).unwrap();
        let mut c = open(&path);
        create(&c, vi);
        let t = Instant::now();
        {
            let tx = c.transaction().unwrap();
            for (i, m) in msgs.iter().enumerate() { insert(&tx, vi, &m.id, &texts[i]); }
            tx.commit().unwrap();
        }
        let bulk = ms(t) * 1000.0 / n as f64;
        c.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();
        let size = file_size(&path).saturating_sub(base_size);
        // по одной записи, каждая в своей транзакции (как правка в приложении); новые id
        let extra: Vec<Msg> = corpus::generate(1200).into_iter().skip(200).map(|mut m| { m.id = format!("x-{}", m.id); m }).collect();
        let t = Instant::now();
        for m in &extra { let tx = c.transaction().unwrap(); insert(&tx, vi, &m.id, &norm.index_text(&m.body, vi == 1)); tx.commit().unwrap(); }
        let single = ms(t) * 1000.0 / extra.len() as f64;
        let t = Instant::now();
        for m in &extra { let tx = c.transaction().unwrap(); remove(&tx, vi, &m.id); tx.commit().unwrap(); }
        let del = ms(t) * 1000.0 / extra.len() as f64;
        println!("| {name} | {:.1} | {:.2} | {:.0} | {:.0} | {:.0} |", size as f64 / 1e6, size as f64 / text_bytes as f64, bulk, single, del);
        ready.push((vi, path));
    }

    println!("\nЗапросы (мс): p50 / p95 / max, LIMIT 50, порядок (journal_date, created_at, id) по убыванию, deleted_at IS NULL");
    println!("| вариант и форма запроса | редкое | частое (30 %) | два слова | префикс | частое, страница в глубине |");
    println!("|---|---|---|---|---|---|");
    let st = |w: &str| norm.stem(w);
    let rare: Vec<String> = RARE.iter().map(|w| format!("\"{}\"", st(w))).collect();
    let freq: Vec<String> = FREQUENT.iter().map(|w| format!("\"{}\"", st(w))).collect();
    let mut two: Vec<String> = Vec::new();
    for i in 0..FREQUENT.len() { for j in i + 1..FREQUENT.len() { two.push(format!("\"{}\" \"{}\"", st(FREQUENT[i]), st(FREQUENT[j]))); } }
    two.push(format!("\"{}\" \"{}\"", st("купить"), st("молоко")));
    let prefix: Vec<String> = ["мо", "моло", "куп", "дела", "вст", "зад", "пис", "дум", "кни", "сег", "про", "раб", "ауп", "зеф", "дир", "пел", "лем", "мар", "нар", "ока"]
        .iter().map(|p| format!("\"{}\"*", st(p))).collect();
    for (vi, path) in &ready {
        if *vi == 4 { continue; }
        let c = open(path);
        let shapes: &[(&str, fn(usize, bool) -> String)] = &[("A: от FTS к message, сортировка", sql_a), ("B: от индекса message, IN (FTS)", sql_b)];
        for (label, f) in shapes {
            let name = variants[*vi].0.split(' ').next().unwrap();
            let cell = |qs: &Vec<String>, reps: usize, deep: bool| -> String {
                let sql = f(*vi, deep);
                let mut s = c.prepare(&sql).unwrap();
                let mut v = Vec::new();
                for _ in 0..reps { for q in qs {
                    let t = Instant::now();
                    let mut rows = if deep { s.query(params![q, "2020-06-15", 0i64, ""]).unwrap() } else { s.query(params![q]).unwrap() };
                    let mut k = 0; while let Some(_r) = rows.next().unwrap() { k += 1; }
                    let _ = k;
                    v.push(ms(t));
                } }
                format!("{:.1} / {:.1} / {:.1}", pct(&mut v.clone(), 0.5), pct(&mut v, 0.95), v.iter().cloned().fold(0.0, f64::max))
            };
            println!("| {name} {label} | {} | {} | {} | {} | {} |", cell(&rare, 3, false), cell(&freq, 20, false), cell(&two, 5, false), cell(&prefix, 3, false), cell(&freq, 20, true));
        }
    }
    // первый запрос на только что открытом соединении (холодный кэш страниц SQLite; файл в кэше ОС)
    for (vi, path) in ready.iter().filter(|(vi, _)| *vi == 0) {
        for q in [&freq[0], &rare[0], &prefix[1]] {
            let c = open(path);
            let t = Instant::now();
            let mut s = c.prepare(&sql_a(*vi, false)).unwrap();
            let mut rows = s.query(params![q]).unwrap();
            while rows.next().unwrap().is_some() {}
            println!("первый запрос на новом соединении (v1, A) {q}: {:.1} мс", ms(t));
        }
    }
    // триграммы: короткие запросы
    let (_, tpath) = ready.iter().find(|(vi, _)| *vi == 4).unwrap();
    let c = open(tpath);
    for q in ["а", "мо", "мол", "молок", "\"молок\""] {
        let r: rusqlite::Result<i64> = c.query_row("SELECT count(*) FROM t3 WHERE t3 MATCH ?1", [q], |r| r.get(0));
        println!("trigram MATCH {q}: {:?}", r.map_err(|e| e.to_string()));
    }
    // dbstat, если есть
    let c = open(&ready[0].1);
    if let Ok(mut s) = c.prepare("SELECT name, sum(pgsize) FROM dbstat GROUP BY name ORDER BY 2 DESC LIMIT 8") {
        println!("\ndbstat v1:");
        let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap();
        for r in rows { let (n, b) = r.unwrap(); println!("  {n}: {:.1} МБ", b as f64 / 1e6); }
    }
}

fn create(c: &Connection, vi: usize) {
    let sql = match vi {
        0 | 1 => format!("CREATE VIRTUAL TABLE fts USING fts5(t, content='', contentless_delete=1, tokenize='{TOKENIZE}');
            CREATE TABLE fts_map (rowid INTEGER PRIMARY KEY, message_id TEXT NOT NULL UNIQUE);"),
        2 => format!("CREATE TABLE fts_src (id INTEGER PRIMARY KEY, message_id TEXT NOT NULL UNIQUE, t TEXT NOT NULL);
            CREATE VIRTUAL TABLE fts USING fts5(t, content='fts_src', content_rowid='id', tokenize='{TOKENIZE}');"),
        3 => format!("CREATE VIRTUAL TABLE fts USING fts5(t, message_id UNINDEXED, tokenize='{TOKENIZE}');"),
        _ => "CREATE VIRTUAL TABLE t3 USING fts5(t, content='', contentless_delete=1, tokenize='trigram');
            CREATE TABLE t3_map (rowid INTEGER PRIMARY KEY, message_id TEXT NOT NULL UNIQUE);".to_string(),
    };
    c.execute_batch(&sql).unwrap();
}

fn insert(c: &Connection, vi: usize, id: &str, text: &str) {
    match vi {
        0 | 1 => {
            c.prepare_cached("INSERT INTO fts_map(message_id) VALUES (?1)").unwrap().execute([id]).unwrap();
            c.prepare_cached("INSERT INTO fts(rowid, t) VALUES (last_insert_rowid(), ?1)").unwrap().execute([text]).unwrap();
        }
        2 => {
            c.prepare_cached("INSERT INTO fts_src(message_id, t) VALUES (?1, ?2)").unwrap().execute([id, text]).unwrap();
            c.prepare_cached("INSERT INTO fts(rowid, t) VALUES (last_insert_rowid(), ?1)").unwrap().execute([text]).unwrap();
        }
        3 => { c.prepare_cached("INSERT INTO fts(t, message_id) VALUES (?1, ?2)").unwrap().execute([text, id]).unwrap(); }
        _ => {
            c.prepare_cached("INSERT INTO t3_map(message_id) VALUES (?1)").unwrap().execute([id]).unwrap();
            c.prepare_cached("INSERT INTO t3(rowid, t) VALUES (last_insert_rowid(), ?1)").unwrap().execute([text]).unwrap();
        }
    }
}

fn remove(c: &Connection, vi: usize, id: &str) {
    match vi {
        0 | 1 => {
            let rowid: i64 = c.prepare_cached("SELECT rowid FROM fts_map WHERE message_id = ?1").unwrap().query_row([id], |r| r.get(0)).unwrap();
            c.prepare_cached("DELETE FROM fts WHERE rowid = ?1").unwrap().execute([rowid]).unwrap();
            c.prepare_cached("DELETE FROM fts_map WHERE rowid = ?1").unwrap().execute([rowid]).unwrap();
        }
        2 => {
            let (rowid, t): (i64, String) = c.prepare_cached("SELECT id, t FROM fts_src WHERE message_id = ?1").unwrap().query_row([id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
            c.prepare_cached("INSERT INTO fts(fts, rowid, t) VALUES ('delete', ?1, ?2)").unwrap().execute(params![rowid, t]).unwrap();
            c.prepare_cached("DELETE FROM fts_src WHERE id = ?1").unwrap().execute([rowid]).unwrap();
        }
        3 => { c.prepare_cached("DELETE FROM fts WHERE message_id = ?1").unwrap().execute([id]).unwrap(); }
        _ => {
            let rowid: i64 = c.prepare_cached("SELECT rowid FROM t3_map WHERE message_id = ?1").unwrap().query_row([id], |r| r.get(0)).unwrap();
            c.prepare_cached("DELETE FROM t3 WHERE rowid = ?1").unwrap().execute([rowid]).unwrap();
            c.prepare_cached("DELETE FROM t3_map WHERE rowid = ?1").unwrap().execute([rowid]).unwrap();
        }
    }
}

/// Источник `message_id` для совпавшего rowid FTS по раскладке.
fn ids(vi: usize) -> &'static str {
    match vi {
        0 | 1 => "SELECT message_id FROM fts_map WHERE rowid IN (SELECT rowid FROM fts WHERE fts MATCH ?1)",
        2 => "SELECT message_id FROM fts_src WHERE id IN (SELECT rowid FROM fts WHERE fts MATCH ?1)",
        _ => "SELECT message_id FROM fts WHERE fts MATCH ?1",
    }
}

const ORDER: &str = "ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT 50";
const CURSOR: &str = "AND (m.journal_date, m.created_at, m.id) < (?2, ?3, ?4)";

/// A: соединение от FTS к message, сортировка найденных.
fn sql_a(vi: usize, deep: bool) -> String {
    let from = match vi {
        0 | 1 => "fts JOIN fts_map mp ON mp.rowid = fts.rowid JOIN message m ON m.id = mp.message_id WHERE fts MATCH ?1",
        2 => "fts JOIN fts_src s ON s.id = fts.rowid JOIN message m ON m.id = s.message_id WHERE fts MATCH ?1",
        _ => "fts JOIN message m ON m.id = fts.message_id WHERE fts MATCH ?1",
    };
    format!("SELECT m.id, m.journal_date FROM {from} AND m.deleted_at IS NULL {} {ORDER}", if deep { CURSOR } else { "" })
}

/// B: идём по индексу message и проверяем принадлежность найденному.
fn sql_b(vi: usize, deep: bool) -> String {
    format!("SELECT m.id, m.journal_date FROM message m WHERE m.deleted_at IS NULL AND m.id IN ({}) {} {ORDER}", ids(vi), if deep { CURSOR } else { "" })
}

/// Холодный первый запрос на существующей `var0.db`: форма запроса и настройки памяти.
pub fn cold() {
    let dir = PathBuf::from(std::env::var("S5_DIR").unwrap_or_else(|_| "out".into()));
    let path = dir.join("var0.db");
    let q = "\"сегодн\"";
    for (label, mmap, cache) in [("по умолчанию", 0i64, -2000i64), ("cache 64 МБ", 0, -65536), ("mmap 256 МБ", 268_435_456, -2000), ("mmap 256 МБ + cache 16 МБ", 268_435_456, -16384)] {
        for (shape, f) in [("A", sql_a as fn(usize, bool) -> String), ("B", sql_b)] {
            let mut first = Vec::new();
            let mut second = Vec::new();
            for _ in 0..5 {
                let c = open(&path);
                c.pragma_update(None, "mmap_size", mmap).unwrap();
                c.pragma_update(None, "cache_size", cache).unwrap();
                for out in [&mut first, &mut second] {
                    let t = Instant::now();
                    let mut s = c.prepare(&f(0, false)).unwrap();
                    let mut rows = s.query(params![q]).unwrap();
                    while rows.next().unwrap().is_some() {}
                    out.push(ms(t));
                }
            }
            println!("{label}, форма {shape}: первый запрос p50 {:.0} мс (max {:.0}), второй p50 {:.0} мс", pct(&mut first.clone(), 0.5), first.iter().cloned().fold(0.0, f64::max), pct(&mut second, 0.5));
        }
    }
}
