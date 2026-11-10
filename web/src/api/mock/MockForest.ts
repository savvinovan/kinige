import { KinigeApiError } from "../KinigeApi";
import { type MockDayFacts, mockDays } from "../mockForest";
import { LIMITS, dayChat, isRealDate } from "../mockRules";
import { plainSnippet } from "../mockText";
import type { FirstWeekday } from "../generated/FirstWeekday";
import type { ForestMode } from "../generated/ForestMode";
import type { ForestTarget } from "../generated/ForestTarget";
import type { ForestView } from "../generated/ForestView";
import type { ForestYear } from "../generated/ForestYear";
import type { WeekdaySource } from "../generated/WeekdaySource";
import type { MockState } from "./MockState";

/**
 * The year's forest in `MockApi` (slice 6): the year table, the chat a click on a day opens, and the view state. The real rules are in
 * Rust (`forest_year`); the stage formula and the choice of chat are pinned by `fixtures/api-contract/`. Simplified on purpose: the
 * day's excerpt is the first entry with a text (Rust takes the first entry and falls back to its attachment summary), and live entries
 * of all chats count (the mock has no chat trash).
 */
export class MockForest {
  /** The weekday sources the window reported (`getForestYear`, once per session in the app). */
  readonly weekdaySources: WeekdaySource[] = [];

  constructor(private readonly s: MockState) {}

  getForestYear(year: number, firstWeekday: FirstWeekday, weekdaySource?: WeekdaySource): Promise<ForestYear> {
    return this.s.run("getForestYear", `${year} ${firstWeekday}`, () => {
      if (!Number.isInteger(year) || year < 1000 || year > 9999) throw new KinigeApiError("invalid_date");
      if (weekdaySource !== undefined) this.weekdaySources.push(weekdaySource);
      const today = this.s.journalDate(this.s.now());
      const facts = new Map<string, MockDayFacts>();
      let oldest = Number.POSITIVE_INFINITY;
      for (const message of this.s.allLive()) {
        oldest = Math.min(oldest, Number(message.journalDate.slice(0, 4)));
        if (!message.journalDate.startsWith(`${String(year).padStart(4, "0")}-`)) continue;
        const day = facts.get(message.journalDate) ?? { lengths: [], attachments: [], excerpt: null };
        day.lengths.push([...message.bodyMd].length);
        day.attachments?.push(message.attachments.length);
        day.excerpt ??= excerptOf(message.bodyMd);
        day.photos = (day.photos ?? 0) + message.attachments.filter((a) => a.kind === "image").length;
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
    return this.s.run("getForestDayTarget", `${date} ${currentChatId ?? ""}`, () => {
      if (!isRealDate(date)) throw new KinigeApiError("invalid_date");
      const byChat = new Map<string, number>();
      for (const message of this.s.allLive()) if (message.journalDate === date) byChat.set(message.chatId, (byChat.get(message.chatId) ?? 0) + 1);
      const chosen = dayChat([...byChat], currentChatId);
      if (chosen === null) return null;
      const first = this.s.live(chosen).find((message) => message.journalDate === date);
      return first ? { chatId: chosen, messageId: first.id } : null;
    });
  }

  setForestView(view: ForestView, mode: ForestMode): Promise<void> {
    return this.s.run("setForestView", `${view} ${mode}`, () => {
      this.s.settings = { ...this.s.settings, forestView: view, forestMode: mode };
    });
  }
}

/** The start of an entry as one plain line; none for an entry with no text. */
function excerptOf(bodyMd: string): string | null {
  const text = plainSnippet(bodyMd, LIMITS.forestExcerptChars);
  return text === "" ? null : text;
}
