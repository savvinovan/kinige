import type { ReactNode } from "react";
import type { SidebarChat } from "./chats";

/** Плитка чата: эмодзи в мягком скруглённом квадрате, подкрашенном цветом чата (без цвета — нейтральная). Декор: имя читается рядом. */
export function ChatTile({ chat }: { chat: Pick<SidebarChat, "emoji" | "tint"> }) {
  return (
    <span className={`chat-tile${chat.tint ? ` chat-tile--${chat.tint}` : ""}`} aria-hidden="true">
      <span className="chat-tile__glyph">{chat.emoji}</span>
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
