import { KinigeApiError } from "../KinigeApi";
import { resolveColor } from "../mockColor";
import { looksLikeEmoji, normalizeIcon, normalizeTitle } from "../mockRules";
import type { ChatColor } from "../generated/ChatColor";
import type { ChatList } from "../generated/ChatList";
import type { ChatSummary } from "../generated/ChatSummary";
import type { MockState } from "./MockState";

/** Chats in `MockApi`: the sidebar list, creating and editing a chat, pinning and archiving, the sidebar state and the recent emoji. */
export class MockChats {
  private readonly openChatListeners = new Set<(chatId: string) => void>();

  constructor(private readonly s: MockState) {}

  listChats(): Promise<ChatList> {
    const rank = (chat: ChatSummary) => (chat.archivedAt !== null ? 2 : chat.pinnedAt !== null ? 0 : 1);
    const ordered = this.s.chats
      .map((chat, index) => ({ chat, index }))
      .sort(
        (a, b) =>
          rank(a.chat) - rank(b.chat) ||
          (rank(a.chat) === 0 ? (a.chat.pinnedAt ?? 0) - (b.chat.pinnedAt ?? 0) : 0) ||
          a.index - b.index,
      )
      .map(({ chat }) => ({ ...chat, count: this.s.rows.filter((r) => r.view.chatId === chat.id && r.deletedAt === null).length }));
    return Promise.resolve({ chats: ordered, trashCount: this.s.rows.filter((r) => r.deletedAt !== null).length });
  }

  createChat(title: string, icon: string | null): Promise<ChatSummary> {
    return this.s.run("createChat", `${title} ${icon}`, () => {
      const name = normalizeTitle(title);
      const chosen = normalizeIcon(icon);
      const id = `chat-${++this.s.counter}`;
      const chat: ChatSummary = {
        id,
        title: name,
        icon: chosen,
        isInbox: false,
        count: 0,
        pinnedAt: null,
        archivedAt: null,
        color: resolveColor(id, null),
      };
      this.s.chats.push(chat);
      return { ...chat };
    });
  }

  renameChat(id: string, title: string): Promise<ChatSummary> {
    return this.s.run("renameChat", `${id} ${title}`, () => {
      // As in Rust, the name is checked before the chat is looked up.
      const name = normalizeTitle(title);
      const chat = this.mutable(id, true);
      chat.title = name;
      return { ...chat };
    });
  }

  setChatIcon(id: string, icon: string): Promise<ChatSummary> {
    return this.s.run("setChatIcon", `${id} ${icon}`, () => {
      const chosen = normalizeIcon(icon);
      const chat = this.mutable(id, true);
      chat.icon = chosen;
      return { ...chat };
    });
  }

  setChatColor(id: string, color: ChatColor): Promise<ChatSummary> {
    return this.s.run("setChatColor", `${id} ${color}`, () => {
      // Inbox is always grey and cannot be recoloured (the same code as in Rust).
      const chat = this.mutable(id, true);
      chat.color = color;
      return { ...chat };
    });
  }

  setChatPinned(id: string, pinned: boolean): Promise<ChatSummary> {
    return this.s.run("setChatPinned", `${id} ${pinned}`, () => {
      const chat = this.mutable(id, false);
      chat.pinnedAt = pinned ? (chat.pinnedAt ?? this.s.now().getTime()) : null;
      return { ...chat };
    });
  }

  setChatArchived(id: string, archived: boolean): Promise<ChatSummary> {
    return this.s.run("setChatArchived", `${id} ${archived}`, () => {
      const chat = this.mutable(id, archived);
      chat.archivedAt = archived ? (chat.archivedAt ?? this.s.now().getTime()) : null;
      return { ...chat };
    });
  }

  setSidebarCollapsed(collapsed: boolean): Promise<void> {
    return this.s.run("setSidebarCollapsed", String(collapsed), () => {
      this.s.settings = { ...this.s.settings, sidebarCollapsed: collapsed };
    });
  }

  addRecentEmoji(emoji: string): Promise<string[]> {
    return this.s.run("addRecentEmoji", emoji, () => {
      if (!looksLikeEmoji(emoji)) throw new KinigeApiError("chat_icon_invalid");
      return this.s.addRecent(emoji);
    });
  }

  onOpenChat(handler: (chatId: string) => void): Promise<() => void> {
    this.openChatListeners.add(handler);
    return Promise.resolve(() => {
      this.openChatListeners.delete(handler);
    });
  }

  /** Like the tray menu "open this chat" event. */
  emitOpenChat(chatId: string): void {
    for (const handler of [...this.openChatListeners]) handler(chatId);
  }

  /** A chat that may be changed; Inbox only if `notInbox` is false. */
  private mutable(id: string, notInbox: boolean): ChatSummary {
    const chat = this.s.chat(id);
    if (notInbox && chat.isInbox) throw new KinigeApiError("inbox_protected");
    return chat;
  }
}
