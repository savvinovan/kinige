import { INBOX_ID, KinigeApiError } from "./KinigeApi";
import { MockApiSlice4b } from "./MockApiSlice4b";
import { compareFeed, isRealDate, mockMessage } from "./mockData";
import type { ChatDraft } from "./generated/ChatDraft";
import type { MessageView } from "./generated/MessageView";
import type { OnThisDay } from "./generated/OnThisDay";
import type { OnThisDayYear } from "./generated/OnThisDayYear";

/** How many years the card shows in detail and how many entries per year (like `kinige_domain::on_this_day`). */
const DETAILED_YEARS = 3;
const ENTRIES_PER_YEAR = 2;

/**
 * Slice 5 in `MockApi`: a reply to an entry (addressee check, the quote in the feed, a draft with a reply) and the "On this
 * day" card. The real rules are in Rust; here is what tests and stories need: the same `live` / `trash` / `missing`
 * states, the same order and content of the card, hiding until the next journal day.
 */
export class MockApiSlice5 extends MockApiSlice4b {
  /** The journal day until whose end the card is hidden (`dismissOnThisDay`). */
  dismissedOnThisDay: string | null = null;

  sendMessage(chatId: string, body: string, replyToId: string | null = null): Promise<MessageView> {
    if (this.failNextSend) {
      const code = this.failNextSend;
      this.failNextSend = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "") return Promise.reject(new KinigeApiError("empty_message"));
    if (replyToId !== null && !this.replyTargetLives(replyToId, chatId)) {
      return Promise.reject(new KinigeApiError("reply_target_not_found"));
    }
    if (!this.chats.some((c) => c.id === chatId)) return Promise.reject(new KinigeApiError("chat_not_found"));
    const now = this.now();
    const journalDate = this.journalDate(now);
    const message = mockMessage({
      id: `mock-${++this.counter}`,
      chatId,
      bodyMd: text,
      journalDate,
      createdAt: now.getTime(),
    });
    const row = { view: message, deletedAt: null, revisions: [], replyToId: replyToId ?? undefined };
    this.rows.push(row);
    // As in Rust: sending clears this chat's draft together with the reply line.
    this.drafts.delete(chatId);
    this.replyDrafts.delete(chatId);
    return Promise.resolve(this.viewOf(row));
  }

  getDraft(chatId: string): Promise<ChatDraft> {
    const target = this.replyDrafts.get(chatId);
    let replyTo = target === undefined ? null : this.replyTo(target, this.journalDate(this.now()));
    // As in Rust: the entry is alive but in another chat - "moved" (a reply to it is not accepted).
    if (replyTo !== null && replyTo.state === "live" && replyTo.chatId !== chatId) replyTo = { ...replyTo, state: "moved" };
    return Promise.resolve({ body: this.drafts.get(chatId) ?? "", replyTo, attachments: [] });
  }

  saveDraft(chatId: string, body: string, replyToId: string | null = null): Promise<void> {
    if (replyToId !== null && replyToId === "") return Promise.reject(new KinigeApiError("reply_target_not_found"));
    if (body === "") this.drafts.delete(chatId);
    else this.drafts.set(chatId, body);
    if (replyToId === null) this.replyDrafts.delete(chatId);
    else this.replyDrafts.set(chatId, replyToId);
    return Promise.resolve();
  }

  getOnThisDay(): Promise<OnThisDay | null> {
    return this.run("getOnThisDay", "", () => {
      const today = this.journalDate(this.now());
      if (this.dismissedOnThisDay === today) return null;
      const [year, monthDay] = [Number(today.slice(0, 4)), today.slice(5)];
      const byDate = new Map<string, MessageView[]>();
      for (const message of this.live(INBOX_ID)) {
        if (message.journalDate.slice(5) !== monthDay || Number(message.journalDate.slice(0, 4)) >= year) continue;
        byDate.set(message.journalDate, [...(byDate.get(message.journalDate) ?? []), message]);
      }
      const dates = [...byDate.keys()].sort().reverse();
      const years: OnThisDayYear[] = dates.map((date, index) => {
        const messages = [...(byDate.get(date) ?? [])].sort(compareFeed);
        const detailed = index < DETAILED_YEARS;
        return {
          journalDate: date,
          yearsAgo: year - Number(date.slice(0, 4)),
          total: messages.length,
          firstId: messages[0]?.id ?? "",
          entries: detailed
            ? messages.slice(0, ENTRIES_PER_YEAR).map((message) => ({ message, text: plainPreview(message.bodyMd) }))
            : [],
        };
      });
      return years.length === 0 ? null : { today, years };
    });
  }

  dismissOnThisDay(date: string): Promise<void> {
    return this.run("dismissOnThisDay", date, () => {
      if (!isRealDate(date)) throw new KinigeApiError("invalid_date");
      this.dismissedOnThisDay = date;
    });
  }

  /** The reply addressee is alive and lies in the same chat. */
  private replyTargetLives(targetId: string, chatId: string): boolean {
    return this.rows.some((r) => r.view.id === targetId && r.view.chatId === chatId && r.deletedAt === null);
  }
}

/** Plain text of an entry on one line (rough in the mock; the real one is computed by Rust). */
function plainPreview(bodyMd: string): string {
  const text = bodyMd.replace(/[*_`#>~]|\[[ xX]\]/g, "").replace(/\s+/g, " ").trim();
  return [...text].length > 300 ? `${[...text].slice(0, 299).join("").trimEnd()}…` : text;
}
