import { useTranslation } from "react-i18next";
import type { AgentChatChoice } from "../../api/generated/AgentChatChoice";
import { Checkbox } from "../controls/Checkbox";

interface AgentChatsProps {
  chats: readonly AgentChatChoice[];
  /** Доступ выключен: список виден, но недоступен. */
  disabled: boolean;
  onToggle: (id: string, on: boolean) => void;
}

/**
 * Список чатов с флажками «видно агенту»: эмодзи, имя (многоточие), число записей. Показывается целиком, своей прокрутки
 * нет (в окне одна прокрутка, у самой панели). Чат с `ai_excluded` неактивен и подписан «исключён из AI»: агент его не видит никогда.
 */
export function AgentChats({ chats, disabled, onToggle }: AgentChatsProps) {
  const { t } = useTranslation();
  return (
    <ul className="agent-chats" aria-label={t("agent.chats.title")}>
      {chats.map((chat) => {
        const name = chat.isInbox ? t("chat.inbox") : chat.title;
        const count = t("count.entries", { count: chat.count });
        return (
          <li key={chat.id} className="agent-chats__item">
            <Checkbox
              checked={chat.visible}
              disabled={disabled || chat.aiExcluded}
              onChange={(event) => onToggle(chat.id, event.target.checked)}
              description={chat.aiExcluded ? `${count} · ${t("agent.chats.excluded")}` : count}
            >
              {chat.icon ? `${chat.icon} ${name}` : name}
            </Checkbox>
          </li>
        );
      })}
    </ul>
  );
}
