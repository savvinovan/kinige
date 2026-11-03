import { useTranslation } from "react-i18next";
import type { AgentChatChoice } from "../../api/generated/AgentChatChoice";
import { Checkbox } from "../controls/Checkbox";

interface AgentChatsProps {
  chats: readonly AgentChatChoice[];
  /** Access is off: the list is visible but unavailable. */
  disabled: boolean;
  onToggle: (id: string, on: boolean) => void;
}

/**
 * A list of chats with "visible to the agent" checkboxes: emoji, name (ellipsis), entry count. It is shown in full, with no scroll
 * of its own (the window has one scroll, the panel's). A chat with `ai_excluded` is inactive and labeled "исключён из AI": the agent never sees it.
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
