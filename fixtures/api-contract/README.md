# API contract fixtures

The web mock (`MockApi`) repeats some rules of the Rust code. Each file here pins one rule with inputs and expected outputs.
Both sides read the same file: `web/src/api/apiContract.test.ts` and `apiContractFlow.test.ts` run it against the mock, and
`crates/kinige-store/tests/api_contract/` runs it against the real rules and the real `Store`. If a rule changes in Rust and the
file is updated, the mock test fails until the mock follows; if the mock drifts, the file fails it.

Rust is the source of truth. A value here that the Rust test disagrees with is a wrong fixture, not a Rust bug to hide.
The data is synthetic. A text may be written as a list of parts, where `[unit, n]` is the unit repeated `n` times.

A rule the mock repeats gets a file here. A rule the mock keeps rough on purpose does not: say so in the comment of the mock
code (the `mock/` folder and `mockRules.ts` do), and do not pin it.

| File                     | Rule                                                               | Rust                                      |
| ------------------------ | ------------------------------------------------------------------ | ----------------------------------------- |
| `chat-color.json`        | the colour of a chat tile                                          | `chat_color::resolve_color`               |
| `date-valid.json`        | a real journal date                                                | `journal::is_valid_journal_date`          |
| `journal-day-utc.json`   | the entry day with the 04:00 boundary, in UTC                      | `journal::journal_date`                   |
| `chat-input.json`        | chat name and emoji checks                                         | `chat::{normalize_title, normalize_icon}` |
| `append-target.json`     | which entry capture may append to                                  | `append::can_append`                      |
| `append-join.json`       | the text after appending                                           | `append::joined_body`                     |
| `reply-quote.json`       | years later, the excerpt, the quote of an old entry                | `reply::{years_later, excerpt, reply_to}` |
| `reply-flow.json`        | replies end to end, the quote states, sending errors (scenario)    | `Store`                                   |
| `trash-days.json`        | days left in the trash                                             | `history::days_left`                      |
| `constants.json`         | page sizes and limits                                              | the constants themselves                  |
| `page-limit.json`        | the size of a feed and a search page (scenario)                    | `Store::{list_messages, search}`          |
| `recent-emoji.json`      | the recent emoji list (scenario)                                   | `Store::add_recent_emoji`                 |
| `tags.json`              | the tags of an entry                                               | `tags::parse_tags`                        |
| `task-items.json`        | toggling a task item                                               | `tasks::set_task_item`                    |
| `forest-stage.json`      | the growth stage of a forest day                                   | `forest::{day_weight, stage_of}`          |
| `forest-day-chat.json`   | the chat a click on a forest day opens                             | `forest_year::day_chat`                   |
| `attachment-kind.json`   | the kind of an attachment from its name                            | `attachment::{stored_ext, classify}`      |
| `attachment-errors.json` | attachment and voice limits and the order of the checks (scenario) | `Store`                                   |

## Scenarios

A scenario is a list of steps run in order against an empty store (Rust) or a new `MockApi` (web). A name in a step is a
label: `chat`, `msg` and `tile` labels are filled by earlier steps (`inbox` is the Inbox; an unknown label such as `nowhere`
or `ghost` is passed as it is). A step with `error` must fail with exactly that code; any other step must succeed.

| Step         | Fields                                            | Meaning                                                           |
| ------------ | ------------------------------------------------- | ----------------------------------------------------------------- |
| `chat`       | `as`                                              | create a chat                                                     |
| `send`       | `chat`, `body`, `as`?, `replyTo`?                 | send an entry                                                     |
| `seed`       | `chat`, `count`                                   | send that many entries ("entry N")                                |
| `page`       | `chat`, `limit` or null, `count`                  | the newest page has that many entries                             |
| `search`     | `query`, `limit` or null, `from`?, `to`?, `count` | a search finds that many entries                                  |
| `saveDraft`  | `chat`, `body`, `replyTo`?                        | save the chat draft                                               |
| `draft`      | `chat`, `body`, `quote`                           | the draft has that text and that reply-line state (null for none) |
| `feedQuote`  | `msg`, `state`, `quoteChat`                       | the quote under that reply has that state and points to that chat |
| `move`       | `msg`, `to`                                       | move an entry to another chat                                     |
| `delete`     | `msg`                                             | move an entry to the trash                                        |
| `restore`    | `msg`                                             | take it out of the trash                                          |
| `purge`      | `msg`                                             | delete it for good                                                |
| `paste`      | `chat`, `bytes`?, `as`?                           | paste bytes into the draft as a tile                              |
| `removeTile` | `chat`, `tile`                                    | remove a tile                                                     |
| `tiles`      | `chat`, `count`                                   | the draft has that many tiles                                     |
| `sendWith`   | `chat`, `body`, `ids`, `replyTo`?, `as`?          | send an entry with tiles                                          |
| `voice`      | `chat`, `bytes`?, `durationMs`, `replyTo`?, `as`? | send a voice note                                                 |
| `recent`     | `emoji`, then `list`, or `first` and `count`      | add a recent emoji and check the list                             |
| `recentMany` | `emojis`, `count`, `first`, `last`                | add many and check the ends                                       |
