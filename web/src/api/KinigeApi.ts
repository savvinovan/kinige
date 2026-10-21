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

/** Id Inbox: константа протокола, одна на все аккаунты (ADR-0001, §3.6). */
export const INBOX_ID = "00000000-0000-4000-8000-000000000001";

/** Ошибка команды: Rust возвращает код, текст — в файлах переводов (`error.<код>`). */
export class KinigeApiError extends Error {
  constructor(readonly code: ErrorCode) {
    super(code);
    this.name = "KinigeApiError";
  }
}

/** Код ошибки любого сбоя; всё, что пришло не от наших команд, — `internal`. */
export function errorCode(error: unknown): ErrorCode {
  return error instanceof KinigeApiError ? error.code : "internal";
}

/**
 * Единственная точка, через которую фронтенд говорит с бэкендом (§3.2 ТЗ).
 * Реализации: `TauriApi` (invoke), `MockApi` (тесты, Storybook).
 * Ошибки приходят как `KinigeApiError` с кодом.
 */
export interface KinigeApi {
  appInfo(): Promise<AppInfo>;
  /** Язык интерфейса по предпочитаемым языкам системы (решает Rust). */
  getLanguage(): Promise<Language>;
  listChats(): Promise<ChatSummary[]>;
  /** Страница ленты: без `before` — самая новая; записи по возрастанию, самая новая последней. */
  listMessages(chatId: string, options?: { before?: FeedCursor; limit?: number }): Promise<MessagePage>;
  /** Отправка: ответ — сохранённая запись; без оптимистичного показа. */
  sendMessage(chatId: string, body: string): Promise<MessageView>;
  /** «Отменить» отправку: запись уходит в корзину, ответ — её текст (для возврата в поле ввода). */
  undoSend(messageId: string): Promise<UndoResult>;
  getSettings(): Promise<Settings>;
  setTextSize(size: TextSize): Promise<void>;
  /** Системный диалог выбора папки Markdown-копии (вызывается из Rust). Отмена возвращает прежние настройки. */
  chooseMirrorFolder(): Promise<Settings>;
  /** Открыть ссылку из записи в системном браузере; Rust пропускает только `http`, `https`, `mailto`. */
  openExternalUrl(url: string): Promise<void>;
}
