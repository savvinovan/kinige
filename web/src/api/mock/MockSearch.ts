import { resolveColor } from "../mockColor";
import { compareFeed } from "../mockData";
import { isRealDate, pageLimit } from "../mockRules";
import { KinigeApiError } from "../KinigeApi";
import type { FeedCursor } from "../generated/FeedCursor";
import type { MessageView } from "../generated/MessageView";
import type { SearchFilters } from "../generated/SearchFilters";
import type { SearchHit } from "../generated/SearchHit";
import type { SearchPage } from "../generated/SearchPage";
import type { MockState } from "./MockState";

// Cyrillic "yo" -> "ye" (no Cyrillic literals in code: UI strings live in locales).
const YO = String.fromCharCode(0x451);
const YE = String.fromCharCode(0x435);
/** The search page when the caller gives no limit (Rust keeps it in `src-tauri`, out of reach of the fixtures: not pinned). */
const SEARCH_PAGE = 50;
const normalize = (text: string) => text.toLowerCase().replaceAll(YO, YE);
const cursorOf = (m: { journalDate: string; createdAt: number; id: string }): FeedCursor => ({
  journalDate: m.journalDate,
  createdAt: m.createdAt,
  id: m.id,
});

/**
 * Search in `MockApi`: a word substring without stemming is fine in the mock (the real word forms are computed by Rust); the filters,
 * the date check, the chats that match and the paging are the same.
 */
export class MockSearch {
  constructor(private readonly s: MockState) {}

  searchMessages(
    query: string,
    options: { filters?: SearchFilters; before?: FeedCursor; limit?: number } = {},
  ): Promise<SearchPage> {
    return this.s.run("searchMessages", query, () => {
      const filters: SearchFilters = options.filters ?? { chatId: null, favorite: false, tasks: false, from: null, to: null };
      // As in Rust: a query with no words gives an empty page before the dates are looked at.
      const words = normalize(query).split(/[^\p{L}\p{N}]+/u).filter((w) => w !== "");
      if (words.length === 0) return { chats: [], hits: [], older: null };
      for (const date of [filters.from, filters.to]) {
        if (date != null && !isRealDate(date)) throw new KinigeApiError("invalid_date");
      }
      const limit = options.limit === undefined ? SEARCH_PAGE : pageLimit(options.limit);
      const matches = this.s.rows
        .filter((r) => r.deletedAt === null)
        .map((r) => r.view)
        .filter((m) => {
          const text = normalize(m.bodyMd);
          return words.every((w) => text.includes(w));
        })
        .filter((m) => !filters.chatId || m.chatId === filters.chatId)
        .filter((m) => !filters.favorite || m.favoriteAt !== null)
        .filter((m) => !filters.tasks || m.isTask)
        .filter((m) => !filters.from || m.journalDate >= filters.from)
        .filter((m) => !filters.to || m.journalDate <= filters.to)
        .filter((m) => !options.before || compareFeed(m, options.before) < 0)
        .sort((a, b) => compareFeed(b, a));
      const page = matches.slice(0, limit);
      const hits: SearchHit[] = page.map((message) => {
        const chat = this.s.chats.find((c) => c.id === message.chatId);
        const terms = [
          ...new Set(
            (message.bodyMd.match(/[\p{L}\p{N}]+/gu) ?? []).filter((w) => words.some((q) => normalize(w).includes(q))),
          ),
        ];
        return {
          message,
          chatTitle: chat?.title ?? "",
          chatIcon: chat?.icon ?? null,
          chatIsInbox: chat?.isInbox ?? false,
          chatColor: chat?.color ?? resolveColor(message.chatId, null),
          snippet: message.bodyMd.replace(/\s+/g, " ").slice(0, 160),
          terms,
        };
      });
      const lastHit = page.at(-1);
      const chats = options.before ? [] : this.s.chats.filter((c) => !c.isInbox && words.every((w) => normalize(c.title).includes(w)));
      return {
        chats,
        hits,
        older: matches.length > limit && lastHit ? cursorOf(lastHit) : null,
      };
    });
  }

  /** Every live entry, for checks. */
  searchable(): readonly MessageView[] {
    return this.s.rows.filter((r) => r.deletedAt === null).map((r) => r.view);
  }
}
