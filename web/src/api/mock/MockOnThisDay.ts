import { INBOX_ID, KinigeApiError } from "../KinigeApi";
import { compareFeed } from "../mockData";
import { LIMITS, isRealDate } from "../mockRules";
import { plainSnippet } from "../mockText";
import type { MessageView } from "../generated/MessageView";
import type { OnThisDay } from "../generated/OnThisDay";
import type { OnThisDayYear } from "../generated/OnThisDayYear";
import type { MockState } from "./MockState";

/**
 * The "On this day" card in `MockApi` (slice 5). The real rules are in Rust; here is what tests and stories need: the same order and
 * content of the card (limits in `fixtures/api-contract/constants.json`), hiding until the next journal day. The text of an entry is
 * plain only roughly: the real Markdown stripping is Rust's.
 */
export class MockOnThisDay {
  /** The journal day until whose end the card is hidden (`dismissOnThisDay`). */
  dismissed: string | null = null;

  constructor(private readonly s: MockState) {}

  getOnThisDay(): Promise<OnThisDay | null> {
    return this.s.run("getOnThisDay", "", () => {
      const today = this.s.journalDate(this.s.now());
      if (this.dismissed === today) return null;
      const [year, monthDay] = [Number(today.slice(0, 4)), today.slice(5)];
      const byDate = new Map<string, MessageView[]>();
      for (const message of this.s.live(INBOX_ID)) {
        if (message.journalDate.slice(5) !== monthDay || Number(message.journalDate.slice(0, 4)) >= year) continue;
        byDate.set(message.journalDate, [...(byDate.get(message.journalDate) ?? []), message]);
      }
      const dates = [...byDate.keys()].sort().reverse();
      const years: OnThisDayYear[] = dates.map((date, index) => {
        const messages = [...(byDate.get(date) ?? [])].sort(compareFeed);
        const detailed = index < LIMITS.onThisDayDetailedYears;
        return {
          journalDate: date,
          yearsAgo: year - Number(date.slice(0, 4)),
          total: messages.length,
          firstId: messages[0]?.id ?? "",
          entries: detailed
            ? messages
                .slice(0, LIMITS.onThisDayEntriesPerYear)
                .map((message) => ({ message, text: plainSnippet(message.bodyMd, LIMITS.onThisDayPreviewChars) }))
            : [],
        };
      });
      return years.length === 0 ? null : { today, years };
    });
  }

  dismissOnThisDay(date: string): Promise<void> {
    return this.s.run("dismissOnThisDay", date, () => {
      if (!isRealDate(date)) throw new KinigeApiError("invalid_date");
      this.dismissed = date;
    });
  }
}
