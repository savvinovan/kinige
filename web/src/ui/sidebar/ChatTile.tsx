import type { ReactNode } from "react";
import { firstLetter, hasEmoji } from "./chatLetters";
import type { SidebarChat } from "./chats";
import "./sidebar.css";

interface ChatTileProps {
  chat: Pick<SidebarChat, "emoji" | "tint">;
  /** The name the person sees (translated for Inbox): the tile letter is taken from it. */
  name: string;
}

/**
 * The chat icon. With an emoji: the emoji itself, without a background, in a slot of the tile size (the names line up).
 * Without an emoji (empty or the default "💬"): a rounded tile in the chat color with the first letter of the name.
 * Decoration: the name is read next to it.
 */
export function ChatTile({ chat, name }: ChatTileProps) {
  if (hasEmoji(chat.emoji)) {
    return (
      <span className="chat-emoji" aria-hidden="true">
        <span className="chat-emoji__glyph">{chat.emoji}</span>
      </span>
    );
  }
  return (
    <span className={`chat-tile chat-tile--${chat.tint}`} aria-hidden="true">
      {firstLetter(name)}
    </span>
  );
}

/** The icon slot of the bottom items ("Trash", profile menu items): the same size as the tile, no fill; the icon is centered. */
export function NavSlot({ children }: { children: ReactNode }) {
  return (
    <span className="nav-slot" aria-hidden="true">
      {children}
    </span>
  );
}
