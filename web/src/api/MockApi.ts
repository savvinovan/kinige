import { INBOX_ID, type KinigeApi, KinigeApiError } from "./KinigeApi";
import { mockTags } from "./mockTags";
import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";

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
  sidebarCollapsed: false,
  recentEmoji: [],
  backup: { folder: "/Users/me/Library/Application Support/me.savvinov.kinige/backups", lastAt: null, count: 0, error: null },
};

const ALLOWED_SCHEMES = ["http:", "https:", "mailto:"];

/** Окно дописывания (ТЗ §2.1): запись не старше десяти минут. */
const APPEND_WINDOW_MS = 10 * 60 * 1000;
/** Сколько записей окно захвата показывает над полем. */
const CONTEXT_LEN = 3;
const DAY_MS = 24 * 3_600_000;
const TRASH_DAYS = 30;

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
    ...part,
  };
}

interface Row {
  view: MessageView;
  deletedAt: number | null;
  /** Тексты до каждой правки (ревизии `edit`). */
  revisions: string[];
}

type MockMethod = keyof KinigeApi;

/**
 * `KinigeApi` в памяти: тот же интерфейс, что у `TauriApi`, для тестов и Storybook. Хранит чаты и записи, считает счётчики,
 * ведёт корзину, правки и ревизии, умеет «отказ» любой команды (`failNext`) и запоминает открытые ссылки (`openedUrls`). День
 * записи считается упрощённо (UTC, граница 04:00): настоящие правила — в Rust.
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
  /** Журнал вызовов команд слайса 3: `метод аргументы` — для проверок порядка и числа вызовов. */
  readonly calls: string[] = [];
  /** Восстановление из копии запрошено: имя копии. */
  restoredFrom: string | null = null;
  /** Перезапуск для восстановления запрошен. */
  restartRequested = false;
  pickedFolder: string | null;

  private rows: Row[];
  private chats: ChatSummary[];
  private readonly drafts = new Map<string, string>();
  private captureDraft = "";
  private readonly listeners = new Set<() => void>();
  private readonly openChatListeners = new Set<(chatId: string) => void>();
  private readonly corruptListeners = new Set<() => void>();
  private readonly failures = new Map<MockMethod, ErrorCode>();
  private settings: Settings;
  private startup: StartupState;
  private readonly now: () => Date;
  private readonly language: Language;
  private counter = 0;

  constructor(options: MockApiOptions = {}) {
    this.now = options.now ?? (() => new Date());
    this.language = options.language ?? "ru";
    this.rows = (options.messages ?? []).map((view) => ({ view, deletedAt: null, revisions: [] }));
    this.chats = [
      { id: INBOX_ID, title: "", icon: "📥", isInbox: true, count: 0, pinnedAt: null, archivedAt: null },
      ...(options.chats ?? []).map((chat, index) => ({
        id: chat.id ?? `chat-${index + 1}`,
        title: chat.title ?? `Чат ${index + 1}`,
        icon: chat.icon === undefined ? "💬" : chat.icon,
        isInbox: false,
        count: 0,
        pinnedAt: chat.pinnedAt ?? null,
        archivedAt: chat.archivedAt ?? null,
      })),
    ];
    this.settings = options.settings ?? DEFAULT_SETTINGS;
    this.startup = options.startup ?? { needsRecovery: false, backups: [], databaseTooNew: false, mirrorFolder: null };
    this.pickedFolder = options.pickedFolder === undefined ? "/Users/me/Notes/Kinige" : options.pickedFolder;
  }

  /** Следующий вызов этой команды завершится ошибкой (один раз). */
  failNext(method: MockMethod, code: ErrorCode): void {
    this.failures.set(method, code);
  }

  appInfo(): Promise<AppInfo> {
    return Promise.resolve({ title: "Kinige mock" });
  }

  getLanguage(): Promise<Language> {
    return Promise.resolve(this.language);
  }

  listChats(): Promise<ChatList> {
    const rank = (chat: ChatSummary) => (chat.archivedAt !== null ? 2 : chat.pinnedAt !== null ? 0 : 1);
    const ordered = this.chats
      .map((chat, index) => ({ chat, index }))
      .sort(
        (a, b) =>
          rank(a.chat) - rank(b.chat) ||
          (rank(a.chat) === 0 ? (a.chat.pinnedAt ?? 0) - (b.chat.pinnedAt ?? 0) : 0) ||
          a.index - b.index,
      )
      .map(({ chat }) => ({ ...chat, count: this.rows.filter((r) => r.view.chatId === chat.id && r.deletedAt === null).length }));
    return Promise.resolve({ chats: ordered, trashCount: this.rows.filter((r) => r.deletedAt !== null).length });
  }

  listMessages(chatId: string, options: { before?: FeedCursor; limit?: number; tag?: string } = {}): Promise<MessagePage> {
    const tag = options.tag?.toLowerCase();
    const all = this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null && (tag === undefined || r.view.tags.includes(tag)))
      .map((r) => r.view)
      .sort(compareFeed);
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
    if (!this.chats.some((c) => c.id === chatId)) return Promise.reject(new KinigeApiError("chat_not_found"));
    const now = this.now();
    const message = mockMessage({
      id: `mock-${++this.counter}`,
      chatId,
      bodyMd: text,
      journalDate: this.journalDate(now),
      createdAt: now.getTime(),
    });
    this.rows.push({ view: message, deletedAt: null, revisions: [] });
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
    this.rows.push({ view: message, deletedAt: null, revisions: [] });
  }

  // ---- слайс 3 ----

  createChat(title: string, icon: string | null): Promise<ChatSummary> {
    return this.run("createChat", `${title} ${icon}`, () => {
      const name = title.trim();
      if (name === "") throw new KinigeApiError("chat_title_empty");
      if ([...name].length > 128) throw new KinigeApiError("chat_title_too_long");
      const chat: ChatSummary = {
        id: `chat-${++this.counter}`,
        title: name,
        icon: icon?.trim() ? icon.trim() : "💬",
        isInbox: false,
        count: 0,
        pinnedAt: null,
        archivedAt: null,
      };
      this.chats.push(chat);
      return { ...chat };
    });
  }

  renameChat(id: string, title: string): Promise<ChatSummary> {
    return this.run("renameChat", `${id} ${title}`, () => {
      const chat = this.mutableChat(id, true);
      const name = title.trim();
      if (name === "") throw new KinigeApiError("chat_title_empty");
      if ([...name].length > 128) throw new KinigeApiError("chat_title_too_long");
      chat.title = name;
      return { ...chat };
    });
  }

  setChatIcon(id: string, icon: string): Promise<ChatSummary> {
    return this.run("setChatIcon", `${id} ${icon}`, () => {
      const chat = this.mutableChat(id, true);
      if (icon.trim() === "") throw new KinigeApiError("chat_icon_invalid");
      chat.icon = icon.trim();
      return { ...chat };
    });
  }

  setChatPinned(id: string, pinned: boolean): Promise<ChatSummary> {
    return this.run("setChatPinned", `${id} ${pinned}`, () => {
      const chat = this.mutableChat(id, false);
      chat.pinnedAt = pinned ? (chat.pinnedAt ?? this.now().getTime()) : null;
      return { ...chat };
    });
  }

  setChatArchived(id: string, archived: boolean): Promise<ChatSummary> {
    return this.run("setChatArchived", `${id} ${archived}`, () => {
      const chat = this.mutableChat(id, archived);
      chat.archivedAt = archived ? (chat.archivedAt ?? this.now().getTime()) : null;
      return { ...chat };
    });
  }

  deleteMessage(id: string): Promise<void> {
    return this.run("deleteMessage", id, () => {
      const row = this.row(id);
      row.deletedAt ??= this.now().getTime();
    });
  }

  restoreMessage(id: string): Promise<void> {
    return this.run("restoreMessage", id, () => {
      this.row(id).deletedAt = null;
    });
  }

  editMessage(id: string, body: string): Promise<MessageView> {
    return this.run("editMessage", `${id} ${body}`, () => {
      const row = this.row(id);
      this.writable(row);
      const text = body.replace(/\r\n?/g, "\n").trim();
      if (text === "") throw new KinigeApiError("empty_message");
      return this.setBody(row, text);
    });
  }

  toggleTaskItem(id: string, index: number, checked: boolean): Promise<MessageView> {
    return this.run("toggleTaskItem", `${id} ${index} ${checked}`, () => {
      const row = this.row(id);
      this.writable(row);
      const lines = row.view.bodyMd.split("\n");
      let seen = 0;
      for (let i = 0; i < lines.length; i++) {
        const match = /^(\s*(?:[-*+]|\d+[.)])\s+\[)([ xX])(\]\s+\S.*)$/.exec(lines[i] ?? "");
        if (!match) continue;
        if (seen++ !== index) continue;
        if ((match[2] !== " ") === checked) throw new KinigeApiError("task_item_stale");
        lines[i] = `${match[1]}${checked ? "x" : " "}${match[3]}`;
        return this.setBody(row, lines.join("\n"));
      }
      throw new KinigeApiError("task_item_stale");
    });
  }

  setMessageFlag(id: string, flag: MessageFlag, on: boolean): Promise<MessageView> {
    return this.run("setMessageFlag", `${id} ${flag} ${on}`, () => {
      const row = this.row(id);
      this.writable(row);
      const now = this.now().getTime();
      const view = { ...row.view };
      if (flag === "task") {
        view.isTask = on;
        if (!on) view.taskDoneAt = null;
      } else if (flag === "task_done") {
        if (!view.isTask) {
          if (on) throw new KinigeApiError("internal");
        } else view.taskDoneAt = on ? now : null;
      } else {
        view.favoriteAt = on ? now : null;
      }
      row.view = view;
      return view;
    });
  }

  setMessageDate(id: string, journalDate: string): Promise<MessageView> {
    return this.run("setMessageDate", `${id} ${journalDate}`, () => {
      const row = this.row(id);
      this.writable(row);
      if (!isRealDate(journalDate)) throw new KinigeApiError("invalid_date");
      row.view = { ...row.view, journalDate };
      return row.view;
    });
  }

  listTrash(): Promise<TrashItem[]> {
    const now = this.now().getTime();
    return this.run("listTrash", "", () =>
      this.rows
        .filter((r) => r.deletedAt !== null)
        .sort((a, b) => (b.deletedAt ?? 0) - (a.deletedAt ?? 0))
        .map((r) => {
          const chat = this.chats.find((c) => c.id === r.view.chatId);
          const deletedAt = r.deletedAt ?? 0;
          return {
            message: r.view,
            chatTitle: chat?.title ?? "",
            chatIcon: chat?.icon ?? null,
            chatIsInbox: chat?.isInbox ?? false,
            deletedAt,
            daysLeft: Math.max(0, Math.ceil((deletedAt + TRASH_DAYS * DAY_MS - now) / DAY_MS)),
          };
        }),
    );
  }

  purgeMessage(id: string): Promise<void> {
    return this.run("purgeMessage", id, () => {
      const row = this.row(id);
      if (row.deletedAt === null) throw new KinigeApiError("not_in_trash");
      this.rows = this.rows.filter((r) => r !== row);
    });
  }

  setSidebarCollapsed(collapsed: boolean): Promise<void> {
    return this.run("setSidebarCollapsed", String(collapsed), () => {
      this.settings = { ...this.settings, sidebarCollapsed: collapsed };
    });
  }

  addRecentEmoji(emoji: string): Promise<string[]> {
    return this.run("addRecentEmoji", emoji, () => {
      if (emoji.trim() === "") throw new KinigeApiError("chat_icon_invalid");
      const recent = [emoji, ...this.settings.recentEmoji.filter((known) => known !== emoji)].slice(0, 24);
      this.settings = { ...this.settings, recentEmoji: recent };
      return recent;
    });
  }

  onOpenChat(handler: (chatId: string) => void): Promise<() => void> {
    this.openChatListeners.add(handler);
    return Promise.resolve(() => {
      this.openChatListeners.delete(handler);
    });
  }

  /** Как событие меню значка «открыть этот чат». */
  emitOpenChat(chatId: string): void {
    for (const handler of [...this.openChatListeners]) handler(chatId);
  }

  startupState(): Promise<StartupState> {
    return Promise.resolve(this.startup);
  }

  restoreBackup(name: string): Promise<RestoreOutcome> {
    return this.run("restoreBackup", name, () => {
      if (!this.startup.backups.some((b) => b.name === name)) throw new KinigeApiError("backup_not_found");
      this.restoredFrom = name;
      const previousMirror = this.startup.mirrorFolder ? `${this.startup.mirrorFolder}-before-restore-20261021T061200Z` : null;
      this.startup = { needsRecovery: false, backups: [], databaseTooNew: false, mirrorFolder: null };
      return { previousMirror };
    });
  }

  onDatabaseCorrupt(handler: () => void): Promise<() => void> {
    this.corruptListeners.add(handler);
    return Promise.resolve(() => {
      this.corruptListeners.delete(handler);
    });
  }

  /** Как событие «проверка целостности нашла повреждение». */
  emitDatabaseCorrupt(): void {
    for (const handler of [...this.corruptListeners]) handler();
  }

  restartForRecovery(): Promise<void> {
    this.restartRequested = true;
    return Promise.resolve();
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
  private live(chatId: string): MessageView[] {
    return this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null)
      .map((r) => r.view)
      .sort(compareFeed);
  }

  private run<T>(method: MockMethod, args: string, action: () => T): Promise<T> {
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

  private row(id: string): Row {
    const row = this.rows.find((r) => r.view.id === id);
    if (!row) throw new KinigeApiError("message_not_found");
    return row;
  }

  private writable(row: Row): void {
    if (row.view.readOnly) throw new KinigeApiError("read_only");
  }

  private mutableChat(id: string, notInbox: boolean): ChatSummary {
    const chat = this.chats.find((c) => c.id === id);
    if (!chat) throw new KinigeApiError("chat_not_found");
    if (notInbox && chat.isInbox) throw new KinigeApiError("inbox_protected");
    return chat;
  }

  private setBody(row: Row, text: string): MessageView {
    if (text === row.view.bodyMd) return row.view;
    row.revisions.push(row.view.bodyMd);
    row.view = { ...row.view, bodyMd: text, tags: mockTags(text) };
    return row.view;
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

/** `YYYY-MM-DD` — существующая дата (как в Rust: будущие разрешены). */
function isRealDate(text: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const [year, month, day] = text.split("-").map(Number);
  if ((year ?? 0) < 1000) return false;
  const date = new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
  return date.getUTCFullYear() === year && date.getUTCMonth() === (month ?? 1) - 1 && date.getUTCDate() === day;
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
