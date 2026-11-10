// Text helpers of the mock rules. Each one repeats a Rust function; `fixtures/api-contract/` pins them against it
// (`apiContract.test.ts`), except where a comment says the copy is rough on purpose.

/** The characters Rust's `char::is_whitespace` accepts (Unicode White_Space): not the same set as JS `\s` (no U+FEFF, U+0085 is in). */
const WHITE_SPACE = "\\t-\\r \\u0085\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000";
const WHITE_AT_EDGES = new RegExp(`^[${WHITE_SPACE}]+|[${WHITE_SPACE}]+$`, "gu");

export const WHITE_SPACE_CHAR = new RegExp(`[${WHITE_SPACE}]`, "u");

/** `str::trim` of Rust. */
export function rustTrim(text: string): string {
  return text.replace(WHITE_AT_EDGES, "");
}

/** The start of a fenced block: up to three spaces, then three or more "`" or "~" (for "`" the info string has no "`"). */
export function openingFence(line: string): readonly [string, number] | null {
  const rest = stripIndent(line);
  const marker = rest?.[0];
  if (rest === null || (marker !== "`" && marker !== "~")) return null;
  const length = runLength(rest, marker);
  if (length < 3 || (marker === "`" && rest.slice(length).includes("`"))) return null;
  return [marker, length];
}

export function isClosingFence(line: string, marker: string, length: number): boolean {
  const rest = stripIndent(line);
  if (rest === null) return false;
  const run = runLength(rest, marker);
  return run >= length && rustTrim(rest.slice(run)) === "";
}

/**
 * The start of an entry as plain text on one line, at most `max` characters, cut at a word boundary with an ellipsis (Rust's
 * `snippet` with no search words). The Markdown markup is stripped roughly: the real function parses lines and links.
 */
export function plainSnippet(bodyMd: string, max: number): string {
  const text = bodyMd.replace(/[*_`#>~]|\[[ xX]\]/g, "").replace(/\s+/g, " ").trim();
  const chars = [...text];
  if (chars.length <= max) return text;
  const inWord = (at: number) => /[\p{Alphabetic}\p{N}]/u.test(chars[at] ?? "");
  // One place goes to the ellipsis; a word is not cut in the middle unless it is longer than the whole line.
  let end = max - 1;
  let cut = end;
  while (cut > 0 && inWord(cut - 1) && inWord(cut)) cut -= 1;
  if (cut > 0) end = cut;
  return `${chars.slice(0, end).join("").trimEnd()}…`;
}

/** The line without its 0-3 space indent; `null` if the indent is larger. */
function stripIndent(line: string): string | null {
  const spaces = runLength(line, " ");
  return spaces > 3 ? null : line.slice(spaces);
}

function runLength(text: string, char: string): number {
  let length = 0;
  while (text[length] === char) length += 1;
  return length;
}
