import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ContextMenu, type MenuItem, type MenuPosition } from "../menu/ContextMenu";
import { ChatTile } from "./ChatTile";
import { CHAT_COLORS, type ChatAction, type ChatTint, chatMenuItems, displayName, type SidebarChat } from "./chats";

interface ChatMenuProps {
  chat: SidebarChat;
  position: MenuPosition;
  onAction: (action: ChatAction) => void;
  /** Choosing a colour; without it there is no "Colour" item. */
  onColor?: (color: ChatTint) => void;
  onClose: () => void;
}

const COLOR_PREFIX = "color:";

/**
 * The sidebar chat's context menu: Rename, Change emoji, Colour (a submenu), Pin, Archive. "Colour" is a second level in the
 * same panel, like "Move to chat" in the entry menu: "Back", then seven rows, each the chat's own letter tile in that colour and the
 * colour's name; the current one has a check. Esc and the left arrow return to the first level.
 */
export function ChatMenu({ chat, position, onAction, onColor, onClose }: ChatMenuProps) {
  const { t } = useTranslation();
  const [level, setLevel] = useState<"main" | "color">("main");
  const name = displayName(chat, t);

  const colors: MenuItem[] = [
    { id: "back", label: t("chatMenu.colorBack"), icon: "chevronLeft" },
    ...CHAT_COLORS.map((color, index) => ({
      id: `${COLOR_PREFIX}${color}`,
      label: t(`chatColor.${color}`),
      // The row previews the chat itself: its first letter on the tile of that colour (never the emoji).
      tile: <ChatTile chat={{ emoji: "", tint: color }} name={name} />,
      checked: chat.tint === color,
      separatorBefore: index === 0,
    })),
  ];

  return (
    <ContextMenu
      items={level === "main" ? chatMenuItems(chat, t, onColor !== undefined) : colors}
      position={position}
      label={level === "main" ? t("chatMenu.label", { name }) : t("chatMenu.colorMenu")}
      levelKey={level}
      onBack={level === "color" ? () => setLevel("main") : undefined}
      onClose={onClose}
      onSelect={(id) => {
        if (id === "color") return setLevel("color");
        if (id === "back") return setLevel("main");
        onClose();
        if (id.startsWith(COLOR_PREFIX)) onColor?.(id.slice(COLOR_PREFIX.length) as ChatTint);
        else onAction(id as ChatAction);
      }}
    />
  );
}
