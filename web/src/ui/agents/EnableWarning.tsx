import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../controls/Button";
import { Modal } from "../settings/Modal";
import "./agents.css";

/**
 * The warning on enabling access (spec §9.3): "the text the agent reads goes to the provider of its model". A single
 * `Modal` over the window: the page under it does not change its `overflow`, and its content does not shift. Default focus is on
 * "Отмена"; Esc and the scrim cancel.
 */
export function EnableWarning({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  const { t } = useTranslation();
  const actions = useRef<HTMLDivElement>(null);
  // Modal focuses the close button in its own effect, which runs after the children's effects: moving focus to "Отмена" happens later.
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
