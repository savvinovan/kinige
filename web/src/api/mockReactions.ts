import { LIMITS } from "./mockRules";

// The rules of reactions (slice 5a) that `MockApi` repeats from Rust, pinned by `fixtures/api-contract/reaction-key.json` and
// `reaction-flow.json`. The emoji check is rough on purpose in one way: Rust has a table of pictographs of Unicode Emoji 16 and the
// mock uses the pictograph property of the JavaScript engine; the fixture holds the cases the two agree on.

/** U+FE0F, written as a code point so that it is visible in the source. */
const VS16 = String.fromCodePoint(0xfe0f);
/** The fixed quick set that follows the recent reactions: heart, face with tears of joy, crying face. */
export const QUICK_DEFAULTS: readonly string[] = [`❤${VS16}`, "\u{1F602}", "\u{1F622}"];

/** One emoji sequence: a keycap, two regional indicators, a subdivision flag, or pictographs (each with VS16 or a skin tone) joined by ZWJ. */
const EMOJI_SEQUENCE = new RegExp(
  "^(?:[0-9#*]\\uFE0F?\\u20E3" +
    "|\\p{Regional_Indicator}{2}" +
    "|\\u{1F3F4}[\\u{E0020}-\\u{E007E}]+\\u{E007F}" +
    "|\\p{Extended_Pictographic}(?:\\uFE0F|\\p{Emoji_Modifier})?(?:\\u200D\\p{Extended_Pictographic}(?:\\uFE0F|\\p{Emoji_Modifier})?)*)$",
  "u",
);

/** The comparison key of an emoji: the string without U+FE0F (`reactions::reaction_key` in Rust). */
export function reactionKey(emoji: string): string {
  return emoji.replaceAll(VS16, "");
}

/** Whether a text is one emoji of at most 40 bytes (`emoji::is_emoji_sequence` in Rust). */
export function isReactionEmoji(text: string): boolean {
  return text !== "" && new TextEncoder().encode(text).length <= LIMITS.chatIconBytes && EMOJI_SEQUENCE.test(text);
}

/** The three emoji of the hover bar: the recent reactions, then the fixed set, without repeats by key. */
export function quickReactions(recent: readonly string[]): string[] {
  const seen = new Set<string>();
  return [...recent, ...QUICK_DEFAULTS].filter((emoji) => !seen.has(reactionKey(emoji)) && seen.add(reactionKey(emoji))).slice(0, LIMITS.reactionsQuick);
}

/** The recent list after a reaction is put: it goes first, an earlier entry with the same key goes out, the list is capped. */
export function pushRecentReaction(recent: readonly string[], emoji: string): string[] {
  const key = reactionKey(emoji);
  return [emoji, ...recent.filter((known) => reactionKey(known) !== key)].slice(0, LIMITS.reactionsRecent);
}
