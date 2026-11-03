import { useEffect } from "react";
import type { ChatList } from "../../api/generated/ChatList";
import type { SidebarChat } from "../sidebar/Sidebar";

/** State of the main window's "New chat" / "Rename" / "Chat emoji" dialog. */
export type ChatDialogState = { mode: "create" } | { mode: "rename" | "icon"; chatId: string };

export const EMPTY_LIST: ChatList = { chats: [], trashCount: 0 };

/** A chat from a Rust reply -> a sidebar row. A chat without an emoji gets a neutral icon. */
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

/** Focus on the entry composer. A dialog returns focus to whoever opened it only after closing, so the composer takes it afterwards. */
export function focusComposer() {
  setTimeout(() => document.querySelector<HTMLTextAreaElement>("textarea.composer__input")?.focus(), 0);
}

/** Runs `check` whenever the window gets focus (has a new journal day begun: the "Today" and "Yesterday" labels, the card). */
export function useCheckOnWindowFocus(check: () => Promise<unknown>) {
  useEffect(() => {
    const onFocus = () => void check();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [check]);
}
