import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { AgentAccess } from "../../../api/generated/AgentAccess";
import type { AgentMode } from "../../../api/generated/AgentMode";
import { AgentConnect } from "../../agents/AgentConnect";
import { Checkbox } from "../../controls/Checkbox";
import { SegmentedChoice } from "../../settings/SegmentedChoice";
import { StatusNote } from "../../settings/StatusNote";
import { ChangedByFile } from "../ChangedByFile";
import { ConfigureRow } from "../ConfigureRow";
import type { Route } from "../routes";
import "../../agents/agents.css";
import "../../settings/settings.css";

interface AccessPageProps {
  access: AgentAccess;
  /** Уже переведённый текст ошибки последнего действия (`role="alert"`). */
  error?: string;
  /** Флажок доступа. Включение родитель сначала подтверждает предупреждением. */
  onEnabledChange: (enabled: boolean) => void;
  onModeChange: (mode: AgentMode) => void;
  onOpen: (route: Route) => void;
  changedAt: number | null;
  now?: Date;
}

/**
 * «Доступ ИИ-агентов»: флажок доступа, режим, «Как подключить» и две строки «Настроить ›» — «Видимые чаты» и «Журнал
 * обращений» (длинные списки живут на подэкранах). Пока доступ выключен, режим и чаты недоступны, журнал виден.
 */
export function AccessPage({ access, error, onEnabledChange, onModeChange, onOpen, changedAt, now }: AccessPageProps) {
  const { t } = useTranslation();
  const titleId = useId();
  const modeId = useId();
  const connectId = useId();
  const off = !access.enabled;
  const visible = access.chats.filter((chat) => chat.visible).length;
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.access")}
      </h2>
      <div className="prefs-block">
        <Checkbox
          checked={access.enabled}
          onChange={(event) => onEnabledChange(event.target.checked)}
          description={t("agent.enable.description")}
        >
          {t("agent.enable.label")}
        </Checkbox>
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {error}
          </StatusNote>
        ) : null}
        <ChangedByFile at={changedAt} now={now} />
      </div>
      <div className="prefs-block" aria-labelledby={modeId} role="group">
        <h3 className="settings__heading" id={modeId}>
          {t("agent.mode.title")}
        </h3>
        <SegmentedChoice<AgentMode>
          labelledBy={modeId}
          value={access.mode}
          disabled={off}
          onChange={onModeChange}
          options={[
            { value: "read", label: t("agent.mode.read") },
            { value: "read_write", label: t("agent.mode.readWrite") },
          ]}
        />
        <p className="settings__description">{t("agent.mode.hint")}</p>
      </div>
      <div className="prefs-block">
        <p className="settings__description">{t("agent.chats.description")}</p>
        <ConfigureRow
          label={t("prefs.access.chatsRow")}
          value={t("prefs.access.chatsValue", { visible, total: access.chats.length })}
          onOpen={() => onOpen("access/chats")}
        />
        <ConfigureRow label={t("prefs.access.logRow")} value={t("prefs.access.logValue")} onOpen={() => onOpen("access/log")} />
      </div>
      <div className="prefs-block" role="group" aria-labelledby={connectId}>
        <h3 className="settings__heading" id={connectId}>
          {t("agent.connect.title")}
        </h3>
        <AgentConnect socket={access.socket} />
      </div>
    </section>
  );
}
