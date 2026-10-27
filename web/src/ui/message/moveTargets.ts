import type { TFunction } from "i18next";
import { displayName, partitionChats, type SidebarChat } from "../sidebar/chats";

/** Чат в списке «Переместить в чат» (подменю записи). */
export interface MoveTarget {
  id: string;
  /** Название для глаз и диктора (у Inbox — перевод). */
  name: string;
  emoji: string;
  group: "pinned" | "chats" | "archive";
  /** Чат, в котором запись уже лежит: пункт виден, но недоступен. */
  current: boolean;
}

/** Закреплённые, обычные и архивные чаты — в порядке сайдбара. `currentId` — открытый чат (запись лежит в нём). */
export function moveTargetsFrom(chats: readonly SidebarChat[], currentId: string | null, t: TFunction): MoveTarget[] {
  const sections = partitionChats(chats);
  const make = (list: readonly SidebarChat[], group: MoveTarget["group"]): MoveTarget[] =>
    list.map((chat) => ({ id: chat.id, name: displayName(chat, t), emoji: chat.emoji, group, current: chat.id === currentId }));
  return [...make(sections.pinned, "pinned"), ...make(sections.chats, "chats"), ...make(sections.archived, "archive")];
}
