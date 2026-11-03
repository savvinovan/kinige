import type { TFunction } from "i18next";
import { displayName, partitionChats, type SidebarChat } from "../sidebar/chats";

/** A chat in the "Move to chat" list (entry submenu). */
export interface MoveTarget {
  id: string;
  /** Name for the eyes and the screen reader (for Inbox, a translation). */
  name: string;
  emoji: string;
  group: "pinned" | "chats" | "archive";
  /** The chat the entry is already in: the item is visible but disabled. */
  current: boolean;
}

/** Pinned, regular and archived chats, in sidebar order. `currentId` is the open chat (the entry is in it). */
export function moveTargetsFrom(chats: readonly SidebarChat[], currentId: string | null, t: TFunction): MoveTarget[] {
  const sections = partitionChats(chats);
  const make = (list: readonly SidebarChat[], group: MoveTarget["group"]): MoveTarget[] =>
    list.map((chat) => ({ id: chat.id, name: displayName(chat, t), emoji: chat.emoji, group, current: chat.id === currentId }));
  return [...make(sections.pinned, "pinned"), ...make(sections.chats, "chats"), ...make(sections.archived, "archive")];
}
