import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../api/generated/Language";
import type { TrashItem } from "../../api/generated/TrashItem";
import { MessageAttachments } from "../attachments/MessageAttachments";
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
  /** Today's `journal_date` (`YYYY-MM-DD`) for the "Today" and "Yesterday" labels: Rust sends it. */
  today: string;
  /** The entry an action is currently running on (restore or delete): its buttons are unavailable meanwhile. */
  busyId?: string | null;
  /** The already translated error of the last action: `role="alert"` above the list. */
  error?: string;
  onRestore: (id: string) => void;
  /** Only after the "Delete forever" confirmation. */
  onPurge: (id: string) => void;
  /** A click on a photo or a file card of a trashed entry (slice 7): it opens in the system app. Without it they are only shown. */
  onOpenAttachment?: (attachmentId: string) => void;
}

const DEFAULT_ICON = "💬";

/**
 * The trash: opens in place of the feed. One bubble per entry: the chat emoji and name, the entry day, "N days left", the entry text
 * (as in the feed, without actions, clamped by lines), "Restore" and "Delete forever". "Delete forever" first
 * asks for confirmation (the only confirmation in the app: the action is irreversible; the default focus is on "Cancel").
 * There is no "Empty trash" button. There is no heading above the list, as above the feed; an empty trash is a quiet line.
 */
export function TrashView({ items, language, today, busyId = null, error, onRestore, onPurge, onOpenAttachment }: TrashViewProps) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState<TrashItem | null>(null);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const taskLabels = { done: t("message.taskDone"), todo: t("message.taskTodo") };

  return (
    // The scrollable area takes keyboard focus, like the feed.
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
                    {message.attachments.length > 0 ? (
                      // The photos and files go to the trash with the entry and show there until it is restored or deleted for good.
                      <MessageAttachments
                        attachments={message.attachments}
                        onOpenImage={(id) => onOpenAttachment?.(id)}
                        onOpenFile={(id) => onOpenAttachment?.(id)}
                      />
                    ) : null}
                    <div
                      className="markdown trash-entry__text"
                      // The same safe rendering as in the feed (markdown.ts), only without links and Tab stops.
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

/** The "Delete forever?" confirmation: the default focus is on "Cancel", Esc and the backdrop cancel. */
function PurgeConfirm({ onCancel, onConfirm }: { onCancel: () => void; onConfirm: () => void }) {
  const { t } = useTranslation();
  const actions = useRef<HTMLDivElement>(null);
  // Modal puts focus on the close button in its own effect, which runs after the effects of its descendants: the move to "Cancel" (the first
  // action button) happens later, in a microtask.
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
