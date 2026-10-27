import { INBOX_ID, type KinigeApi, KinigeApiError } from "./KinigeApi";
import { APPEND_WINDOW_MS, CONTEXT_LEN, DEFAULT_SETTINGS, type MockApiOptions, type Row, compareFeed, mockMessage } from "./mockData";
import { mockTags } from "./mockTags";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { Language } from "./generated/Language";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { StartupState } from "./generated/StartupState";

export type MockMethod = keyof KinigeApi;

/**
 * Общее состояние `MockApi` и окно захвата: записи, чаты, настройки, события, журнал вызовов и отказы команд. Команды
 * главного окна — в `MockApi`.
 */
export class MockApiBase {
  /** Ссылки, которые «открыты в системном браузере». */
  readonly openedUrls: string[] = [];
  /** Следующая отправка завершится этой ошибкой, ничего не записав. */
  failNextSend: ErrorCode | null = null;
  /** Следующий выбор папки завершится этой ошибкой. */
  failNextPick: ErrorCode | null = null;
  /** Следующая смена или сброс сочетания завершится этой ошибкой. */
  failNextShortcut: ErrorCode | null = null;
  /** Следующая отправка или дописывание из окна захвата завершится этой ошибкой. */
  failNextCapture: ErrorCode | null = null;
  /** Окно захвата спрятано командой `captureHide` столько раз. */
  hideCount = 0;
  /** Высоты окна захвата, которые страница просила командой `captureResize`. */
  readonly resizeLog: number[] = [];
  /** Записи режима записи сочетания: `true` при начале, `false` в конце. */
  readonly recordingLog: boolean[] = [];
  /** Журнал вызовов команд слайса 3: `метод аргументы` — для проверок порядка и числа вызовов. */
  readonly calls: string[] = [];
  /** Восстановление из копии запрошено: имя копии. */
  restoredFrom: string | null = null;
  /** Перезапуск для восстановления запрошен. */
  restartRequested = false;
  pickedFolder: string | null;

  protected rows: Row[];
  protected chats: ChatSummary[];
  protected readonly drafts = new Map<string, string>();
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
      { id: INBOX_ID, title: "", icon: "📥", isInbox: true, count: 0, pinnedAt: null, archivedAt: null },
      ...(options.chats ?? []).map((chat, index) => ({
        id: chat.id ?? `chat-${index + 1}`,
        title: chat.title ?? `Chat ${index + 1}`,
        icon: chat.icon === undefined ? "💬" : chat.icon,
        isInbox: false,
        count: 0,
        pinnedAt: chat.pinnedAt ?? null,
        archivedAt: chat.archivedAt ?? null,
      })),
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

  /** Следующий вызов этой команды завершится ошибкой (один раз). */
  failNext(method: MockMethod, code: ErrorCode): void {
    this.failures.set(method, code);
  }

  /** Как событие Rust «записи изменились» (в тестах — запись, сделанная окном захвата). */
  emitEntriesChanged(): void {
    for (const handler of [...this.listeners]) handler();
  }

  /** Запись, появившаяся «снаружи» (окно захвата, буфер): попадает в ленту после события. */
  addExternalMessage(message: MessageView): void {
    this.rows.push({ view: message, deletedAt: null, revisions: [] });
  }

  /** Тексты до каждой правки записи (ревизии `edit`), старые первыми. */
  revisionsOf(id: string): string[] {
    return this.rows.find((r) => r.view.id === id)?.revisions ?? [];
  }

  /** Запись как она сейчас в «базе» (в том числе из корзины). */
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
    });
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

  /** Живые (не из корзины) записи чата по порядку ленты. */
  protected live(chatId: string): MessageView[] {
    return this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null)
      .map((r) => r.view)
      .sort(compareFeed);
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

  /** День записи с границей 04:00 (упрощённо, UTC). */
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

  /** Правило цели дописывания, упрощённо: сегодняшняя запись не старше десяти минут (остальное решает Rust). */
  private canAppend(message: MessageView, now: Date): boolean {
    return message.journalDate === this.journalDate(now) && now.getTime() - message.createdAt <= APPEND_WINDOW_MS;
  }
}
