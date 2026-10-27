import { INBOX_ID, type KinigeApi, KinigeApiError } from "./KinigeApi";
import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { TextSize } from "./generated/TextSize";

export interface MockApiOptions {
  /** «Сейчас» для новых записей и метки «сегодня»; по умолчанию — часы окна. */
  now?: () => Date;
  language?: Language;
  /** Записи, которые уже есть в Inbox. */
  messages?: readonly MessageView[];
  settings?: Settings;
  /** Следующая папка, которую «выберет» системный диалог; `null` — пользователь отменил. */
  pickedFolder?: string | null;
}

const DEFAULT_SHORTCUTS: Record<ShortcutKind, string> = {
  capture: "Ctrl+Alt+KeyN",
  clipboard: "Alt+Shift+KeyV",
};

const DEFAULT_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [
    { kind: "capture", accelerator: DEFAULT_SHORTCUTS.capture, isDefault: true, state: "assigned" },
    { kind: "clipboard", accelerator: DEFAULT_SHORTCUTS.clipboard, isDefault: true, state: "assigned" },
  ],
};

const ALLOWED_SCHEMES = ["http:", "https:", "mailto:"];

/** Окно дописывания (ТЗ §2.1): запись не старше десяти минут. */
const APPEND_WINDOW_MS = 10 * 60 * 1000;
/** Сколько записей окно захвата показывает над полем. */
const CONTEXT_LEN = 3;

/**
 * `KinigeApi` в памяти: тот же интерфейс, что у `TauriApi`, для тестов и Storybook. Хранит записи, считает счётчик,
 * умеет «отказ записи» (`failNextSend`) и запоминает открытые ссылки (`openedUrls`). День записи считается упрощённо
 * (UTC, граница 04:00): настоящие правила — в Rust.
 */
export class MockApi implements KinigeApi {
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
  pickedFolder: string | null;

  private messages: MessageView[];
  private readonly drafts = new Map<string, string>();
  private captureDraft = "";
  private readonly listeners = new Set<() => void>();
  private settings: Settings;
  private readonly now: () => Date;
  private readonly language: Language;
  private counter = 0;

  constructor(options: MockApiOptions = {}) {
    this.now = options.now ?? (() => new Date());
    this.language = options.language ?? "ru";
    this.messages = [...(options.messages ?? [])];
    this.settings = options.settings ?? DEFAULT_SETTINGS;
    this.pickedFolder = options.pickedFolder === undefined ? "/Users/me/Notes/Kinige" : options.pickedFolder;
  }

  appInfo(): Promise<AppInfo> {
    return Promise.resolve({ title: "Kinige mock" });
  }

  getLanguage(): Promise<Language> {
    return Promise.resolve(this.language);
  }

  listChats(): Promise<ChatSummary[]> {
    return Promise.resolve([{ id: INBOX_ID, title: "", icon: "📥", isInbox: true, count: this.messages.length }]);
  }

  listMessages(chatId: string, options: { before?: FeedCursor; limit?: number } = {}): Promise<MessagePage> {
    const all = this.messages.filter((m) => m.chatId === chatId).sort(compareFeed);
    const older = options.before ? all.filter((m) => compareFeed(m, options.before as FeedCursor) < 0) : all;
    const limit = options.limit ?? 200;
    const page = older.slice(-limit);
    const first = page[0];
    return Promise.resolve({
      messages: page,
      older: older.length > page.length && first ? { journalDate: first.journalDate, createdAt: first.createdAt, id: first.id } : null,
      today: this.journalDate(this.now()),
    });
  }

  sendMessage(chatId: string, body: string): Promise<MessageView> {
    if (this.failNextSend) {
      const code = this.failNextSend;
      this.failNextSend = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "") return Promise.reject(new KinigeApiError("empty_message"));
    if (chatId !== INBOX_ID) return Promise.reject(new KinigeApiError("chat_not_found"));
    const now = this.now();
    const message: MessageView = {
      id: `mock-${++this.counter}`,
      chatId,
      bodyMd: text,
      journalDate: this.journalDate(now),
      createdAt: now.getTime(),
      createdTz: "UTC",
    };
    this.messages.push(message);
    // Как в Rust: отправка очищает черновик этого чата.
    this.drafts.delete(chatId);
    return Promise.resolve(message);
  }

  getSettings(): Promise<Settings> {
    return Promise.resolve(this.settings);
  }

  setTextSize(size: TextSize): Promise<void> {
    this.settings = { ...this.settings, textSize: size };
    return Promise.resolve();
  }

