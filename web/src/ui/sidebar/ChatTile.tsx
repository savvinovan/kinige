import type { ReactNode } from "react";
import { firstLetter, hasEmoji, tintOf } from "./chatLetters";
import type { SidebarChat } from "./chats";

interface ChatTileProps {
  chat: Pick<SidebarChat, "id" | "emoji" | "tint">;
  /** Название, которое видит человек (у Inbox переведённое): из него берётся буква плитки. */
  name: string;
}

/**
 * Значок чата. С эмодзи — сам эмодзи, без фона, в слоте размера плитки (названия стоят ровно). Без эмодзи (пусто или «💬» по
 * умолчанию) — скруглённая плитка цвета чата с первой буквой названия. Декор: название читается рядом.
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
    <span className={`chat-tile chat-tile--${tintOf(chat)}`} aria-hidden="true">
      {firstLetter(name)}
    </span>
  );
}

/** Слот значка у нижних пунктов («Корзина», пункты меню профиля): того же размера, что плитка, без заливки; значок по центру. */
export function NavSlot({ children }: { children: ReactNode }) {
  return (
    <span className="nav-slot" aria-hidden="true">
      {children}
    </span>
  );
}
