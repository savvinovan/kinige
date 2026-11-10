import { WHITE_SPACE_CHAR, isClosingFence, openingFence } from "./mockText";

// Entry tags for `MockApi`: a port of `kinige_domain::tags` (the real rules are in Rust, `fixtures/api-contract/tags.json` pins both). A
// tag is `#` followed by letters of any alphabet, digits, `_`, not digits only, at most 64 characters; case does not matter. Not tags:
// code, links, HTML tags, bare addresses, and `#` after a letter, digit, `_`, `#`, `&` or `\`.

const MAX_TAG_CHARS = 64;
/** Brackets deeper than this are not counted (the same guard as in Rust: a line of `[a](` must not take quadratic time). */
const MAX_NESTING = 64;
const URL_PREFIXES = ["http://", "https://", "mailto:", "www."];
const TAG_CHAR = /^[\p{Alphabetic}\p{N}_]$/u;

export function mockTags(body: string): string[] {
  const tags: string[] = [];
  let fence: readonly [string, number] | null = null;
  for (const line of body.split("\n")) {
    if (fence) {
      if (isClosingFence(line, fence[0], fence[1])) fence = null;
      continue;
    }
    fence = openingFence(line);
    if (fence === null) scanLine([...line], tags);
  }
  return tags;
}

function scanLine(chars: readonly string[], tags: string[]): void {
  // Without a `>` further on, `<` opens nothing, so the closing one is not searched for every `<`.
  const lastGt = chars.lastIndexOf(">");
  let i = 0;
  while (i < chars.length) {
    const c = chars[i] ?? "";
    if (c === "`") i = skipCodeSpan(chars, i);
    else if (c === "[") i = skipLink(chars, i);
    else if (c === "<") i = skipAngle(chars, i, lastGt);
    else if (c === "\\") i += 2;
    else if (c === "#") i = scanTag(chars, i, tags);
    else if (startsUrl(chars, i)) {
      while (i < chars.length && !WHITE_SPACE_CHAR.test(chars[i] ?? "")) i += 1;
    } else i += 1;
  }
}

/** A `#` at `at`: records the tag if it is one. Returns the index after the tag characters. */
function scanTag(chars: readonly string[], at: number, tags: string[]): number {
  const before = at > 0 ? chars[at - 1] : undefined;
  const allowed = before === undefined || !(isTagChar(before) || before === "#" || before === "&");
  let length = 0;
  while (isTagChar(chars[at + 1 + length])) length += 1;
  const word = chars.slice(at + 1, at + 1 + length);
  if (allowed && length > 0 && length <= MAX_TAG_CHARS && word.some((c) => !/^[0-9]$/.test(c))) {
    const tag = word.join("").toLowerCase();
    if (!tags.includes(tag)) tags.push(tag);
  }
  return at + 1 + length;
}

/** A bare address starts at a word boundary: `https://…`, `http://…`, `mailto:…`, `www.…`. */
function startsUrl(chars: readonly string[], at: number): boolean {
  if (at > 0 && isTagChar(chars[at - 1])) return false;
  const rest = chars.slice(at, at + 8).join("").toLowerCase();
  return URL_PREFIXES.some((prefix) => rest.startsWith(prefix));
}

/** A string in backticks: a run of n backticks and a closing run of the same length. If not closed, the backticks are plain text. */
function skipCodeSpan(chars: readonly string[], start: number): number {
  const run = runOf(chars, start, "`");
  let i = start + run;
  while (i < chars.length) {
    if (chars[i] === "`") {
      const close = runOf(chars, i, "`");
      if (close === run) return i + close;
      i += close;
    } else i += 1;
  }
  return start + run;
}

/** `[text](address)`: skipped whole, together with the text. Otherwise (`[ ]`, `[x]`, footnotes) `[` is a plain character. */
function skipLink(chars: readonly string[], start: number): number {
  const close = matching(chars, start, "[", "]");
  if (close === null || chars[close + 1] !== "(") return start + 1;
  const end = matching(chars, close + 1, "(", ")");
  return end === null ? start + 1 : end + 1;
}

/** `<https://…>` and HTML tags: skipped up to `>` if `<` is followed by a letter, `/` or `!`. */
function skipAngle(chars: readonly string[], start: number, lastGt: number): number {
  const next = chars[start + 1] ?? "";
  const looksLikeTag = /^[\p{Alphabetic}/!]$/u.test(next);
  if (!looksLikeTag || lastGt < start) return start + 1;
  const end = chars.indexOf(">", start);
  return end < 0 ? start + 1 : end + 1;
}

/** The index of the bracket that closes `open` at `start`, with nesting; within the line and not deeper than `MAX_NESTING`. */
function matching(chars: readonly string[], start: number, open: string, close: string): number | null {
  let depth = 0;
  let i = start;
  while (i < chars.length) {
    const c = chars[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === open) {
      depth += 1;
      if (depth > MAX_NESTING) return null;
    } else if (c === close) {
      depth = Math.max(0, depth - 1);
      if (depth === 0) return i;
    }
    i += 1;
  }
  return null;
}

function isTagChar(char: string | undefined): boolean {
  return char !== undefined && TAG_CHAR.test(char);
}

function runOf(chars: readonly string[], start: number, char: string): number {
  let length = 0;
  while (chars[start + length] === char) length += 1;
  return length;
}
