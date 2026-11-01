import { INBOX_ID } from "./KinigeApi";
import { mockTags } from "./mockTags";
import type { ChatSummary } from "./generated/ChatSummary";
import type { Language } from "./generated/Language";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";

export interface MockApiOptions {
  /** «Сейчас» для новых записей и метки «сегодня»; по умолчанию — часы окна. */
  now?: () => Date;
  language?: Language;
  /** Записи, которые уже есть в Inbox (и, если у них другой `chatId`, в соответствующих чатах). */
  messages?: readonly MessageView[];
  /** Чаты кроме Inbox, в порядке раздела «Чаты». */
  chats?: readonly Partial<ChatSummary>[];
  settings?: Settings;
  /** Следующая папка, которую «выберет» системный диалог; `null` — пользователь отменил. */
  pickedFolder?: string | null;
  /** Состояние запуска; по умолчанию журнал открыт. */
  startup?: StartupState;
}

export const DEFAULT_SHORTCUTS: Record<ShortcutKind, string> = {
  capture: "Ctrl+Alt+KeyN",
  clipboard: "Alt+Shift+KeyV",
};

/** Настройки по умолчанию для тестов и историй (все поля `Settings`). */
export const DEFAULT_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [
    { kind: "capture", accelerator: DEFAULT_SHORTCUTS.capture, isDefault: true, state: "assigned" },
    { kind: "clipboard", accelerator: DEFAULT_SHORTCUTS.clipboard, isDefault: true, state: "assigned" },
  ],
  sidebarCollapsed: false,
  recentEmoji: [],
  backup: { folder: "/Users/me/Library/Application Support/me.savvinov.kinige/backups", lastAt: null, count: 0, error: null },
};

/** Окно дописывания (ТЗ §2.1): запись не старше десяти минут. */
export const APPEND_WINDOW_MS = 10 * 60 * 1000;
/** Сколько записей окно захвата показывает над полем. */
export const CONTEXT_LEN = 3;
export const DAY_MS = 24 * 3_600_000;
export const TRASH_DAYS = 30;

/** Запись для тестов и историй: все поля `MessageView` с разумными значениями по умолчанию. */
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
  /** Тексты до каждой правки (ревизии `edit`). */
  revisions: string[];
  /** Запись — ответ (слайс 5): id записи-адресата; цитата `view.replyTo` собирается при чтении (`MockApiBase.viewOf`). */
  replyToId?: string;
}

/** Полные годы между днём старой записи и днём ответа; 0, если меньше года или ответ раньше (как `reply::years_later` в Rust). */
export function yearsLater(old: string, reply: string): number {
  if (!isRealDate(old) || !isRealDate(reply)) return 0;
  const [oy, om, od] = old.split("-").map(Number) as [number, number, number];
  const [ry, rm, rd] = reply.split("-").map(Number) as [number, number, number];
  const years = ry - oy - (rm * 32 + rd < om * 32 + od ? 1 : 0);
  return Math.max(0, years);
}

/** `YYYY-MM-DD` — существующая дата (как в Rust: будущие разрешены). */
export function isRealDate(text: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const [year, month, day] = text.split("-").map(Number);
  if ((year ?? 0) < 1000) return false;
  const date = new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
  return date.getUTCFullYear() === year && date.getUTCMonth() === (month ?? 1) - 1 && date.getUTCDate() === day;
}

/** Порядок ленты `(journal_date, created_at, id)`. */
export function compareFeed(
  a: { journalDate: string; createdAt: number; id: string },
  b: { journalDate: string; createdAt: number; id: string },
): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}
