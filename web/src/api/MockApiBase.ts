import { INBOX_ID, type KinigeApi, KinigeApiError } from "./KinigeApi";
import { summaryOf } from "../ui/attachments/summaryText";
import { APPEND_WINDOW_MS, CONTEXT_LEN, DEFAULT_SETTINGS, NO_ATTACHMENTS, type MockApiOptions, type Row, compareFeed, mockMessage, yearsLater } from "./mockData";
import { INBOX_COLOR, resolveColor } from "./mockColor";
import { mockTags } from "./mockTags";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { Language } from "./generated/Language";
import type { MessageView } from "./generated/MessageView";
import type { ReplyTo } from "./generated/ReplyTo";
import type { Settings } from "./generated/Settings";
import type { StartupState } from "./generated/StartupState";

export type MockMethod = keyof KinigeApi;

/**
 * The shared `MockApi` state and the capture window: entries, chats, settings, events, the call log and command failures.
 * The main window commands are in `MockApi`.
 */
export class MockApiBase {
  /** Links that were "opened in the system browser". */
  readonly openedUrls: string[] = [];
  /** The next send fails with this error without writing anything. */
  failNextSend: ErrorCode | null = null;
  /** The next folder choice fails with this error. */
  failNextPick: ErrorCode | null = null;
  /** The next shortcut change or reset fails with this error. */
  failNextShortcut: ErrorCode | null = null;
  /** The next send or append from the capture window fails with this error. */
  failNextCapture: ErrorCode | null = null;
  /** The capture window was hidden by the `captureHide` command this many times. */
  hideCount = 0;
  /** Capture window heights that the page asked for with the `captureResize` command. */
  readonly resizeLog: number[] = [];
  /** Shortcut recording mode records: `true` at the start, `false` at the end. */
  readonly recordingLog: boolean[] = [];
  /** The slice 3 command call log: `method arguments` - for checking the order and number of calls. */
  readonly calls: string[] = [];
  /** Restoring from a backup was requested: the backup name. */
  restoredFrom: string | null = null;
  /** A restart for the restore was requested. */
  restartRequested = false;
  pickedFolder: string | null;

  protected rows: Row[];
  protected chats: ChatSummary[];
  protected readonly drafts = new Map<string, string>();
  /** The entry that a chat draft replies to (slice 5). */
  protected readonly replyDrafts = new Map<string, string>();
  protected captureDraft = "";
  protected readonly listeners = new Set<() => void>();
  protected readonly failures = new Map<MockMethod, ErrorCode>();
  protected settings: Settings;
  protected startup: StartupState;
  protected readonly now: () => Date;
  protected readonly language: Language;
  protected counter = 0;

  constructor(options: MockApiOptions = {}) {
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
    this.startup = options.startup ?? {
      needsRecovery: false,
      backups: [],
      databaseTooNew: false,
      corruptionFound: false,
    };
    this.pickedFolder = options.pickedFolder === undefined ? "/Users/me/Notes/Kinige" : options.pickedFolder;
  }

  /** The next call of this command fails with an error (once). */
  failNext(method: MockMethod, code: ErrorCode): void {
    this.failures.set(method, code);
  }

  /** Like the Rust "entries changed" event (in tests, an entry made by the capture window). */
  emitEntriesChanged(): void {
    for (const handler of [...this.listeners]) handler();
  }

  /** An entry that appeared "from outside" (the capture window, the clipboard): reaches the feed after the event. */
  addExternalMessage(message: MessageView): void {
    this.rows.push({ view: message, deletedAt: null, revisions: [] });
  }

  /** Texts before each entry edit (`edit` revisions), oldest first. */
  revisionsOf(id: string): string[] {
    return this.rows.find((r) => r.view.id === id)?.revisions ?? [];
  }

  /** An entry as it is now in the "database" (including from the trash). */
  messageById(id: string): MessageView | undefined {
    return this.rows.find((r) => r.view.id === id)?.view;
  }

  captureState(): Promise<CaptureState> {
    const now = this.now();
    const today = this.journalDate(now);
    const inbox = this.live(INBOX_ID);
    const context = inbox.filter((m) => m.journalDate === today).slice(-CONTEXT_LEN);
    const last = inbox[inbox.length - 1];
    return Promise.resolve({
      language: this.language,
      textSize: this.settings.textSize,
      today,
      context,
      appendTarget: last && this.canAppend(last, now) ? last.id : null,
      draft: this.captureDraft,
      error: null,
      recentEmoji: this.settings.recentEmoji,
    });
  }

  /** The emoji chosen in the capture window picker: the shared "recent" list (the same as the main window's). */
  captureAddRecentEmoji(emoji: string): Promise<string[]> {
    if (emoji.trim() === "") return Promise.reject(new KinigeApiError("chat_icon_invalid"));
    const recent = [emoji, ...this.settings.recentEmoji.filter((known) => known !== emoji)].slice(0, 24);
    this.settings = { ...this.settings, recentEmoji: recent };
    return Promise.resolve(recent);
  }

