import { useTranslation } from "react-i18next";
import type { AgentChatChoice } from "../../../api/generated/AgentChatChoice";
import { AgentChats } from "../../agents/AgentChats";
import { ChangedByFile } from "../ChangedByFile";
import { SubscreenHeader } from "../SubscreenHeader";
import "../../agents/agents.css";
import "../../settings/settings.css";

interface AccessChatsPageProps {
  chats: readonly AgentChatChoice[];
  /** Доступ выключен: список виден, но недоступен. */
  disabled: boolean;
  onToggle: (id: string, on: boolean) => void;
  onBack: () => void;
  changedAt: number | null;
  now?: Date;
}

/** Подэкран «Видимые чаты»: список целиком, флажок «видно агенту» у каждого чата. */
export function AccessChatsPage({ chats, disabled, onToggle, onBack, changedAt, now }: AccessChatsPageProps) {
  const { t } = useTranslation();
  return (
    <section className="prefs-page" aria-label={t("prefs.section.accessChats")}>
      <SubscreenHeader parent={t("prefs.section.access")} title={t("prefs.section.accessChats")} onBack={onBack} />
      <div className="prefs-block">
        <p className="settings__description">{t("agent.chats.description")}</p>
        <AgentChats chats={chats} disabled={disabled} onToggle={onToggle} />
        <ChangedByFile at={changedAt} now={now} />
      </div>
    </section>
  );
}
