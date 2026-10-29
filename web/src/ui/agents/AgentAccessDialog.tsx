import { useEffect, useId, useRef, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { AgentAccess } from "../../api/generated/AgentAccess";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentMode } from "../../api/generated/AgentMode";
import type { Language } from "../../api/generated/Language";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { Modal } from "../settings/Modal";
import { SegmentedChoice } from "../settings/SegmentedChoice";
import { StatusNote } from "../settings/StatusNote";
import { AgentChats } from "./AgentChats";
import { AgentConnect } from "./AgentConnect";
import { AgentLog, type AgentLogView } from "./AgentLog";
import "./agents.css";
import "../settings/settings.css";

export interface AgentAccessDialogProps {
  access: AgentAccess;
  log: AgentLogView;
  language: Language;
  /** «Сейчас» для меток времени журнала; по умолчанию часы окна. В тестах и историях фиксируется. */
  now?: Date;
  /** Показано предупреждение «текст уйдёт провайдеру модели»: вторым окном поверх. */
  warning: boolean;
  /** Уже переведённый текст ошибки последнего действия (`role="alert"`). */
  error?: string;
  /** Флажок доступа. Включение родитель сначала подтверждает предупреждением. */
  onEnabledChange: (enabled: boolean) => void;
  onConfirmEnable: () => void;
  onCancelEnable: () => void;
  onModeChange: (mode: AgentMode) => void;
  onChatToggle: (id: string, on: boolean) => void;
  onToggleRow: (id: number) => void;
  onOpenEntry: (logId: number, entry: AgentLogEntry) => void;
  onMoreLog: () => void;
  onClose: () => void;
  returnFocusRef?: RefObject<HTMLElement | null>;
}

/**
 * Окно «Доступ ИИ-агентов» (слайс 4a, ADR-0009) поверх главного: флажок доступа, режим, список чатов «видно агенту»,
 * «Как подключить» и «Журнал обращений». Пока доступ выключен, режим и чаты недоступны, журнал виден. Включение требует
 * предупреждения (ТЗ §9.3): оно открывается вторым окном. Только вид и поведение: данные и действия приходят снаружи.
 */
export function AgentAccessDialog({
  access,
  log,
  language,
  now,
  warning,
  error,
  onEnabledChange,
  onConfirmEnable,
  onCancelEnable,
  onModeChange,
  onChatToggle,
  onToggleRow,
  onOpenEntry,
  onMoreLog,
  onClose,
  returnFocusRef,
}: AgentAccessDialogProps) {
  const { t } = useTranslation();
  const modeTitleId = useId();
  const chatsTitleId = useId();
  const connectTitleId = useId();
  const logTitleId = useId();
  const off = !access.enabled;
  return (
    <>
      <Modal title={t("agent.title")} closeLabel={t("agent.close")} onClose={onClose} returnFocusRef={returnFocusRef}>
        <section className="settings__section">
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
        </section>
        <section className="settings__section" aria-labelledby={modeTitleId}>
          <h3 className="settings__heading" id={modeTitleId}>
            {t("agent.mode.title")}
          </h3>
          <SegmentedChoice<AgentMode>
            labelledBy={modeTitleId}
            value={access.mode}
            disabled={off}
            onChange={onModeChange}
            options={[
              { value: "read", label: t("agent.mode.read") },
              { value: "read_write", label: t("agent.mode.readWrite") },
            ]}
          />
          <p className="settings__description">{t("agent.mode.hint")}</p>
        </section>
        <section className="settings__section" aria-labelledby={chatsTitleId}>
          <h3 className="settings__heading" id={chatsTitleId}>
            {t("agent.chats.title")}
          </h3>
          <p className="settings__description">{t("agent.chats.description")}</p>
          <AgentChats chats={access.chats} disabled={off} onToggle={onChatToggle} />
        </section>
        <section className="settings__section" aria-labelledby={connectTitleId}>
          <h3 className="settings__heading" id={connectTitleId}>
            {t("agent.connect.title")}
          </h3>
          <AgentConnect socket={access.socket} />
        </section>
        <section className="settings__section" aria-labelledby={logTitleId}>
          <h3 className="settings__heading" id={logTitleId}>
            {t("agent.log.title")}
          </h3>
          <AgentLog log={log} language={language} now={now ?? new Date()} onToggleRow={onToggleRow} onOpenEntry={onOpenEntry} onMore={onMoreLog} />
        </section>
      </Modal>
      {warning ? <EnableWarning onConfirm={onConfirmEnable} onCancel={onCancelEnable} /> : null}
    </>
  );
}

/** Предупреждение при включении (ТЗ §9.3): фокус по умолчанию на «Отмена», Esc и подложка отменяют. */
function EnableWarning({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  const { t } = useTranslation();
  const actions = useRef<HTMLDivElement>(null);
  // Modal ставит фокус на кнопку закрытия в своём эффекте, который идёт после эффектов потомков: перенос на «Отмена» — позже.
  useEffect(() => {
    queueMicrotask(() => actions.current?.querySelector("button")?.focus());
  }, []);
  return (
    <Modal title={t("agent.enable.warningTitle")} closeLabel={t("common.close")} onClose={onCancel}>
      <div className="agent-warning">
        <p className="agent-warning__text">{t("agent.enable.warning")}</p>
        <div className="agent-warning__actions" ref={actions}>
          <Button variant="secondary" onClick={onCancel}>
            {t("agent.enable.cancel")}
          </Button>
          <Button onClick={onConfirm}>{t("agent.enable.confirm")}</Button>
        </div>
      </div>
    </Modal>
  );
}
