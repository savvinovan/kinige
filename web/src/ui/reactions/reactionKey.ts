/** U+FE0F, written as a code point so that it is visible in the source. */
const VS16 = String.fromCodePoint(0xfe0f);

/**
 * The comparison key of an emoji: the string without U+FE0F (`reactions::reaction_key` in Rust, pinned by `fixtures/api-contract/reaction-key.json`).
 * So the heart with and without the variation selector is one reaction.
 */
export function reactionKey(emoji: string): string {
  return emoji.replaceAll(VS16, "");
}

/** An entry holds at most this many live reactions and has had at most this many different ones (`reactions::{MAX_LIVE, MAX_KEYS}`). */
export const MAX_LIVE_REACTIONS = 10;
export const MAX_REACTION_KEYS = 32;
