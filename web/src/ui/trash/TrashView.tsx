import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../api/generated/Language";
import type { TrashItem } from "../../api/generated/TrashItem";
import { renderContextMarkdown } from "../capture/contextMarkdown";
import { Button } from "../controls/Button";
import { formatDayLabel } from "../feed/dates";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import "../message/markdown.css";
import "./trash.css";

interface TrashViewProps {
  items: readonly TrashItem[];
  language: Language;
  /** Сегодняшний `journal_date` (`YYYY-MM-DD`) для меток «Сегодня» и «Вчера»: его присылает Rust. */
  today: string;
  /** Запись, над которой сейчас идёт действие (восстановление или удаление): её кнопки на это время недоступны. */
  busyId?: string | null;
  /** Уже переведённая ошибка последнего действия: `role="alert"` над списком. */
  error?: string;
  onRestore: (id: string) => void;
  /** Только после подтверждения «Удалить навсегда». */
  onPurge: (id: string) => void;
}

const DEFAULT_ICON = "💬";

/**
 * Корзина: открывается на месте ленты. Плашка на запись: эмодзи и имя чата, день записи, «осталось N дн.», текст записи
 * (как в ленте, без действий, обрезан по строкам), «Восстановить» и «Удалить навсегда». «Удалить навсегда» сначала
 * спрашивает подтверждение (единственное подтверждение в приложении: действие необратимо; фокус по умолчанию на «Отмена»).
 * Кнопки «Очистить корзину» нет. Заголовка над списком нет, как и над лентой; пустая корзина — тихая строка.
 */
export function TrashView({ items, language, today, busyId = null, error, onRestore, onPurge }: TrashViewProps) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState<TrashItem | null>(null);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const taskLabels = { done: t("message.taskDone"), todo: t("message.taskTodo") };

  return (
    // Прокручиваемая область берёт фокус с клавиатуры, как лента.
    <section className="trash" aria-label={t("trash.label")} tabIndex={0}>
      <div className="trash__column">
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {error}
          </StatusNote>
        ) : null}
        {items.length === 0 ? (
          <p className="trash__empty">{t("trash.empty")}</p>
        ) : (
          <ol className="trash__list">
            {items.map((item) => {
              const { message } = item;
              const chat = item.chatIsInbox && item.chatTitle === "" ? t("chat.inbox") : item.chatTitle;
              const busy = busyId === message.id;
              const left = item.daysLeft <= 0 ? t("trash.deletesToday") : t("trash.daysLeft", { count: item.daysLeft });
              return (
                <li key={message.id}>
                  <article className="trash-entry" aria-busy={busy || undefined}>
                    <header className="trash-entry__head">
                      <span className="trash-entry__chat" title={chat}>
                        <span className="trash-entry__emoji" aria-hidden="true">
                          {item.chatIcon ?? DEFAULT_ICON}
                        </span>
                        <span className="trash-entry__name">{chat}</span>
                      </span>
                      <span className="trash-entry__when">
                        {formatDayLabel(message.journalDate, today, language, labels)} · {left}
                      </span>
                    </header>
                    <div
                      className="markdown trash-entry__text"
                      // Тот же безопасный рендер, что и в ленте (markdown.ts), только без ссылок и остановок Tab.
                      dangerouslySetInnerHTML={{ __html: renderContextMarkdown(message.bodyMd, taskLabels) }}
                    />
                    <footer className="trash-entry__actions">
                      <Button
                        variant="secondary"
                        aria-label={t("trash.restoreLabel", { chat })}
                        aria-disabled={busy || undefined}
                        onClick={() => {
                          if (!busy) onRestore(message.id);
                        }}
                      >
                        {t("trash.restore")}
                      </Button>
                      <Button
                        className="trash__danger"
                        aria-label={t("trash.purgeLabel", { chat })}
                        aria-disabled={busy || undefined}
                        onClick={() => {
                          if (!busy) setConfirming(item);
                        }}
                      >
                        {t("trash.purge")}
                      </Button>
                    </footer>
                  </article>
                </li>
              );
            })}
          </ol>
        )}
      </div>
      {confirming ? (
        <PurgeConfirm
          onCancel={() => setConfirming(null)}
          onConfirm={() => {
            const { id } = confirming.message;
            setConfirming(null);
            onPurge(id);
          }}
        />
      ) : null}
    </section>
  );
}

/** Подтверждение «Удалить навсегда?»: фокус по умолчанию на «Отмена», Esc и подложка отменяют. */
function PurgeConfirm({ onCancel, onConfirm }: { onCancel: () => void; onConfirm: () => void }) {
  const { t } = useTranslation();
  const actions = useRef<HTMLDivElement>(null);
  // Modal ставит фокус на кнопку закрытия в своём эффекте, который идёт после эффектов потомков: перенос на «Отмена» (первую
  // кнопку действий) — позже, в микрозадаче.
  useEffect(() => {
    queueMicrotask(() => actions.current?.querySelector("button")?.focus());
  }, []);
  return (
    <Modal title={t("trash.confirm.title")} closeLabel={t("common.close")} onClose={onCancel}>
      <div className="trash-confirm">
        <p className="trash-confirm__text">{t("trash.confirm.text")}</p>
        <div className="trash-confirm__actions" ref={actions}>
          <Button variant="secondary" onClick={onCancel}>
            {t("trash.confirm.cancel")}
          </Button>
          <Button className="trash__danger" onClick={onConfirm}>
            {t("trash.confirm.purge")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