  chooseMirrorFolder(): Promise<Settings> {
    if (this.failNextPick) {
      const code = this.failNextPick;
      this.failNextPick = null;
      return Promise.reject(new KinigeApiError(code));
    }
    if (this.pickedFolder) {
      this.settings = { ...this.settings, mirror: { folder: this.pickedFolder, isDefault: false, error: null, pendingDays: 0 } };
    }
    return Promise.resolve(this.settings);
  }

  openExternalUrl(url: string): Promise<void> {
    let scheme = "";
    try {
      scheme = new URL(url).protocol;
    } catch {
      // относительный или пустой адрес
    }
    if (!ALLOWED_SCHEMES.includes(scheme)) return Promise.reject(new KinigeApiError("link_not_allowed"));
    this.openedUrls.push(url);
    return Promise.resolve();
  }

  getDraft(chatId: string): Promise<string> {
    return Promise.resolve(this.drafts.get(chatId) ?? "");
  }

  saveDraft(chatId: string, body: string): Promise<void> {
    if (body === "") this.drafts.delete(chatId);
    else this.drafts.set(chatId, body);
    return Promise.resolve();
  }

  setShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings> {
    return this.changeShortcut(kind, accelerator);
  }

  resetShortcut(kind: ShortcutKind): Promise<Settings> {
    return this.changeShortcut(kind, DEFAULT_SHORTCUTS[kind]);
  }

  shortcutRecording(active: boolean): Promise<void> {
    this.recordingLog.push(active);
    return Promise.resolve();
  }

  onEntriesChanged(handler: () => void): Promise<() => void> {
    this.listeners.add(handler);
    return Promise.resolve(() => {
      this.listeners.delete(handler);
    });
  }

  /** Как событие Rust «записи изменились» (в тестах — запись, сделанная окном захвата). */
  emitEntriesChanged(): void {
    for (const handler of [...this.listeners]) handler();
  }

  /** Запись, появившаяся «снаружи» (окно захвата, буфер): попадает в ленту после события. */
  addExternalMessage(message: MessageView): void {
    this.messages.push(message);
  }

  captureState(): Promise<CaptureState> {
    const now = this.now();
    const today = this.journalDate(now);
    const inbox = this.messages.filter((m) => m.chatId === INBOX_ID).sort(compareFeed);
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

  private capture(body: string, append: boolean): Promise<CaptureOutcome> {
    if (this.failNextCapture) {
      const code = this.failNextCapture;
      this.failNextCapture = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "") return Promise.reject(new KinigeApiError("empty_message"));
    const now = this.now();
    const inbox = this.messages.filter((m) => m.chatId === INBOX_ID).sort(compareFeed);
    const last = inbox[inbox.length - 1];
    this.captureDraft = "";
    if (append && last && this.canAppend(last, now)) {
      const message: MessageView = { ...last, bodyMd: `${last.bodyMd}\n\n${text}` };
      this.messages = this.messages.map((m) => (m.id === last.id ? message : m));
      this.emitEntriesChanged();
      return Promise.resolve({ action: "appended", message });
    }
    const message: MessageView = {
      id: `mock-${++this.counter}`,
      chatId: INBOX_ID,
      bodyMd: text,
      journalDate: this.journalDate(now),
      createdAt: now.getTime(),
      createdTz: "UTC",
    };
    this.messages.push(message);
    this.emitEntriesChanged();
    return Promise.resolve({ action: "created", message });
  }

  /** Правило цели дописывания, упрощённо: сегодняшняя запись не старше десяти минут (остальное решает Rust). */
  private canAppend(message: MessageView, now: Date): boolean {
    return message.journalDate === this.journalDate(now) && now.getTime() - message.createdAt <= APPEND_WINDOW_MS;
  }

  private changeShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings> {
    if (this.failNextShortcut) {
      const code = this.failNextShortcut;
      this.failNextShortcut = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const other = this.settings.shortcuts.find((s) => s.kind !== kind);
    if (other?.accelerator === accelerator) return Promise.reject(new KinigeApiError("shortcut_duplicate"));
    this.settings = {
      ...this.settings,
      shortcuts: this.settings.shortcuts.map((s) =>
        s.kind === kind
          ? { kind, accelerator, isDefault: accelerator === DEFAULT_SHORTCUTS[kind], state: "assigned" as const }
          : s,
      ),
    };
    return Promise.resolve(this.settings);
  }

  /** День записи с границей 04:00 (упрощённо, UTC). */
  private journalDate(date: Date): string {
    return new Date(date.getTime() - 4 * 3_600_000).toISOString().slice(0, 10);
  }
}

/** Порядок ленты `(journal_date, created_at, id)`. */
function compareFeed(
  a: { journalDate: string; createdAt: number; id: string },
  b: { journalDate: string; createdAt: number; id: string },
): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}
