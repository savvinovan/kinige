import type { TFunction } from "i18next";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { MenuItem } from "../menu/ContextMenu";

/** The color of the letter tile (D2). Matches the `tile.*` roles of the palette. */
export type ChatTint = "blue" | "teal" | "green" | "orange" | "pink" | "violet";

export interface SidebarChat {
  id: string;
  /** The chat name is user data. For Inbox (`isInbox`) the stored name is empty and the UI shows the translated one. */
  name: string;
  /** Inbox: the empty name is replaced by the translation "Входящие" / "Inbox". It can be neither renamed nor archived. */
  isInbox?: boolean;
  /** The chat emoji is user data; drawn by the system emoji font. */
  emoji: string;
  /**
   * The color of the letter tile. The `chat.color` column in the database is always empty for now and is not returned in `ChatSummary` (D2 does not change Rust),
   * so `toSidebarChat` does not set it and the color is derived from the id (`tintOf`); the stored color arrives with KINIG-59.
   */
  tint?: ChatTint;
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

/**
 * The chat menu items. Inbox has only "Pin" / "Unpin". An archived chat has no pinning: it lies in the "Archive"
 * anyway, and the item would show nothing.
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
