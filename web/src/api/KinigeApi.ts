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
  getSettings(): Promise<Settings>;
  setTextSize(size: TextSize): Promise<void>;
  /** Системный диалог выбора папки Markdown-копии (вызывается из Rust). Отмена возвращает прежние настройки. */
  chooseMirrorFolder(): Promise<Settings>;
  /** Открыть ссылку из записи в системном браузере; Rust пропускает только `http`, `https`, `mailto`. */
  openExternalUrl(url: string): Promise<void>;

  // ---- главное окно (слайс 2) ----
  /** Черновик поля ввода чата (§5): пустая строка, если его нет. */
  getDraft(chatId: string): Promise<string>;
  /** Сохраняет черновик; пустой текст удаляет его. Отправка очищает черновик в Rust, в той же транзакции. */
  saveDraft(chatId: string, body: string): Promise<void>;
  /** Смена своего сочетания: Rust проверяет запись, регистрирует в системе и сохраняет. Ответ — настройки целиком. */
  setShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings>;
  /** «Сбросить»: сочетание по умолчанию, те же проверки. */
  resetShortcut(kind: ShortcutKind): Promise<Settings>;
  /** Идёт (`true`) или закончилась запись нового сочетания: на это время хоткеи не открывают окно захвата. */
  shortcutRecording(active: boolean): Promise<void>;
  /** Подписка на событие «записи изменились» (захват, дописывание, буфер). Возвращает отписку. */
  onEntriesChanged(handler: () => void): Promise<() => void>;

  // ---- окно захвата (слайс 2): отдельный набор команд, только Inbox ----
  /** Всё, что нужно окну при показе: язык, размер текста, контекст за сегодня, цель дописывания, черновик, ошибка. */
  captureState(): Promise<CaptureState>;
  /** Enter: новая запись в Inbox. */
  captureSend(body: string): Promise<CaptureOutcome>;
  /** ⌘Enter: дописать к последней записи (если Rust решил, что можно) или новая запись; ответ сообщает, что сделано. */
  captureAppend(body: string): Promise<CaptureOutcome>;
  captureSaveDraft(body: string): Promise<void>;
  /** Спрятать окно (после отправки, Esc, потери фокуса). */
  captureHide(): Promise<void>;
  /**
   * Подогнать высоту окна под содержимое (логические px, вместе с прозрачным полем под тень). Rust ограничивает значение
   * диапазоном `layout.capture.window-min-height … window-max-height`, ширина и верх окна не меняются.
   */
  captureResize(height: number): Promise<void>;
}
