import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../controls/Button";
import { Modal } from "../settings/Modal";
import "./agents.css";

/**
 * Предупреждение при включении доступа (ТЗ §9.3): «текст, который прочитает агент, уходит провайдеру его модели». Один
 * `Modal` поверх окна: страница под ним свой `overflow` не меняет, и её содержимое не сдвигается. Фокус по умолчанию на
 * «Отмена», Esc и подложка отменяют.
 */
export function EnableWarning({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
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
