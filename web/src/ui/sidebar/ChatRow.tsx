import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { useMenuTrigger, type MenuPosition } from "../menu/ContextMenu";
import { ChatTile } from "./ChatTile";
import { displayName, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";

interface ChatRowProps {
  chat: SidebarChat;
  selected: boolean;
  language: Language;
  onSelect?: (chatId: string) => void;
  /** An entry is being dragged over the row (acceptance decision): highlight the target. The row itself is always a target (`data-chat-drop`). */
  dropOver?: boolean;
  /** Context menu: right click, Shift+F10 and the menu key. Without a handler there is no menu (D1). */
  onOpenMenu?: (chat: SidebarChat, position: MenuPosition) => void;
}

/** The chat row label for the screen reader and the tooltip: "Рецепты, 12 345 записей". */
export function chatLabel(chat: SidebarChat, name: string, counter: string, t: TFunction): string {
  return counter ? `${name}, ${t("count.entries", { count: chat.count ?? 0 })}` : name;
}

/** A chat row: emoji, name (truncated with an ellipsis), a quiet entry counter. The selected chat is highlighted and marked `aria-current`. */
export function ChatRow({ chat, selected, language, onSelect, dropOver = false, onOpenMenu }: ChatRowProps) {
  const { t } = useTranslation();
  const name = displayName(chat, t);
  const counter = formatCount(chat.count ?? 0, language);
  const trigger = useMenuTrigger((position) => onOpenMenu?.(chat, position));
  return (
    <li>
      <button
        type="button"
        className={`chat-row${selected ? " chat-row--selected" : ""}${dropOver ? " chat-row--drop" : ""}`}
        data-chat-drop={chat.id}
        aria-current={selected ? "true" : undefined}
        aria-haspopup={onOpenMenu ? "menu" : undefined}
        title={chatLabel(chat, name, counter, t)}
        onClick={onSelect ? () => onSelect(chat.id) : undefined}
        {...(onOpenMenu ? trigger : {})}
      >
        <ChatTile chat={chat} name={name} />
        <span className="chat-row__name">{name}</span>
        {counter ? (
          // The screen reader reads "Рецепты, записей: 12 тыс.": the comma and the label are visually hidden, the number is visible.
          <span className="chat-row__count">
            <span className="visually-hidden">{t("sidebar.entriesLabel")}</span> {counter}
          </span>
        ) : null}
      </button>
    </li>
  );
}
