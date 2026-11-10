import { INBOX_ID, type KinigeApi, KinigeApiError } from "../KinigeApi";
import { summaryOf } from "../../ui/attachments/summaryText";
import { DEFAULT_SETTINGS, type MockApiOptions, type Row, compareFeed } from "../mockData";
import { INBOX_COLOR, resolveColor } from "../mockColor";
import { LIMITS, emojiIdentity, mockJournalDate, replyQuote } from "../mockRules";
import { mockTags } from "../mockTags";
import type { AttachmentView } from "../generated/AttachmentView";
import type { ChatSummary } from "../generated/ChatSummary";
import type { ErrorCode } from "../generated/ErrorCode";
import type { Language } from "../generated/Language";
import type { MessageView } from "../generated/MessageView";
import type { ReplyTo } from "../generated/ReplyTo";
import type { Settings } from "../generated/Settings";
import type { StartupState } from "../generated/StartupState";

export type MockMethod = keyof KinigeApi;

/**
 * What every area of `MockApi` shares: the entries and chats, the settings, the drafts, the clock, the call log, the command
 * failures and the two events that several areas send. An area keeps its own knobs and counters.
 */
export class MockState {
  readonly now: () => Date;
  readonly language: Language;
  rows: Row[];
  chats: ChatSummary[];
  settings: Settings;
  startup: StartupState;
  counter = 0;
  /** The chat drafts; the entry a draft replies to is in `replyDrafts`; the attachment tiles are in `draftTiles`. */
  readonly drafts = new Map<string, string>();
  readonly replyDrafts = new Map<string, string>();
  readonly draftTiles = new Map<string, AttachmentView[]>();
  /** The command call log: `method arguments` - for checking the order and number of calls. */
  readonly calls: string[] = [];
  private readonly failures = new Map<MockMethod, ErrorCode>();
  private readonly entryListeners = new Set<() => void>();
  private readonly settingsListeners = new Set<() => void>();

  constructor(options: MockApiOptions) {
    this.now = options.now ?? (() => new Date());
    this.language = options.language ?? "ru";
    this.rows = (options.messages ?? []).map((view) => ({ view, deletedAt: null, revisions: [] }));
    this.chats = [
      { id: INBOX_ID, title: "", icon: "📥", isInbox: true, count: 0, pinnedAt: null, archivedAt: null, color: INBOX_COLOR },
      ...(options.chats ?? []).map((chat, index) => {
        const id = chat.id ?? `chat-${index + 1}`;
        return {
          id,
          title: chat.title ?? `Chat ${index + 1}`,
          icon: chat.icon === undefined ? "💬" : chat.icon,
          isInbox: false,
          count: 0,
          pinnedAt: chat.pinnedAt ?? null,
          archivedAt: chat.archivedAt ?? null,
          color: chat.color ?? resolveColor(id, null),
        };
      }),
    ];
    this.settings = options.settings ?? DEFAULT_SETTINGS;
    this.startup = options.startup ?? { needsRecovery: false, backups: [], databaseTooNew: false, corruptionFound: false };
  }

  /** The next call of this command fails with an error (once). */
  failNext(method: MockMethod, code: ErrorCode): void {
    this.failures.set(method, code);
  }

  /** The error the next call of this command is set to fail with, if any; taking it clears it. */
  takeFailure(method: MockMethod): ErrorCode | undefined {
    const code = this.failures.get(method);
    this.failures.delete(method);
    return code;
  }

  /** Runs a command: logs the call, applies a `failNext`, and turns a thrown `KinigeApiError` into a rejection. */
  run<T>(method: MockMethod, args: string, action: () => T): Promise<T> {
    this.calls.push(`${method} ${args}`.trim());
    const code = this.takeFailure(method);
    if (code) return Promise.reject(new KinigeApiError(code));
    try {
      return Promise.resolve(action());
    } catch (error) {
      return Promise.reject(error);
    }
  }

  onEntriesChanged(handler: () => void): Promise<() => void> {
    this.entryListeners.add(handler);
    return Promise.resolve(() => {
      this.entryListeners.delete(handler);
    });
  }

  /** Like the Rust "entries changed" event (in tests, an entry made by the capture window). */
  emitEntriesChanged(): void {
    for (const handler of [...this.entryListeners]) handler();
  }

  onSettingsChanged(handler: () => void): Promise<() => void> {
    this.settingsListeners.add(handler);
    return Promise.resolve(() => {
      this.settingsListeners.delete(handler);
    });
  }

  /** Like the Rust `settings-changed` event: an edit from the window, another window or the file. */
  emitSettingsChanged(): void {
    for (const handler of [...this.settingsListeners]) handler();
  }

  /** The entry day with the 04:00 boundary (simplified, UTC). */
  journalDate(date: Date): string {
    return mockJournalDate(date);
  }

  /** Live (not trashed) entries of a chat in feed order. */
  live(chatId: string): MessageView[] {
    return this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null)
      .map((r) => this.viewOf(r))
      .sort(compareFeed);
  }

  /** Live entries of every chat in feed order. */
  allLive(): MessageView[] {
    return this.rows
      .filter((row) => row.deletedAt === null)
      .map((row) => row.view)
      .sort(compareFeed);
  }

  /** An entry with a reply quote as Rust returns it: the old entry's state is computed on every read (slice 5). */
  viewOf(row: Row): MessageView {
    return row.replyToId === undefined ? row.view : { ...row.view, replyTo: this.quote(row.replyToId, row.view.journalDate) };
  }

  /** The quote for the addressee entry `targetId` on day `replyDate`: live, trashed or deleted. */
  quote(targetId: string, replyDate: string): ReplyTo {
    const target = this.rows.find((r) => r.view.id === targetId);
    return replyQuote(
      targetId,
      replyDate,
      target && {
        chatId: target.view.chatId,
        journalDate: target.view.journalDate,
        bodyMd: target.view.bodyMd,
        inTrash: target.deletedAt !== null,
        attachments: summaryOf(target.view.attachments),
      },
    );
  }

  row(id: string): Row {
    const row = this.rows.find((r) => r.view.id === id);
    if (!row) throw new KinigeApiError("message_not_found");
    return row;
  }

  /** An entry from a newer app version (or an import) is read-only. */
  writable(row: Row): void {
    if (row.view.readOnly) throw new KinigeApiError("read_only");
  }

  chat(id: string): ChatSummary {
    const chat = this.chats.find((c) => c.id === id);
    if (!chat) throw new KinigeApiError("chat_not_found");
    return chat;
  }

  /** The new text of an entry; the old one goes to the revisions and the tags are read again. */
  setBody(row: Row, text: string): MessageView {
    if (text === row.view.bodyMd) return row.view;
    row.revisions.push(row.view.bodyMd);
    row.view = { ...row.view, bodyMd: text, tags: mockTags(text) };
    return row.view;
  }

  /** An emoji from a picker goes first in the shared "recent" list; the same emoji moves to the front; the list is capped. */
  addRecent(emoji: string): string[] {
    const identity = emojiIdentity(emoji);
    const recent = [emoji, ...this.settings.recentEmoji.filter((known) => emojiIdentity(known) !== identity)];
    this.settings = { ...this.settings, recentEmoji: recent.slice(0, LIMITS.recentEmoji) };
    return this.settings.recentEmoji;
  }
}
