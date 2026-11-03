import { INBOX_ID, KinigeApiError } from "./KinigeApi";
import { MockApiSlice5 } from "./MockApiSlice5";
import type { FirstWeekday } from "./generated/FirstWeekday";
import type { ForestMode } from "./generated/ForestMode";
import type { ForestTarget } from "./generated/ForestTarget";
import type { ForestView } from "./generated/ForestView";
import type { ForestYear } from "./generated/ForestYear";
import type { WeekdaySource } from "./generated/WeekdaySource";
import { mockDays, type MockDayFacts } from "./mockForest";
import { compareFeed, isRealDate } from "./mockData";

const EXCERPT_CHARS = 120;

/**
 * Slice 6 in `MockApi`: the year table of the forest, the chat a click on a day opens, and the view state. The real rules are in
 * Rust (`forest_year`); this keeps what tests and stories need: live entries of all chats count (the mock has no chat trash),
 * the same stage formula, the same choice of chat.
 */
export class MockApiSlice6 extends MockApiSlice5 {
  /** The weekday sources the window reported (`getForestYear`, once per session in the app). */
  readonly weekdaySources: WeekdaySource[] = [];

  getForestYear(year: number, firstWeekday: FirstWeekday, weekdaySource?: WeekdaySource): Promise<ForestYear> {
    return this.run("getForestYear", `${year} ${firstWeekday}`, () => {
      if (!Number.isInteger(year) || year < 1000 || year > 9999) throw new KinigeApiError("invalid_date");
      if (weekdaySource !== undefined) this.weekdaySources.push(weekdaySource);
      const today = this.journalDate(this.now());
      const facts = new Map<string, MockDayFacts>();
      let oldest = Number.POSITIVE_INFINITY;
      for (const message of this.allLive()) {
        oldest = Math.min(oldest, Number(message.journalDate.slice(0, 4)));
        if (!message.journalDate.startsWith(`${String(year).padStart(4, "0")}-`)) continue;
        const day = facts.get(message.journalDate) ?? { lengths: [], excerpt: null };
        day.lengths.push([...message.bodyMd].length);
        day.excerpt ??= excerptOf(message.bodyMd);
        facts.set(message.journalDate, day);
      }
      const { cols, days } = mockDays(year, firstWeekday, facts);
      return {
        year,
        firstWeekday,
        cols,
        minYear: Math.min(Number.isFinite(oldest) ? oldest : Number(today.slice(0, 4)), Number(today.slice(0, 4))),
        today,
        days,
      };
    });
  }

  getForestDayTarget(date: string, currentChatId: string | null): Promise<ForestTarget | null> {
    return this.run("getForestDayTarget", `${date} ${currentChatId ?? ""}`, () => {
      if (!isRealDate(date)) throw new KinigeApiError("invalid_date");
      const byChat = new Map<string, number>();
      for (const message of this.allLive()) if (message.journalDate === date) byChat.set(message.chatId, (byChat.get(message.chatId) ?? 0) + 1);
      if (byChat.size === 0) return null;
      const chosen =
        currentChatId !== null && byChat.has(currentChatId)
          ? currentChatId
          : [...byChat].sort(
              ([aId, a], [bId, b]) => b - a || Number(bId === INBOX_ID) - Number(aId === INBOX_ID) || (aId < bId ? -1 : aId > bId ? 1 : 0),
            )[0]?.[0];
      const first = this.live(chosen ?? INBOX_ID).find((message) => message.journalDate === date);
      return first && chosen ? { chatId: chosen, messageId: first.id } : null;
    });
  }

  setForestView(view: ForestView, mode: ForestMode): Promise<void> {
    return this.run("setForestView", `${view} ${mode}`, () => {
      this.settings = { ...this.settings, forestView: view, forestMode: mode };
    });
  }

  /** Live entries of every chat in feed order. */
  private allLive() {
    return this.rows
      .filter((row) => row.deletedAt === null)
      .map((row) => row.view)
      .sort(compareFeed);
  }
}

/** The start of an entry as one plain line (rough: the real text comes from Rust). */
function excerptOf(bodyMd: string): string | null {
  const text = bodyMd.replace(/[*_`#>~]|\[[ xX]\]/g, "").replace(/\s+/g, " ").trim();
  if (text === "") return null;
  return [...text].length > EXCERPT_CHARS ? `${[...text].slice(0, EXCERPT_CHARS - 1).join("").trimEnd()}…` : text;
}
