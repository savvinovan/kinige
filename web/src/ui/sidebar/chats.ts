import type { TFunction } from "i18next";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { MenuItem } from "../menu/ContextMenu";

export interface SidebarChat {
  id: string;
  /** Название чата — данные пользователя. У Inbox (`isInbox`) хранимое имя пустое, интерфейс показывает переведённое. */
  name: string;
  /** Inbox: пустое имя заменяется переводом «Входящие» / «Inbox». Его нельзя ни переименовать, ни архивировать. */
  isInbox?: boolean;
  /** Эмодзи чата — данные пользователя; рисует системный эмодзи-шрифт. */
  emoji: string;
  /** Сколько записей в чате, кроме корзины. 0 или не задано — счётчик скрыт. */
  count?: number;
  /** Закреплён: строка в разделе «Закреплённые». Если чат ещё и в архиве, архив главнее. */
  pinned?: boolean;
  /** В архиве: строка в свёрнутом разделе «Архив». У Inbox флаг игнорируется. */
  archived?: boolean;
}

/** Действие из контекстного меню чата. */
export type ChatAction = "rename" | "icon" | "pin" | "unpin" | "archive" | "unarchive";

const isSet = (time: number | null | undefined) => time !== null && time !== undefined;

/** Чат из ответа Rust → строка сайдбара. Чат без эмодзи получает нейтральный значок. */
export function toSidebarChat(chat: ChatSummary): SidebarChat {
  return {
    id: chat.id,
    name: chat.title,
    isInbox: chat.isInbox,
    emoji: chat.icon ?? "💬",
    count: chat.count,
    pinned: isSet(chat.pinnedAt) && !isSet(chat.archivedAt),
    archived: isSet(chat.archivedAt),
  };
}

export interface ChatSections {
  pinned: SidebarChat[];
  chats: SidebarChat[];
  archived: SidebarChat[];
}

/** Разделы по флагам: архив главнее закрепления; Inbox в архив не попадает. Порядок внутри раздела — как пришёл. */
export function partitionChats(chats: readonly SidebarChat[]): ChatSections {
  const sections: ChatSections = { pinned: [], chats: [], archived: [] };
  for (const chat of chats) {
    if (chat.archived && !chat.isInbox) sections.archived.push(chat);
    else if (chat.pinned) sections.pinned.push(chat);
    else sections.chats.push(chat);
  }
  return sections;
}

/** Название для глаз и диктора: у Inbox пустое хранимое имя заменяется переводом. */
export function displayName(chat: SidebarChat, t: TFunction): string {
  return chat.isInbox && chat.name === "" ? t("chat.inbox") : chat.name;
}

/**
 * Пункты меню чата. У Inbox только «Закрепить» / «Открепить». У архивного чата закрепления нет: он всё равно лежит в
 * «Архиве», и пункт ничего бы не показал.
 */
export function chatMenuItems(chat: SidebarChat, t: TFunction): MenuItem[] {
  const items: MenuItem[] = [];
  if (!chat.isInbox) {
    items.push({ id: "rename", label: t("chatMenu.rename"), icon: "edit" }, { id: "icon", label: t("chatMenu.icon"), icon: "emoji" });
  }
  if (!chat.archived || chat.isInbox) {
    items.push(chat.pinned ? { id: "unpin", label: t("chatMenu.unpin"), icon: "unpin" } : { id: "pin", label: t("chatMenu.pin"), icon: "pin" });
  }
  if (!chat.isInbox) {
    items.push(
      chat.archived
        ? { id: "unarchive", label: t("chatMenu.unarchive"), icon: "unarchive", separatorBefore: true }
        : { id: "archive", label: t("chatMenu.archive"), icon: "archive", separatorBefore: true },
    );
  }
  return items;
}
