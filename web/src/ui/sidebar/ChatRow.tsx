import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { useMenuTrigger, type MenuPosition } from "../menu/ContextMenu";
import { displayName, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";

interface ChatRowProps {
  chat: SidebarChat;
  selected: boolean;
  language: Language;
  onSelect?: (chatId: string) => void;
  /** Контекстное меню: правый щелчок, Shift+F10 и клавиша меню. Без обработчика меню нет (D1). */
  onOpenMenu?: (chat: SidebarChat, position: MenuPosition) => void;
}

/** Подпись строки чата для диктора и подсказки: «Рецепты, 12 345 записей». */
export function chatLabel(chat: SidebarChat, name: string, counter: string, t: TFunction): string {
  return counter ? `${name}, ${t("count.entries", { count: chat.count ?? 0 })}` : name;
}

/** Строка чата: эмодзи, имя (обрезается многоточием), тихий счётчик записей. Выбранный чат подсвечен и помечен `aria-current`. */
export function ChatRow({ chat, selected, language, onSelect, onOpenMenu }: ChatRowProps) {
  const { t } = useTranslation();
  const name = displayName(chat, t);
  const counter = formatCount(chat.count ?? 0, language);
  const trigger = useMenuTrigger((position) => onOpenMenu?.(chat, position));
  return (
    <li>
      <button
        type="button"
        className={`chat-row${selected ? " chat-row--selected" : ""}`}
        aria-current={selected ? "true" : undefined}
        aria-haspopup={onOpenMenu ? "menu" : undefined}
        title={chatLabel(chat, name, counter, t)}
        onClick={onSelect ? () => onSelect(chat.id) : undefined}
        {...(onOpenMenu ? trigger : {})}
      >
        <span className="chat-row__emoji" aria-hidden="true">
          {chat.emoji}
        </span>
        <span className="chat-row__name">{name}</span>
        {counter ? (
          // Диктору читается «Рецепты, записей: 12 тыс.»: запятая и подпись скрыты визуально, число видно.
          <span className="chat-row__count">
            <span className="visually-hidden">{t("sidebar.entriesLabel")}</span> {counter}
          </span>
        ) : null}
      </button>
    </li>
  );
}
