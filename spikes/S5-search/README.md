# Spike S5: search in Russian

Date: 2026-10-28. Budget per the specification: up to 3 h. Decision: `docs/adr/0008-search.md`. Code: this crate (outside the workspace): `cargo run --release -- pairs | prefix | tokenizer | bench [N] | cold`, the database for the measurements lives in the directory from `S5_DIR`.

Machine: Apple M3 Pro, 18 GB, macOS 26.5.2, SQLite 3.53.2 (bundled with `rusqlite 0.40`), Rust 1.94, `--release` mode. WAL, `synchronous=NORMAL`.

## Corpus
There is no real data in the repository, so the corpus is generated (`src/corpus.rs`, deterministic): 100,000 entries over 5 years (1826 days), 40 chats, 48.9 MB of text (490 bytes on average; the user's real "Избранное" (Saved Messages) is about 650 bytes per entry). The vocabulary is 574 hand-written word forms plus 40,000 pseudo-words, with Zipf frequencies. Each of the five "frequent" words is mixed into 30 % of the entries, "купить молоко" (buy milk) into 2 %, rare words occur 2–5 times. The speed result depends on the word distribution; it is artificial, but the worst case (a word in 30 % of the entries) was chosen on purpose.

## Stemmer
`rust-stemmers 1.2.0` (Snowball, Russian and English): license MIT/BSD-3-Clause, last release 17.11.2019 (unmaintained, the Snowball algorithm doesn't change), 39.8 million downloads on crates.io, dependencies `serde` and `serde_derive`. `cargo build --release --target wasm32-unknown-unknown` of a crate with it passes. The rejection criterion did not trigger. Building `kinige-domain` itself with it is in task 4.4.

## Word forms (50 pairs, `pairs`)
50 of 50 matched the expectation, where the expectation ("linked" or "not linked") was written down before the run. All "yes" pairs (книга/книги/книгу/книгой/книгам, ёлка/елка, длинный/длиннее, молоко/молока/молоком, работать/работал, красивый/красиво, run/running/runs, city/cities…) are linked. Known misses that the stemmer doesn't link (expected, these are not implementation bugs):

| pair                                        | reason                                   |
|---------------------------------------------|------------------------------------------|
| бежал / бегу                                | consonant alternation in the root        |
| человек / люди, ребёнок / дети              | different stems                          |
| идти / шёл                                  | suppletion                               |
| хороший / лучше                             | comparative formed from a different stem |
| день / дня                                  | fleeting vowel (`ден` / `дня`)           |
| happy / happier, write / written, go / went | English irregulars                       |

"йод" and "иод" are not merged: the stems are `йод` and `иод`, and `unicode61` doesn't fold "й" into "и" with any `remove_diacritics` setting (`tokenizer`). But it doesn't fold "ё" into "е" either (`елка` doesn't find `ёлка`), so Rust does "ё" → "е" before indexing and before the query.

## Prefix of an unfinished word (`prefix`)
For each of the 574 word forms, every prefix of 2 characters or longer was checked: does the query find the document with this form.

| index                    | query for the last word | found                     |
|--------------------------|-------------------------|---------------------------|
| stems only               | `stem(p)*`              | **99.0 %** (2834 of 2863) |
| stems only               | `p*`                    | 72.8 %                    |
| stems only               | `stem(p)* OR p*`        | 99.0 %                    |
| stems and original words | `p*`                    | 100 %                     |

"моло" → stem `мол`, and `мол*` finds `молок`; "длинн" → `длин`, and `длин*` finds the stem of the word "длинный" (`длин`). A plain prefix `p*` without the original words doesn't work (the stem `длин` has no "нн"), `OR p*` adds nothing. The misses of `stem(p)*` are prefixes that break off in the middle of an unrecognized ending: "длинног" (on the way to "длинного"), "большог", "бежат"; after the next letter the query finds the entry again. The remaining 1 % is weighed against index growth: with the words, 34.3 MB versus 25.7 MB.

The minimum prefix length is 2 characters: one character would turn the query into `м*` over a quarter of the index; a one-character word is searched as an ordinary term.

## FTS5 layout (100,000 entries, `bench`)
Text volume 48.9 MB; the database without an index is 75.0 MB. "To text" is the index size divided by the text volume (success: ≤ 1.5).

| variant                                                                           | index, MB | to text  | batch insert, µs/entry | single insert, µs | single delete, µs      |
|-----------------------------------------------------------------------------------|-----------|----------|------------------------|-------------------|------------------------|
| **v1** contentless (`contentless_delete=1`) + a `rowid → message_id` table, stems | **25.7**  | **0.53** | 13                     | 244               | 87                     |
| v1s the same, stems and original words                                            | 34.3      | 0.70     | 26                     | 211               | 230                    |
| v2 external content (a table of stems + `content=`)                               | 74.2      | 1.52     | 15                     | 249               | 198                    |
| v3 ordinary table, `message_id UNINDEXED`                                         | 69.3      | 1.42     | 14                     | 386               | **21,988** (full scan) |
| t3 trigrams, contentless, lowercase                                               | 96.7      | 1.98     | 38                     | 357               | 285                    |

v1 per `dbstat`: `fts_data` 11.4 MB, the mapping table 4.6 MB and its unique index 5.0 MB, `fts_docsize` 1.2 MB. The `rowid` of the `message` table can change after `VACUUM` (the primary key is text), so the mapping is stored in a separate table.

`index_text` on 100,000 entries: 1.29 s (12.9 µs per entry) — this is the normalization and stemming during an import of 100,000 entries.

## Query speed (ms, p50 / p95 / max; `LIMIT 50`, order `(journal_date, created_at, id)` descending, `deleted_at IS NULL`)
Form A: a join from FTS to `message`, sorting the matches (plan: `SCAN fts`, lookup by the mapping `rowid` and by `id`, a temporary B-tree for `ORDER BY`; no date index needed). Form B: a walk of the `message(journal_date, created_at, id)` index with a check `id IN (…FTS…)`.

| variant and form | rare            | frequent (30 %)     | two words          | prefix              | frequent, deep page (keyset) |
|------------------|-----------------|---------------------|--------------------|---------------------|------------------------------|
| **v1 A**         | 0.0 / 0.9 / 1.6 | 48.1 / 69.1 / 363¹  | 24.6 / 33.7 / 36.2 | 24.6 / 57.6 / 66.1  | 47.7 / 60.9 / 108.7          |
| v1 B             | 0.0 / 0.3 / 0.4 | 65.4 / 95.2 / 114.8 | 26.3 / 31.5 / 41.3 | 27.9 / 79.9 / 84.9  | 70.3 / 101.8 / 129.2         |
| v2 A             | 0.0 / 1.0 / 2.3 | 47.9 / 64.1 / 523¹  | 21.6 / 44.9 / 74.3 | 27.6 / 57.5 / 94.5  | 47.9 / 54.3 / 98.4           |
| v3 A             | 0.0 / 1.0 / 2.8 | 55.8 / 71.4 / 544¹  | 24.0 / 33.6 / 49.6 | 33.1 / 83.1 / 128.8 | 56.2 / 81.4 / 111.6          |

¹ The first run on a freshly written file (pages not in the OS cache). In a separate series (`cold`: 5 new connections in a row, file in the OS cache) the first query for a frequent word takes 45 ms (maximum 357 ms on the very first of the five), the second 44 ms; `mmap_size` 256 MB lowers both to 29 and 26 ms, but is not needed. **The p95 of all query classes in form A is at most 70 ms against a target of 300 ms.**

Trigrams: queries of 1–2 characters return 0 rows, "мол" finds 24,699 entries (by substring, not by stem); the index is 1.98 × the text. They are no good for word forms, and a short prefix needs the stem anyway, so they are not part of the slice.

## Success criteria
| criterion               | result                                                          |
|-------------------------|-----------------------------------------------------------------|
| word forms are found    | yes, 50 of 50 expectations; misses are listed                   |
| p95 on 100,000 < 300 ms | yes: ≤ 70 ms (form A, v1)                                       |
| index ≤ 1.5 × text      | yes: 0.53 (v1); v2 (1.52) doesn't pass                          |
| insert < 1 ms           | yes: 13 µs in a batch, 244 µs one by one in its own transaction |
| `wasm32`                | yes                                                             |

## Not verified
- Speed on the user's real texts (the word distribution is different).
- Behavior at 1 million entries: form A sorts all matches, the time grows linearly with the number of matches.
- A chat filter together with search: the query plan was not captured.
