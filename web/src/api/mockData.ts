import { INBOX_ID } from "./KinigeApi";
import { LIMITS } from "./mockRules";
import { mockTags } from "./mockTags";
import type { ChatSummary } from "./generated/ChatSummary";
import type { Language } from "./generated/Language";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";

// The rules moved to `mockRules.ts`; the names the stories and tests import stay here.
export { DAY_MS, NO_ATTACHMENTS, isRealDate, yearsLater } from "./mockRules";
export const APPEND_WINDOW_MS = LIMITS.appendWindowMs;
export const TRASH_DAYS = LIMITS.trashDays;

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

/** How many entries the capture window shows above the field. */
export const CONTEXT_LEN = 3;

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
    attachments: [],
    ...part,
  };
}

export interface Row {
  view: MessageView;
  deletedAt: number | null;
  /** Texts before each edit (`edit` revisions). */
  revisions: string[];
  /** The entry is a reply (slice 5): the addressee entry id; the `view.replyTo` quote is built on read (`MockState.viewOf`). */
  replyToId?: string;
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

/** The media fields of an import preview chat that has none (slice 7). */
export const NO_MEDIA = { mediaFiles: 0, mediaBytes: 0, mediaMissing: 0 } as const;
