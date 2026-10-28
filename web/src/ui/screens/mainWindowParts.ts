import type { ChatList } from "../../api/generated/ChatList";
import type { SidebarChat } from "../sidebar/Sidebar";

/** Состояние окна «Новый чат» / «Переименовать» / «Эмодзи чата» главного окна. */
export type ChatDialogState = { mode: "create" } | { mode: "rename" | "icon"; chatId: string };

export const EMPTY_LIST: ChatList = { chats: [], trashCount: 0 };

/** Чат из ответа Rust → строка сайдбара. Чат без эмодзи получает нейтральный значок. */
export function toSidebar(chat: ChatList["chats"][number]): SidebarChat {
  return {
    id: chat.id,
    name: chat.title,
    isInbox: chat.isInbox,
    emoji: chat.icon ?? "💬",
    count: chat.count,
    pinned: chat.pinnedAt !== null && chat.archivedAt === null,
    archived: chat.archivedAt !== null,
  };
}

/** Фокус в поле ввода записи. Диалог возвращает фокус тому, кто его открыл, уже после закрытия: поле берёт его следом. */
export function focusComposer() {
  setTimeout(() => document.querySelector<HTMLTextAreaElement>("textarea.composer__input")?.focus(), 0);
}
