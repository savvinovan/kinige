//! Спайк S5: поиск по-русски. `cargo run --release -- pairs | prefix | bench [N]`.
mod bench;
mod corpus;
mod norm;
mod words;

use norm::Norm;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let n: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    match arg.as_str() {
        "pairs" => pairs(),
        "prefix" => prefix(),
        "bench" => bench::run(n),
        "cold" => bench::cold(),
        "tokenizer" => tokenizer(),
        _ => eprintln!("pairs | prefix | bench [N]"),
    }
}

fn pairs() {
    let norm = Norm::new();
    let (mut ok, mut total) = (0, 0);
    println!("| A | B | стемы | связаны | ожидалось | итог |");
    println!("|---|---|---|---|---|---|");
    for (a, b, expected) in words::PAIRS {
        let (na, nb) = (Norm::words(a).join(" "), Norm::words(b).join(" "));
        let (sa, sb) = (norm.stem(&na), norm.stem(&nb));
        let linked = sa == sb;
        total += 1;
        let good = linked == *expected;
        if good { ok += 1; }
        println!("| {a} | {b} | {sa} / {sb} | {} | {} | {} |", yn(linked), yn(*expected), if good { "ok" } else { "ПРОМАХ" });
    }
    println!("\nсовпало с ожиданием: {ok} из {total}");
}

fn yn(b: bool) -> &'static str { if b { "да" } else { "нет" } }

/// Префикс незаконченного слова: находит ли запрос из первых L знаков документ с этой словоформой.
fn prefix() {
    let norm = Norm::new();
    let mut forms: Vec<String> = Vec::new();
    for g in words::RU_GROUPS { for f in *g { forms.push(Norm::words(f).join(" ")); } }
    forms.sort();
    forms.dedup();
    println!("словоформ: {}", forms.len());
    // стратегии: (индекс содержит исходные слова?, запрос)
    let strategies = ["stem-only index, q=stem(p)*", "stem-only index, q=p*", "stem-only index, q=stem(p)* OR p*", "stem+source index, q=p*", "stem+source index, q=stem(p)* OR p*"];
    for min_len in [2usize, 3] {
        println!("\nМинимальная длина префикса {min_len}:");
        for (si, name) in strategies.iter().enumerate() {
            let (mut hit, mut total) = (0, 0);
            let mut fails: Vec<String> = Vec::new();
            for f in &forms {
                let chars: Vec<char> = f.chars().collect();
                let stem = norm.stem(f);
                let mut tokens = vec![stem.clone()];
                if si >= 3 { tokens.push(f.clone()); }
                for l in min_len..=chars.len() {
                    let p: String = chars[..l].iter().collect();
                    let sp = norm.stem(&p);
                    let matches = |q: &str| tokens.iter().any(|t| t.starts_with(q));
                    let found = match si {
                        0 => matches(&sp),
                        1 | 3 => matches(&p),
                        _ => matches(&sp) || matches(&p),
                    };
                    total += 1;
                    if found { hit += 1 } else if fails.len() < 6 { fails.push(format!("{p}→{f}")); }
                }
            }
            println!("  {name}: {hit}/{total} = {:.1} %  промахи: {}", 100.0 * hit as f64 / total as f64, fails.join(", "));
        }
    }
    println!("\nПримеры стеммов:");
    for w in ["моло", "молоко", "длинный", "длинн", "длин", "длиннее", "книга", "книг", "кни", "йод", "иод", "ёлка", "елка", "бежал", "бегу", "running", "run"] {
        println!("  {w} → {}", norm.stem(w));
    }
}

/// `unicode61` и «й»: с `remove_diacritics 1` (по умолчанию) «йод» и «иод» слились бы.
fn tokenizer() {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    let v: String = c.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();
    println!("SQLite {v}");
    for opt in ["", " remove_diacritics 0", " remove_diacritics 2"] {
        let t = format!("unicode61{opt}");
        c.execute_batch(&format!("DROP TABLE IF EXISTS t; CREATE VIRTUAL TABLE t USING fts5(x, tokenize='{t}'); INSERT INTO t VALUES ('йод'), ('елка'), ('ёлка');")).unwrap();
        let count = |q: &str| -> i64 { c.query_row("SELECT count(*) FROM t WHERE t MATCH ?1", [q], |r| r.get(0)).unwrap() };
        println!("{t:34} 'иод' находит {} из 'йод'; 'елка' находит {} (ёлка и елка)", count("иод"), count("елка"));
    }
}
