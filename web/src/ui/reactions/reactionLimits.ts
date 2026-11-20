import type { ReactionView } from "../../api/generated/ReactionView";
import { MAX_LIVE_REACTIONS, MAX_REACTION_KEYS, reactionKey } from "./reactionKey";

/** Why an entry cannot take a reaction: it holds the most live ones, or it has already had the most different emoji. */
export type ReactionLimit = "live" | "keys";

/**
 * What the caps count: every item the entry's group holds, and every live one, hidden items (a group from elsewhere, an item that shows nothing)
 * included, so the interface refuses exactly what the store would.
 */
export interface ReactionCounts {
  key: number;
  live: number;
}

/** What the interface knows of an entry's reactions: the live ones, the keys of every emoji it has had and, when known, what the caps count. */
export interface ReactionState {
  reactions: readonly ReactionView[];
  keysHad: readonly string[];
  counts?: ReactionCounts;
}

// The counts are never below what the lists show (Rust guarantees it); the larger is taken so that a caller that has no counts, or zeros, is still right.
const liveCount = (state: ReactionState) => Math.max(state.counts?.live ?? 0, state.reactions.length);
const keyCount = (state: ReactionState) => Math.max(state.counts?.key ?? 0, new Set(state.keysHad).size);

/** Whether `emoji` is one of the entry's live reactions. */
export function isLive(state: ReactionState, emoji: string): boolean {
  const key = reactionKey(emoji);
  return state.reactions.some((reaction) => reactionKey(reaction.emoji) === key);
}

/**
 * The reason a put of `emoji` would be refused, or `null` when it would go through. Putting what is live is a removal, never refused; at 10 live
 * ones nothing new goes in; an emoji the entry has had (removed since) still can, but a new one cannot once it has had 32 different ones. A group over
 * the caps that came from elsewhere is shown in full and refuses a put by the same rules.
 */
export function putRefusal(state: ReactionState, emoji: string): ReactionLimit | null {
  if (isLive(state, emoji)) return null;
  if (liveCount(state) >= MAX_LIVE_REACTIONS) return "live";
  const had = new Set(state.keysHad);
  return !had.has(reactionKey(emoji)) && keyCount(state) >= MAX_REACTION_KEYS ? "keys" : null;
}

/** The "picker" button adds an emoji that is not known yet: it is shut only when nothing at all can be added (10 live). */
export function pickerRefusal(state: ReactionState): ReactionLimit | null {
  return liveCount(state) >= MAX_LIVE_REACTIONS ? "live" : null;
}
