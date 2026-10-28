//! Генератор корпуса: N записей за 5 лет. Словарь — словоформы из `words.rs` (Ципф) плюс длинный хвост
//! псевдослов (около 40 000 различных), чтобы словарь индекса был похож на настоящий. Детерминирован.
use crate::words::RU_GROUPS;

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self { Rng(seed) }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
    pub fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
}

pub struct Msg {
    pub id: String,
    pub chat_id: String,
    pub body: String,
    pub journal_date: String,
    pub created_at: i64,
}

/// Слова, которые мы подмешиваем в заданную долю записей (для запросов).
pub const FREQUENT: &[&str] = &["сегодня", "делал", "думаю", "потом", "ещё"];
pub const RARE: &[&str] = &[
    "аксолотль", "бегемотик", "вермишель", "гондола", "дирижабль", "жонглёр", "зефирка", "каракатица", "лемминг",
    "мармелад", "нарвал", "окарина", "пеликан", "ракушка", "саламандра", "тапир", "утконос", "фламинго", "хамелеон", "цикада",
];
const FUNCTION_WORDS: &[&str] = &[
    "и", "в", "не", "на", "я", "что", "это", "как", "с", "по", "но", "из", "за", "то", "так", "же", "от", "до", "для",
    "было", "все", "уже", "если", "или", "тоже", "очень", "надо", "нужно", "the", "and", "to", "of", "is", "it", "for", "with",
];
const ENGLISH: &[&str] = &[
    "meeting", "project", "deadline", "coffee", "review", "release", "server", "notes", "ideas", "bug", "feature",
    "travel", "flight", "dinner", "movie", "running", "reading", "writing", "plan", "weekend",
];
const ONSET: &[&str] = &["б", "в", "г", "д", "з", "к", "л", "м", "н", "п", "р", "с", "т", "ф", "х", "ц", "ч", "ш", "бр", "вр", "гр", "др", "кр", "пр", "тр", "ст", "сп", "пл", "кл"];
const VOWEL: &[&str] = &["а", "е", "и", "о", "у", "я", "ю"];
const CODA: &[&str] = &["", "н", "р", "л", "т", "к", "с", "м", "нт", "рт"];
const ENDING: &[&str] = &["", "а", "у", "ом", "е", "ы", "ой", "ая", "ый", "ое", "ие", "ов", "ам", "ть", "л", "ла", "ет", "ут"];

fn pseudo_word(rng: &mut Rng) -> String {
    let syl = 2 + rng.below(3);
    let mut w = String::new();
    for _ in 0..syl {
        w.push_str(ONSET[rng.below(ONSET.len())]);
        w.push_str(VOWEL[rng.below(VOWEL.len())]);
        w.push_str(CODA[rng.below(CODA.len())]);
    }
    w.push_str(ENDING[rng.below(ENDING.len())]);
    w
}

pub fn vocabulary(rng: &mut Rng) -> Vec<String> {
    let mut v: Vec<String> = FUNCTION_WORDS.iter().map(|s| s.to_string()).collect();
    for g in RU_GROUPS {
        for f in *g { v.push((*f).to_string()); }
    }
    for e in ENGLISH { v.push((*e).to_string()); }
    for _ in 0..40_000 { v.push(pseudo_word(rng)); }
    v
}

/// Слово по рангу Ципфа (s ~ 1): ранг = n^u.
fn zipf(rng: &mut Rng, n: usize) -> usize {
    let r = (n as f64).powf(rng.unit()) as usize;
    r.saturating_sub(1).min(n - 1)
}

fn civil(days_from_epoch: i64) -> (i64, u32, u32) {
    // алгоритм Хиннанта
    let z = days_from_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn generate(n: usize) -> Vec<Msg> {
    let mut rng = Rng::new(0x5EED_1234_ABCD_9876);
    let vocab = vocabulary(&mut rng);
    let start_day = 17_532; // 2018-01-01
    let days = 1826;
    let chats: Vec<String> = (0..40).map(|i| format!("chat-{i:02}")).collect();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let kind = rng.unit();
        let words = if kind < 0.70 { 4 + rng.below(12) } else if kind < 0.95 { 25 + rng.below(40) } else { 150 + rng.below(250) };
        let mut body = String::new();
        let mut cap = true;
        for k in 0..words {
            let mut w = vocab[zipf(&mut rng, vocab.len())].clone();
            if cap {
                let mut c = w.chars();
                w = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
                cap = false;
            }
            if k > 0 { body.push(' '); }
            body.push_str(&w);
            if rng.below(11) == 0 { body.push_str(if rng.below(3) == 0 { "." } else { "," }); cap = body.ends_with('.'); }
        }
        for f in FREQUENT {
            if rng.unit() < 0.30 { body.push(' '); body.push_str(f); }
        }
        if rng.unit() < 0.02 { body.push_str(" купить молоко"); }
        if i % 5000 == 17 { body.push(' '); body.push_str(RARE[(i / 5000) % RARE.len()]); }
        if i % 20_011 == 3 { body.push(' '); body.push_str(RARE[(i / 20_011 + 7) % RARE.len()]); }
        if rng.unit() < 0.1 { body.push_str(" #дела"); }
        let day = start_day + rng.below(days) as i64;
        let (y, m, d) = civil(day);
        let secs = rng.below(86_400) as i64;
        out.push(Msg {
            id: format!("{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}", rng.next() as u32, i & 0xffff, i & 0xfff, (i >> 4) & 0xfff, rng.next() & 0xffff_ffff_ffff),
            chat_id: chats[rng.below(chats.len())].clone(),
            body,
            journal_date: format!("{y:04}-{m:02}-{d:02}"),
            created_at: (day * 86_400 + secs) * 1000,
        });
    }
    out
}
