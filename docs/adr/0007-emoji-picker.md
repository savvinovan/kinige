# ADR-0007: Emoji picker — frimousse and emojibase data in the bundle

Date: 2026-10-27. Status: **accepted** (slice 3). PRD §2.1, §5, §16; ADR-0004 (tokens), ADR-0005 (i18n).

## Context
Slice 3: one picker for the input field and for the chat window; search by name in Russian and English; "recent"; no network (local-first, the CSP is not widened); the look only from tokens.

## Decision
- **`frimousse` 0.4.0** (MIT, a headless library: list, search, keyboard, virtual rows; the look is our CSS on tokens). The peer dependency is React 18 or 19 (we have 19). Verified from the package's `package.json` and its sources (`resolveEmojiData`: the data can be supplied by our own function rather than from a CDN). The version was chosen as being older than two weeks.
- **`emojibase-data` 17.0.0** (MIT): `en` and `ru` in the bundle, loaded with a dynamic `import()` on the first opening of the picker (two chunks, ≈ 96 and 110 KB after gzip). Search covers both languages at once, so with the Russian UI both «собака» ("dog") and `dog` are found. Version 18.0.0 was rejected: it was published less than two weeks ago. The JSON types are pulled in from `emojibase` as a peer dependency; the package is not imported directly.
- "Recent" is up to 24 emoji in `app_setting` (Rust); nothing is written to `localStorage` or the library's cache.
- Which Unicode Emoji versions the system can draw, the picker determines via canvas (as the library itself does); the fallback threshold without canvas is 15.1, **from memory, not verified**.

## Consequences
- New dependencies of the web part: `frimousse`, `emojibase-data` (+ `emojibase` as a peer). No network; the CSP did not change (`script-src 'self'` allows the chunks).
- jsdom does not lay out a virtual list, so the tests substitute sizes and `ResizeObserver` (`web/src/test/emoji.tsx`); the layout is shown by Storybook stories and the live window.
- Category names in the Russian data are clumsy in places («тело людей», literally "body of people"); if needed — our own labels in `emoji.*`.
