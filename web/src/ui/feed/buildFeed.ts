import type { ReplyTo } from "../../api/generated/ReplyTo";
import { continuesSeries } from "./dates";

/** A message for the feed: the fields of the real Rust response (`MessageView`) plus two view states from D1. */
export interface FeedMessage {
  id: string;
  /** UTC, ms. */
  createdAt: number;
  /** The IANA zone in which the entry was created: the time is shown in it. */
  createdTz: string;
  /** The entry's day (`YYYY-MM-DD`): day labels and series are based on it; Rust computes it (day boundary 04:00). */
  journalDate: string;
  /** Text in Markdown. */
  bodyMd: string;
  /** "failed" is the "Not sent" bubble (the slice 1 app does not have it: on error the text stays in the field; the state remained in Storybook). */
  status?: "sent" | "failed";
  highlighted?: boolean;
  /** The entry is a task (a checkbox on the bubble). */
  isTask?: boolean;
  /** When the task was completed (ms); `null` or absent means not completed. */
  taskDoneAt?: number | null;
  /** When it was added to favorites (ms). */
  favoriteAt?: number | null;
  /** The entry's tags in lower case, without `#` (parsed by Rust): the text is highlighted by them. */
  tags?: readonly string[];
  /** The entry is newer than the supported version: read-only, actions unavailable. */
  readOnly?: boolean;
  /** Who created the entry (`core.origin`): `agent` is a quiet "added by an agent" mark in the time line. */
  origin?: string;
  /** The entry this is a reply to (slice 5): a quote in the bubble. */
  replyTo?: ReplyTo | null;
}

export type FeedItem =
  | { kind: "day"; key: string; journalDate: string }
  /** The "On this day" card (slice 5): stands under its own day label, before the entries of that day. */
  | { kind: "card"; key: "card" }
  | { kind: "message"; key: string; message: FeedMessage; /** Continues a series: the gap above is smaller. */ inSeries: boolean };

/** Feed and cursor order (§4.1 of the spec): `(journal_date, created_at, id)`. */
function byFeedOrder(a: FeedMessage, b: FeedMessage): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * The feed: a day label before the first message of each `journal_date`, every message has a series flag (pause < 5 minutes,
 * same day). The order is `(journal_date, created_at, id)`, as in Rust; there is no merging into "pages", each has its own time (spec §6).
 */
export function buildFeed(messages: readonly FeedMessage[], cardDate?: string): FeedItem[] {
  const sorted = [...messages].sort(byFeedOrder);
  const items: FeedItem[] = [];
  let previous: FeedMessage | undefined;
  let cardPlaced = cardDate === undefined;
  const placeCard = (date: string) => {
    items.push({ kind: "day", key: `day-${date}`, journalDate: date }, { kind: "card", key: "card" });
    cardPlaced = true;
  };
  for (const message of sorted) {
    if (!previous || previous.journalDate !== message.journalDate) {
      // The card stands under its own day label before its entries; with no entries that day, in its place by date.
      if (!cardPlaced && cardDate !== undefined && message.journalDate > cardDate) placeCard(cardDate);
      items.push({ kind: "day", key: `day-${message.journalDate}`, journalDate: message.journalDate });
      if (!cardPlaced && message.journalDate === cardDate) {
        items.push({ kind: "card", key: "card" });
        cardPlaced = true;
      }
    }
    items.push({
      kind: "message",
      key: message.id,
      message,
      inSeries: previous !== undefined && continuesSeries(previous, message),
    });
    previous = message;
  }
  if (!cardPlaced && cardDate !== undefined) placeCard(cardDate);
  return items;
}
