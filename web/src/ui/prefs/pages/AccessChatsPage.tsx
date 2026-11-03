import { useTranslation } from "react-i18next";
import type { AgentChatChoice } from "../../../api/generated/AgentChatChoice";
import { AgentChats } from "../../agents/AgentChats";
import { ChangedByFile } from "../ChangedByFile";
import { SubscreenHeader } from "../SubscreenHeader";
import "../../agents/agents.css";
import "../../settings/settings.css";

interface AccessChatsPageProps {
  chats: readonly AgentChatChoice[];
  /** Access is off: the list is visible but unavailable. */
  disabled: boolean;
  onToggle: (id: string, on: boolean) => void;
  onBack: () => void;
  changedAt: number | null;
  now?: Date;
}

/** The "Видимые чаты" subscreen: the whole list, a "visible to the agent" checkbox on each chat. */
export function AccessChatsPage({ chats, disabled, onToggle, onBack, changedAt, now }: AccessChatsPageProps) {
  const { t } = useTranslation();
  return (
    <section className="prefs-page" aria-label={t("prefs.section.accessChats")}>
      <SubscreenHeader parent={t("prefs.section.access")} title={t("prefs.section.accessChats")} onBack={onBack} />
      <div className="prefs-block">
        {chats.every((chat) => !chat.visible) ? <p className="settings__description">{t("agent.chats.description")}</p> : null}
        <AgentChats chats={chats} disabled={disabled} onToggle={onToggle} />
        <ChangedByFile at={changedAt} now={now} />
      </div>
    </section>
  );
}
