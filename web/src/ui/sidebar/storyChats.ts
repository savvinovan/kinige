import { resolveColor } from "../../api/mockColor";
import type { ChatTint, SidebarChat } from "./chats";

/** A chat as stories and tests write it: the colour is optional. */
export type ChatWithoutTint = Omit<SidebarChat, "tint"> & { tint?: ChatTint };

/**
 * For stories and tests only: chats without a colour get the automatic one that Rust would compute for their id (the `MockApi`
 * emulation), Inbox is grey. The app never calls this: the colour comes from `ChatSummary.color`.
 */
export function withTints(chats: readonly ChatWithoutTint[]): SidebarChat[] {
  return chats.map((chat) => ({ ...chat, tint: chat.tint ?? (chat.isInbox ? "grey" : resolveColor(chat.id, null)) }));
}
