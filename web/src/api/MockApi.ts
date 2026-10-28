import { type KinigeApi, KinigeApiError } from "./KinigeApi";
import { MockApiSlice4a } from "./MockApiSlice4a";
import { DAY_MS, DEFAULT_SHORTCUTS, TRASH_DAYS, isRealDate, mockMessage } from "./mockData";
import type { AppInfo } from "./generated/AppInfo";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessageView } from "./generated/MessageView";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";

export { DEFAULT_SETTINGS, type MockApiOptions, mockMessage } from "./mockData";

const ALLOWED_SCHEMES = ["http:", "https:", "mailto:"];

/**
 * `KinigeApi` в памяти: тот же интерфейс, что у `TauriApi`, для тестов и Storybook. Хранит чаты и записи, считает счётчики,
 * ведёт корзину, правки и ревизии, умеет «отказ» любой команды (`failNext`) и запоминает открытые ссылки (`openedUrls`). День
 * записи считается упрощённо (UTC, граница 04:00): настоящие правила — в Rust. Состояние и окно захвата — в `MockApiBase`.
 */
export class MockApi extends MockApiSlice4a implements KinigeApi {
  private readonly openChatListeners = new Set<(chatId: string) => void>();
  private readonly corruptListeners = new Set<() => void>();

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
    return this.run("getSettings", "", () => this.settings);
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

  moveMessage(id: string, chatId: string): Promise<MessageView> {
    return this.run("moveMessage", `${id} ${chatId}`, () => {
      const row = this.row(id);
      this.writable(row);
      if (row.deletedAt !== null) throw new KinigeApiError("message_in_trash");
      if (!this.chats.some((chat) => chat.id === chatId)) throw new KinigeApiError("chat_not_found");
      row.view = { ...row.view, chatId };
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
      const folder = this.settings.mirror.folder;
      const previousMirror = folder ? `${folder}-before-restore-20261021T061200Z` : null;
      this.startup = { needsRecovery: false, backups: [], databaseTooNew: false, corruptionFound: false };
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
}
