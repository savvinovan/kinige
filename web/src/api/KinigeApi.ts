import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { ImportOpened } from "./generated/ImportOpened";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportReport } from "./generated/ImportReport";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { SearchFilters } from "./generated/SearchFilters";
import type { SearchPage } from "./generated/SearchPage";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";

/** Позиция и размер страницы ленты (`KinigeApi.listMessages`). */
export interface ListMessagesOptions {
  before?: FeedCursor;
  after?: FeedCursor;
  /** Id записи, вокруг которой нужна страница. */
  around?: string;
  limit?: number;
  tag?: string;
}

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
  /** Чаты в порядке показа (закреплённые, чаты, архив) и размер корзины. */
  listChats(): Promise<ChatList>;
  /**
   * Страница ленты: без позиции — самая новая; записи по возрастанию, самая новая последней. `before` — более старая
   * страница после курсора, `after` — более новая (слайс 4), `around` — страница вокруг записи (переход из поиска); если
   * указаны несколько позиций, `around` главнее `after`, а `after` — `before`. `tag` (без `#`, любой регистр) оставляет
   * только записи с этим тегом.
   */
  listMessages(chatId: string, options?: ListMessagesOptions): Promise<MessagePage>;
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

  // ---- главное окно (слайс 3): чаты, действия с записью, корзина ----
  /** Новый чат в конце раздела «Чаты». Имя 1–128 знаков; без эмодзи — 💬. */
  createChat(title: string, icon: string | null): Promise<ChatSummary>;
  renameChat(id: string, title: string): Promise<ChatSummary>;
  setChatIcon(id: string, icon: string): Promise<ChatSummary>;
  setChatPinned(id: string, pinned: boolean): Promise<ChatSummary>;
  /** Inbox архивировать нельзя (`inbox_protected`). */
  setChatArchived(id: string, archived: boolean): Promise<ChatSummary>;
  /** «Удалить»: запись уходит в корзину. Без оптимистичного показа: ответ приходит после фиксации. */
  deleteMessage(id: string): Promise<void>;
  /** «Вернуть» из корзины: запись возвращается на своё место. */
  restoreMessage(id: string): Promise<void>;
  /** Правка текста; старая версия сохраняется ревизией. Ответ — запись, как она теперь выглядит. */
  editMessage(id: string, body: string): Promise<MessageView>;
  /** Щелчок по `- [ ]` в тексте: пункт `index` (с нуля, по порядку в тексте) получает состояние `checked`. */
  toggleTaskItem(id: string, index: number, checked: boolean): Promise<MessageView>;
  /** Задача, выполнено (только у задачи), избранное. */
  setMessageFlag(id: string, flag: MessageFlag, on: boolean): Promise<MessageView>;
  /** «Изменить дату»: `YYYY-MM-DD`, будущие даты разрешены. */
  setMessageDate(id: string, journalDate: string): Promise<MessageView>;
  /** «Переместить в чат» (меню записи, перетаскивание на чат): день и текст не меняются, в корзину нельзя. */
  moveMessage(id: string, chatId: string): Promise<MessageView>;
  /** Корзина: недавно удалённые первыми. */
  listTrash(): Promise<TrashItem[]>;
  /** «Удалить навсегда» из корзины (интерфейс просит подтверждения). */
  purgeMessage(id: string): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
  /** Выбранный в пикере эмодзи — первым в «недавних»; ответ — список целиком. */
  addRecentEmoji(emoji: string): Promise<string[]>;
  /** Меню значка: «открыть этот чат». Возвращает отписку. */
  onOpenChat(handler: (chatId: string) => void): Promise<() => void>;

  // ---- запуск и копии базы (слайс 3) ----
  /** Журнал или окно «База повреждена» со списком копий. */
  startupState(): Promise<StartupState>;
  /**
   * Восстановить из копии `name` (только когда журнал не открыт; повторный вызов во время восстановления — ошибка).
   * Ответ — куда переехала прежняя папка Markdown-копии.
   */
  restoreBackup(name: string): Promise<RestoreOutcome>;
  /** Проверка целостности нашла повреждение работающей базы. Возвращает отписку. */
  onDatabaseCorrupt(handler: () => void): Promise<() => void>;
  /** Перезапустить приложение для восстановления (после события `onDatabaseCorrupt`). */
  restartForRecovery(): Promise<void>;

  // ---- импорт из Telegram и поиск (слайс 4) ----
  /**
   * «Выбрать result.json…»: системный диалог (вызывается из Rust, путь во фронтенд не приходит), чтение и разбор файла.
   * Отмена диалога — `null`. Не тот файл — `import_not_telegram`, не прочитался — `import_file_unreadable`, больше 1 ГБ —
   * `import_file_too_large`; ничего не записано.
   */
  importTelegramOpen(): Promise<ImportOpened | null>;
  /**
   * «Импортировать»: выбранные чаты (id Telegram) разобранного файла. Ответ приходит в конце; прогресс — событие
   * `onImportProgress`. Чужая или забытая сессия — `import_session_expired`, второй импорт — `import_in_progress`.
   */
  importTelegramRun(session: number, chatIds: readonly string[]): Promise<ImportReport>;
  /** «Остановить»: идущий импорт дописывает текущую пачку и останавливается. */
  importTelegramStop(): Promise<void>;
  /** Окно импорта закрыто: разобранный файл забывается. */
  importTelegramClose(): Promise<void>;
  /** Прогресс идущего импорта после каждой пачки. Возвращает отписку. */
  onImportProgress(handler: (progress: ImportProgress) => void): Promise<() => void>;
  /**
   * Поиск по записям: ввод как есть (словоформы и префикс решает Rust). Новые записи сверху; `before` — следующая страница;
   * чаты с совпавшим названием приходят на первой. Пустой запрос — пустая страница.
   */
  searchMessages(query: string, options?: { filters?: SearchFilters; before?: FeedCursor; limit?: number }): Promise<SearchPage>;

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
  /** Эмодзи, выбранный в пикере окна захвата, встаёт первым в общем списке «недавних»; ответ — список целиком. */
  captureAddRecentEmoji(emoji: string): Promise<string[]>;
}
