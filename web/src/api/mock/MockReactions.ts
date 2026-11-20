import { KinigeApiError } from "../KinigeApi";
import { LIMITS, mockJournalDate, yearsLater } from "../mockRules";
import { isReactionEmoji, pushRecentReaction, quickReactions, reactionKey } from "../mockReactions";
import type { MessageView } from "../generated/MessageView";
import type { MockState } from "./MockState";

/**
 * Reactions in `MockApi` (slice 5a): the rules of Rust that matter to the window, pinned by `reaction-flow.json`. Put and remove, the
 * caps (10 live, 32 different ever), the order put, the recent list that feeds the hover bar. Rough on purpose: the mock knows only the
 * UTC zone, so a reaction's day is the UTC one, and an emoji is checked by the pictograph property of the engine, not by Rust's table.
 */
export class MockReactions {
  constructor(private readonly s: MockState) {}

  setMessageReaction(id: string, emoji: string, on: boolean): Promise<MessageView> {
    return this.s.run("setMessageReaction", `${id} ${emoji} ${on}`, () => {
      const row = this.s.rows.find((r) => r.view.id === id);
      if (!row) throw new KinigeApiError("message_not_found");
      if (row.deletedAt !== null) throw new KinigeApiError("message_in_trash");
      this.s.writable(row);
      if (!isReactionEmoji(emoji)) throw new KinigeApiError("reaction_invalid");
      const key = reactionKey(emoji);
      const view = row.view;
      const had = (row.reactionKeys ??= new Set(view.reactions.map((reaction) => reactionKey(reaction.emoji))));
      const live = view.reactions;
      const there = live.some((reaction) => reactionKey(reaction.emoji) === key);
      if (on === there) return this.s.viewOf(row);
      if (on) {
        if (live.length >= LIMITS.reactionsLive) throw new KinigeApiError("reaction_limit");
        if (!had.has(key) && Math.max(view.reactionKeysHad, had.size) >= LIMITS.reactionKeys) throw new KinigeApiError("reaction_limit");
        const now = this.s.now();
        const put = { emoji, putAt: now.getTime(), putTz: "UTC", yearsLater: yearsLater(view.journalDate, mockJournalDate(now)) };
        had.add(key);
        row.view = { ...view, reactions: [...live, put], reactionKeysHad: Math.max(view.reactionKeysHad, had.size) };
        const recent = pushRecentReaction(this.s.settings.recentReactions, emoji);
        this.s.settings = { ...this.s.settings, recentReactions: recent, quickReactions: quickReactions(recent) };
      } else {
        row.view = { ...view, reactions: live.filter((reaction) => reactionKey(reaction.emoji) !== key) };
      }
      return this.s.viewOf(row);
    });
  }
}
