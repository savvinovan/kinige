import { INBOX_ID, type KinigeApi, KinigeApiError } from "./KinigeApi";
import type { AppInfo } from "./generated/AppInfo";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { TextSize } from "./generated/TextSize";
import type { UndoResult } from "./generated/UndoResult";

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

const DEFAULT_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
};

const ALLOWED_SCHEMES = ["http:", "https:", "mailto:"];

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
  pickedFolder: string | null;

  private messages: MessageView[];
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
    return Promise.resolve(message);
  }

  undoSend(messageId: string): Promise<UndoResult> {
    const found = this.messages.find((m) => m.id === messageId);
    if (!found) return Promise.reject(new KinigeApiError("message_not_found"));
    this.messages = this.messages.filter((m) => m.id !== messageId);
    return Promise.resolve({ bodyMd: found.bodyMd });
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