  captureSend(body: string): Promise<CaptureOutcome> {
    return this.capture(body, false);
  }

  captureAppend(body: string): Promise<CaptureOutcome> {
    return this.capture(body, true);
  }

  captureSaveDraft(body: string): Promise<void> {
    this.captureDraft = body;
    return Promise.resolve();
  }

  captureHide(): Promise<void> {
    this.hideCount += 1;
    return Promise.resolve();
  }

  captureResize(height: number): Promise<void> {
    this.resizeLog.push(height);
    return Promise.resolve();
  }

  /** Live (not trashed) entries of a chat in feed order. */
  protected live(chatId: string): MessageView[] {
    return this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null)
      .map((r) => this.viewOf(r))
      .sort(compareFeed);
  }

  /** An entry with a reply quote as Rust returns it: the old entry's state is computed on every read (slice 5). */
  protected viewOf(row: Row): MessageView {
    return row.replyToId === undefined ? row.view : { ...row.view, replyTo: this.replyTo(row.replyToId, row.view.journalDate) };
  }

  /** The quote for the addressee entry `targetId` on day `replyDate`: live, trashed or "deleted". */
  protected replyTo(targetId: string, replyDate: string): ReplyTo {
    const target = this.rows.find((r) => r.view.id === targetId);
    if (!target) return { id: targetId, chatId: null, journalDate: null, excerpt: "", state: "missing", yearsLater: 0, attachments: NO_ATTACHMENTS };
    const text = target.view.bodyMd.replace(/[*_`#>~]|\[[ xX]\]/g, "").replace(/\s+/g, " ").trim();
    return {
      id: targetId,
      chatId: target.view.chatId,
      journalDate: target.view.journalDate,
      excerpt: [...text].length > 120 ? `${[...text].slice(0, 119).join("").trimEnd()}…` : text,
      state: target.deletedAt === null ? "live" : "trash",
      yearsLater: yearsLater(target.view.journalDate, replyDate),
      attachments: summaryOf(target.view.attachments),
    };
  }

  protected run<T>(method: MockMethod, args: string, action: () => T): Promise<T> {
    this.calls.push(`${method} ${args}`.trim());
    const code = this.failures.get(method);
    if (code) {
      this.failures.delete(method);
      return Promise.reject(new KinigeApiError(code));
    }
    try {
      return Promise.resolve(action());
    } catch (error) {
      return Promise.reject(error);
    }
  }

  protected row(id: string): Row {
    const row = this.rows.find((r) => r.view.id === id);
    if (!row) throw new KinigeApiError("message_not_found");
    return row;
  }

  protected writable(row: Row): void {
    if (row.view.readOnly) throw new KinigeApiError("read_only");
  }

  protected mutableChat(id: string, notInbox: boolean): ChatSummary {
    const chat = this.chats.find((c) => c.id === id);
    if (!chat) throw new KinigeApiError("chat_not_found");
    if (notInbox && chat.isInbox) throw new KinigeApiError("inbox_protected");
    return chat;
  }

  protected setBody(row: Row, text: string): MessageView {
    if (text === row.view.bodyMd) return row.view;
    row.revisions.push(row.view.bodyMd);
    row.view = { ...row.view, bodyMd: text, tags: mockTags(text) };
    return row.view;
  }

  /** The entry's day with the 04:00 boundary (simplified, UTC). */
  protected journalDate(date: Date): string {
    return new Date(date.getTime() - 4 * 3_600_000).toISOString().slice(0, 10);
  }

  private capture(body: string, append: boolean): Promise<CaptureOutcome> {
    if (this.failNextCapture) {
      const code = this.failNextCapture;
      this.failNextCapture = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "") return Promise.reject(new KinigeApiError("empty_message"));
    const now = this.now();
    const inbox = this.live(INBOX_ID);
    const last = inbox[inbox.length - 1];
    this.captureDraft = "";
    if (append && last && this.canAppend(last, now)) {
      const message = this.setBody(this.row(last.id), `${last.bodyMd}\n\n${text}`);
      this.emitEntriesChanged();
      return Promise.resolve({ action: "appended", message });
    }
    const message = mockMessage({
      id: `mock-${++this.counter}`,
      chatId: INBOX_ID,
      bodyMd: text,
      journalDate: this.journalDate(now),
      createdAt: now.getTime(),
    });
    this.rows.push({ view: message, deletedAt: null, revisions: [] });
    this.emitEntriesChanged();
    return Promise.resolve({ action: "created", message });
  }

  /** The append target rule, simplified: today's entry not older than ten minutes (Rust decides the rest). */
  private canAppend(message: MessageView, now: Date): boolean {
    return message.journalDate === this.journalDate(now) && now.getTime() - message.createdAt <= APPEND_WINDOW_MS;
  }
}
