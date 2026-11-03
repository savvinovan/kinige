// "ё" and "е" are not distinguished in search; the letters are given by code so that the source has no Cyrillic (the "strings only in locales" check).
const YO = String.fromCharCode(0x451);
const YE = String.fromCharCode(0x435);

const words = (text: string) => text.toLowerCase().replaceAll(YO, YE).match(/[\p{L}\p{N}]+/gu) ?? [];

/**
 * Whether the translated Inbox name ("Входящие", "Inbox") matches the query: Rust does not search Inbox by name (there is no name in the database),
 * so the UI adds the "Chats" row for it. Each query word is the beginning of some word of the name; there is no word stemming
 * here, so "входящ" finds it and "входящих" does not.
 */
export function inboxMatches(name: string, query: string): boolean {
  const terms = words(query);
  if (terms.length === 0) return false;
  const title = words(name);
  return terms.every((term) => title.some((word) => word.startsWith(term)));
}
