# ADR-0008: Search in Russian — Snowball, FTS5 without stored text, prefix on the stem

Date: 2026-10-28. Status: **accepted** (slice 4, spike S5). PRD §9.1, §16 (S5); ADR-0001 §C; numbers and methodology — `spikes/S5-search/README.md`.

## Context
Search must find «книги», «книгу», «книгой» (forms of "book") for the query «книга» ("book"), find «молоко» ("milk") for «моло», and fit into 300 ms on 100,000 entries. The index is a derived table on the device and is not synchronized.

## Decision
- **Stemmer: `rust-stemmers` 1.2.0** (Snowball, Russian and English; MIT/BSD-3-Clause; released in 2019, not developed further, but the algorithm is stable; pulls in `serde`, `serde_derive`; builds for `wasm32-unknown-unknown`).
- **Normalization (Rust, `kinige_domain::search`):** a word is a run of `char::is_alphanumeric`; lowercase; «ё» → «е»; a word with Cyrillic goes to the Russian stemmer, one of ASCII letters to the English stemmer, words with digits and everything else as is. Rust normalizes «ё» because `unicode61` does not fold it into «е».
- **FTS5 layout (variant v1):** `fts5(t, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 0')` — the text is not stored — and a `rowid → message_id` table (`message_id` is unique). One cannot rely on the entry's `rowid` (it changes after `VACUUM`), so the mapping is our own. `remove_diacritics 0` is written explicitly so that «йод» and «иод» (two spellings of "iodine") do not merge. SQLite ≥ 3.43 is needed; the bundle has 3.53.2.
- **The index holds only stems**, without the original words: 0.53 × the text volume versus 0.70 with the words; the difference in prefix recall is 1% (a cut in the middle of an unrecognized ending, fixed by the next letter).
- **Query:** the terms in quotes, joined with AND. The last term, if the input does not end with a space and its stem is at least two characters long, is a prefix `"stem"*` (not `word*`: the stem of «длинного» ("long", genitive) is `длин`, and it has no «нн»). Otherwise the term is exact. Nothing of the FTS5 syntax from the input gets into the query.
- **Order and selection:** form A — from FTS to `message`, sorting the found entries by `(journal_date, created_at, id)` descending; we do not add an index on dates.
- **Version:** `SEARCH_INDEX_VERSION = 1` in `kinige_domain::search`; stored in `app_setting.search_index_version`. Changing the stemmer, the word rules or `tokenize` requires a new number: the index is rebuilt at startup.
- **Trigrams and a custom tokenizer are not needed.**

## Measurements (100,000 entries, 48.9 MB of text, M3 Pro, SQLite 3.53.2)
Index 25.7 MB (0.53 ×); insertion 13 µs in a batch, 244 µs for one entry in its own transaction; deletion 87 µs; query p95 — no more than 70 ms (a word in 30% of entries, two words, a prefix); `index_text` — 12.9 µs per entry. Rejected: external content (1.52 ×, duplicates the stem text), an ordinary table with `UNINDEXED` (deletion by `message_id` is a full scan, 22 ms per entry), trigrams (1.98 ×, do not find word forms, do not search 1–2 characters).

## Consequences
- The stemmer does not connect: бежал/бегу ("ran"/"run"), человек/люди ("person"/"people"), день/дня ("day", with a fleeting vowel), идти/шёл ("go"/"went"), irregular English forms. This is accepted; it needs a dictionary-based morphological analyzer, outside the slice.
- A contentless table cannot do `snippet()` and `highlight()` and cannot be rebuilt with the `rebuild` command: Rust makes the snippet and the highlighting from `body_md`, and rebuilding means deleting the tables and recomputing from `message.body_md`.
- There is no `message.body_search` column from the PRD §4.2 draft (slice 4 spec, decision 9).
- The measurements were made on a generated corpus; on real text the numbers differ. The measurement on 100,000 entries of the app is in the PR.
