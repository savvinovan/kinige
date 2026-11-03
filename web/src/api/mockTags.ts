/**
 * Entry tags for `MockApi`: a simplified copy of the `kinige_domain::tags` rules (the real parsing is in Rust). A tag is `#`
 * followed by letters of any alphabet, digits, `_`, not digits only; case does not matter; code, links and `#` after a
 * letter/digit/`_` are not tags.
 */
export function mockTags(body: string): string[] {
  const text = body
    .replace(/^(```|~~~)[\s\S]*?(?:^\1|$(?![\s\S]))/gm, " ")
    .replace(/`[^`\n]*`/g, " ")
    .replace(/\[[^\]\n]*\]\([^)\n]*\)/g, " ")
    .replace(/<[^>\n]+>/g, " ")
    .replace(/(?:https?:\/\/|mailto:|www\.)\S+/gi, " ");
  const tags: string[] = [];
  for (const match of text.matchAll(/(?<![\p{L}\p{N}_#&\\])#([\p{L}\p{N}_]{1,64})/gu)) {
    const tag = (match[1] ?? "").toLowerCase();
    if (/^\d+$/.test(tag) || tags.includes(tag)) continue;
    tags.push(tag);
  }
  return tags;
}
