import { INBOX_ID } from "./KinigeApi";
import { mockTags } from "./mockTags";
import type { ChatSummary } from "./generated/ChatSummary";
import type { Language } from "./generated/Language";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";

export interface MockApiOptions {
  /** "Now" for new entries and the "today" label; the window clock by default. */
  now?: () => Date;
  language?: Language;
  /** Entries that are already in the Inbox (and, if they have another `chatId`, in the matching chats). */
  messages?: readonly MessageView[];
  /** Chats other than the Inbox, in the order of the "Чаты" section. */
  chats?: readonly Partial<ChatSummary>[];
  settings?: Settings;
  /** The next folder the system dialog will "pick"; `null` - the user cancelled. */
  pickedFolder?: string | null;
  /** The startup state; the journal is open by default. */
  startup?: StartupState;
}

export const DEFAULT_SHORTCUTS: Record<ShortcutKind, string> = {
  capture: "Ctrl+Alt+KeyN",
  clipboard: "Alt+Shift+KeyV",
};

/** Default settings for tests and stories (all `Settings` fields). */
export const DEFAULT_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [
    { kind: "capture", accelerator: DEFAULT_SHORTCUTS.capture, isDefault: true, state: "assigned" },
    { kind: "clipboard", accelerator: DEFAULT_SHORTCUTS.clipboard, isDefault: true, state: "assigned" },
  ],
  sidebarCollapsed: false,
  forestView: "strip",
  forestMode: "forest",
  recentEmoji: [],
  backup: { folder: "/Users/me/Library/Application Support/me.savvinov.kinige/backups", lastAt: null, count: 0, error: null },
};

/** The append window (PRD §2.1): an entry not older than ten minutes. */
export const APPEND_WINDOW_MS = 10 * 60 * 1000;
/** How many entries the capture window shows above the field. */
export const CONTEXT_LEN = 3;
export const DAY_MS = 24 * 3_600_000;
export const TRASH_DAYS = 30;

/** An entry for tests and stories: all `MessageView` fields with sensible defaults. */
export function mockMessage(part: Partial<MessageView> & Pick<MessageView, "id" | "bodyMd">): MessageView {
  return {
    chatId: INBOX_ID,
    journalDate: "2026-10-21",
    createdAt: Date.UTC(2026, 9, 21, 6, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: mockTags(part.bodyMd),
    readOnly: false,
    origin: "user",
    replyTo: null,
    ...part,
  };
}

export interface Row {
  view: MessageView;
  deletedAt: number | null;
  /** Texts before each edit (`edit` revisions). */
  revisions: string[];
  /** The entry is a reply (slice 5): the addressee entry id; the `view.replyTo` quote is built on read (`MockApiBase.viewOf`). */
  replyToId?: string;
}

/** Full years between the old entry's day and the reply day; 0 if under a year or the reply is earlier (like `reply::years_later` in Rust). */
export function yearsLater(old: string, reply: string): number {
  if (!isRealDate(old) || !isRealDate(reply)) return 0;
  const [oy, om, od] = old.split("-").map(Number) as [number, number, number];
  const [ry, rm, rd] = reply.split("-").map(Number) as [number, number, number];
  const years = ry - oy - (rm * 32 + rd < om * 32 + od ? 1 : 0);
  return Math.max(0, years);
}

/** `YYYY-MM-DD` - an existing date (as in Rust: future dates are allowed). */
export function isRealDate(text: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const [year, month, day] = text.split("-").map(Number);
  if ((year ?? 0) < 1000) return false;
  const date = new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
  return date.getUTCFullYear() === year && date.getUTCMonth() === (month ?? 1) - 1 && date.getUTCDate() === day;
}

/** Feed order `(journal_date, created_at, id)`. */
export function compareFeed(
  a: { journalDate: string; createdAt: number; id: string },
  b: { journalDate: string; createdAt: number; id: string },
): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}
