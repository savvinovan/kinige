import type { TFunction } from "i18next";
import type { ChatColor } from "../../api/generated/ChatColor";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { MenuItem } from "../menu/ContextMenu";
import { hasEmoji } from "./chatLetters";

/** The color of the letter tile (D2, KINIG-59). Matches the `tile.*` roles of the palette and the names Rust stores in `meta.color`. */
export type ChatTint = ChatColor;

/** The colours in the "Colour" submenu, in the order of the rows. */
export const CHAT_COLORS: readonly ChatTint[] = ["blue", "teal", "green", "orange", "pink", "violet", "grey"];

export interface SidebarChat {
  id: string;
  /** The chat name is user data. For Inbox (`isInbox`) the stored name is empty and the UI shows the translated one. */
  name: string;
  /** Inbox: the empty name is replaced by the translation "Входящие" / "Inbox". It can be neither renamed nor archived. */
  isInbox?: boolean;
  /** The chat emoji is user data; drawn by the system emoji font. */
  emoji: string;
  /** The color of the letter tile, as Rust resolved it (`ChatSummary.color`: the picked colour or the automatic one; Inbox is grey). */
  tint: ChatTint;
  /** How many entries are in the chat, excluding the trash. 0 or unset: the counter is hidden. */
  count?: number;
  /** Pinned: the row is in the "Pinned" section. If the chat is also archived, the archive wins. */
  pinned?: boolean;
  /** Archived: the row is in the collapsed "Archive" section. The flag is ignored for Inbox. */
  archived?: boolean;
}

/** An action from the chat context menu. */
export type ChatAction = "rename" | "icon" | "pin" | "unpin" | "archive" | "unarchive";

const isSet = (time: number | null | undefined) => time !== null && time !== undefined;

/** A chat from the Rust response -> a sidebar row. A chat without an emoji is stored with "💬": the sidebar shows a letter tile for it. */
export function toSidebarChat(chat: ChatSummary): SidebarChat {
  return {
    id: chat.id,
    name: chat.title,
    isInbox: chat.isInbox,
    emoji: chat.icon ?? "💬",
    tint: chat.color,
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

/** Sections by flags: the archive wins over pinning; Inbox never goes to the archive. The order inside a section is as received. */
export function partitionChats(chats: readonly SidebarChat[]): ChatSections {
  const sections: ChatSections = { pinned: [], chats: [], archived: [] };
  for (const chat of chats) {
    if (chat.archived && !chat.isInbox) sections.archived.push(chat);
    else if (chat.pinned) sections.pinned.push(chat);
    else sections.chats.push(chat);
  }
  return sections;
}

/** The name for eyes and the screen reader: for Inbox the empty stored name is replaced by the translation. */
export function displayName(chat: SidebarChat, t: TFunction): string {
  return chat.isInbox && chat.name === "" ? t("chat.inbox") : chat.name;
}

/** Whether the chat has a colour to choose: the colour is the tile's, so a chat shown with an emoji has none; Inbox is fixed grey. */
export function canPickColor(chat: SidebarChat): boolean {
  return !chat.isInbox && !hasEmoji(chat.emoji);
}

/**
 * The chat menu items. Inbox has only "Pin" / "Unpin". An archived chat has no pinning: it lies in the "Archive"
 * anyway, and the item would show nothing. "Colour" (a submenu, KINIG-59) is only for a chat shown as a letter tile and only
 * when the caller can change it (`withColor`).
 */
export function chatMenuItems(chat: SidebarChat, t: TFunction, withColor = false): MenuItem[] {
  const items: MenuItem[] = [];
  if (!chat.isInbox) {
    items.push({ id: "rename", label: t("chatMenu.rename"), icon: "edit" }, { id: "icon", label: t("chatMenu.icon"), icon: "emoji" });
  }
  if (withColor && canPickColor(chat)) {
    items.push({ id: "color", label: t("chatMenu.color"), icon: "palette", submenu: true });
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
