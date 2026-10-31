import { useTranslation } from "react-i18next";
import type { AgentLogEntry } from "../../../api/generated/AgentLogEntry";
import type { Language } from "../../../api/generated/Language";
import { AgentLog, type AgentLogView } from "../../agents/AgentLog";
import { SubscreenHeader } from "../SubscreenHeader";
import "../../agents/agents.css";
import "../../settings/settings.css";

interface AccessLogPageProps {
  log: AgentLogView;
  language: Language;
  now?: Date;
  onToggleRow: (id: number) => void;
  onOpenEntry: (logId: number, entry: AgentLogEntry) => void;
  onMore: () => void;
  onBack: () => void;
}

/** Подэкран «Журнал обращений»: строки журнала, «Показать ещё» внизу страницы; прокрутка — одна, у страницы. */
export function AccessLogPage({ log, language, now, onToggleRow, onOpenEntry, onMore, onBack }: AccessLogPageProps) {
  const { t } = useTranslation();
  return (
    <section className="prefs-page" aria-label={t("prefs.section.accessLog")}>
      <SubscreenHeader parent={t("prefs.section.access")} title={t("prefs.section.accessLog")} onBack={onBack} />
      <div className="prefs-block">
        <AgentLog log={log} language={language} now={now ?? new Date()} onToggleRow={onToggleRow} onOpenEntry={onOpenEntry} onMore={onMore} />
      </div>
    </section>
  );
}
