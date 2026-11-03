import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import type { ImportProgress } from "../../api/generated/ImportProgress";
import type { ImportReport } from "../../api/generated/ImportReport";
import { useLanguage } from "../../i18n/I18n";
import { formatSize } from "../attachments/formatSize";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { ProgressBar } from "../controls/ProgressBar";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import "./import.css";

export type ImportStep = "pick" | "choose" | "running" | "report";

export interface ImportDialogProps {
  step: ImportStep;
  /** The error of the last action (picking a file, starting). On the file step it stands above the button, on the chats step above the list. */
  error?: ErrorCode | null;
  /** The file is being opened: the button is unavailable. */
  opening?: boolean;
  preview?: ImportPreview;
  /** The checked chats (Telegram ids). */
  selected?: ReadonlySet<string>;
  progress?: ImportProgress;
  /** A stop was requested, the import is finishing the current batch. */
  stopping?: boolean;
  report?: ImportReport;
  onPick: () => void;
  onToggle: (chatId: string, on: boolean) => void;
  onRun: () => void;
  onStop: () => void;
  /** The cross, Esc, the backdrop and "Done". Not called on the progress step: there is nothing to close the window with except "Stop". */
  onClose: () => void;
}

/**
 * The "Import from Telegram" window over the main one (Modal): instructions and file picking -> a chat list with checkboxes -> progress with
 * "Stop" -> a report. The report is quiet lines without colored bubbles; an error is the D1 error line. No toasts or animations.
 * On the progress step there is no cross and no Esc: the window is closed only by "Done" after the report.
 */
export function ImportDialog(props: ImportDialogProps) {
  const { t } = useTranslation();
  const { step, onClose } = props;
  return (
    <Modal title={t("import.title")} closeLabel={t("common.close")} onClose={onClose} closable={step !== "running"}>
      <div className="import">
        {step === "pick" ? <PickStep {...props} /> : null}
        {step === "choose" ? <ChooseStep {...props} /> : null}
        {step === "running" ? <RunningStep {...props} /> : null}
        {step === "report" ? <ReportStep {...props} /> : null}
      </div>
    </Modal>
  );
}

function ErrorLine({ error }: { error?: ErrorCode | null }) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <StatusNote tone="error" icon="alert" role="alert">
      {t(`error.${error}`)}
    </StatusNote>
  );
}

function PickStep({ error, opening, onPick }: ImportDialogProps) {
  const { t } = useTranslation();
  return (
    <>
      <p className="import__text">{t("import.instruction")}</p>
      <ErrorLine error={error} />
      <div className="import__actions">
        <Button variant="secondary" aria-disabled={opening || undefined} onClick={() => !opening && onPick()}>
          {opening ? t("import.opening") : t("import.choose")}
        </Button>
      </div>
    </>
  );
}

function ChooseStep({ error, preview, selected, opening, onToggle, onRun, onPick }: ImportDialogProps) {
  const { t } = useTranslation();
  const chats = preview?.chats ?? [];
  const chosen = selected ?? new Set<string>();
  const canRun = chats.some((chat) => chosen.has(chat.id) && chat.total > 0);
  return (
    <>
      <ErrorLine error={error} />
      {chats.length === 0 ? (
        <p className="import__text import__text--quiet">{t("import.nothing")}</p>
      ) : (
        <ul className="import__chats" aria-label={t("import.chats")}>
          {chats.map((chat) => {
            const name = chat.kind === "saved" ? t("import.savedName") : (chat.name ?? chat.id);
            const entries = chat.total === 0 ? t("import.noEntries") : t("count.entries", { count: chat.total });
            const existing = chat.existing > 0 ? ` · ${t("import.existing", { count: chat.existing })}` : "";
            // For "Saved Messages" the name is the type: we do not repeat it a second time.
            const kind = chat.kind === "saved" ? "" : `${t(`import.kind.${chat.kind}`)} · `;
            return (
              <li key={chat.id} className="import__chat">
                <Checkbox
                  checked={chosen.has(chat.id) && chat.total > 0}
                  disabled={chat.total === 0}
                  onChange={(event) => onToggle(chat.id, event.target.checked)}
                  description={`${kind}${entries}${existing}`}
                >
                  {name}
                </Checkbox>
              </li>
            );
          })}
        </ul>
      )}
      {preview && preview.skippedChats > 0 ? (
        <p className="import__text import__text--quiet">{t("import.skippedChats", { count: preview.skippedChats })}</p>
      ) : null}
      <MediaLine chats={chats.filter((chat) => chosen.has(chat.id) && chat.total > 0)} />

      {chats.length > 0 ? <p className="import__text import__text--quiet">{t("import.twoDevices")}</p> : null}
      <div className="import__actions">
        <Button variant="secondary" aria-disabled={opening || undefined} onClick={() => !opening && onPick()}>
          {t("import.otherFile")}
        </Button>
        <Button variant="secondary" aria-disabled={!canRun || undefined} onClick={() => canRun && onRun()}>
          {t("import.run")}
        </Button>
      </div>
    </>
  );
}

/** "Media: 1,240 files, 6.7 GB · not in the export: 12" for the checked chats; nothing when the export holds no files (slice 7). */
function MediaLine({ chats }: { chats: ImportPreview["chats"] }) {
  const { t } = useTranslation();
  const language = useLanguage();
  const files = chats.reduce((sum, chat) => sum + chat.mediaFiles, 0);
  const bytes = chats.reduce((sum, chat) => sum + chat.mediaBytes, 0);
  const missing = chats.reduce((sum, chat) => sum + chat.mediaMissing, 0);
  if (files === 0 && missing === 0) return null;
  const head = files > 0 ? t("import.media", { count: files, size: formatSize(bytes, language) }) : t("import.mediaNone");
  const tail = missing > 0 ? ` · ${t("import.mediaMissing", { count: missing })}` : "";
  return <p className="import__text import__text--quiet">{`${head}${tail}`}</p>;
}

function RunningStep({ progress, stopping, error, onStop }: ImportDialogProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const stop = useRef<HTMLButtonElement>(null);
  // There is no cross on this step, and the previous step's button vanished together with it: focus moves to "Stop".
  useEffect(() => stop.current?.focus(), []);
  const messages = progress?.done ?? 0;
  const messagesTotal = progress?.total ?? 0;
  // With files the bar runs in bytes ("1.2 GB of 6.7 GB"), because copying them is what takes the time (slice 7).
  const bytesTotal = progress?.bytesTotal ?? 0;
  const byBytes = bytesTotal > 0;
  const done = byBytes ? (progress?.bytesDone ?? 0) : messages;
  const total = byBytes ? bytesTotal : messagesTotal;
  const text = byBytes
    ? t("import.progressBytes", { done: formatSize(done, language), total: formatSize(total, language) })
    : t("import.progressText", { done, total });
  return (
    <>
      <p className="import__text" role="status">
        {stopping ? t("import.stopping") : t("import.running")}
      </p>
      <ProgressBar value={done} max={total} label={t("import.progress")} valueText={text} />
      <p className="import__text import__text--quiet import__progress-text">{text}</p>
      <ErrorLine error={error} />
      <div className="import__actions">
        <Button variant="secondary" ref={stop} aria-disabled={stopping || undefined} onClick={() => !stopping && onStop()}>
          {t("import.stop")}
        </Button>
      </div>
    </>
  );
}

function ReportStep({ report, onClose }: ImportDialogProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const done = useRef<HTMLButtonElement>(null);
  useEffect(() => done.current?.focus(), []);
  if (!report) return null;
  const lines: string[] = [t("import.report.imported", { count: report.imported })];
  if (report.skippedExisting > 0) lines.push(t("import.report.existing", { count: report.skippedExisting }));
  if (report.skippedService > 0) lines.push(t("import.report.service", { count: report.skippedService }));
  if (report.skippedEmpty > 0) lines.push(t("import.report.empty", { count: report.skippedEmpty }));
  if (report.mediaAttached > 0) {
    lines.push(t("import.report.mediaAttached", { count: report.mediaAttached, size: formatSize(report.mediaBytes, language) }));
  }
  if (report.mediaMissing > 0) lines.push(t("import.report.mediaMissing", { count: report.mediaMissing }));
  if (report.mediaSkipped > 0) lines.push(t("import.report.mediaSkipped", { count: report.mediaSkipped }));
  if (report.mediaPlaceholders > 0) lines.push(t("import.report.media", { count: report.mediaPlaceholders }));
  if (report.chatsCreated > 0) lines.push(t("import.report.chats", { count: report.chatsCreated }));
  if (report.backupName) lines.push(t("import.report.backup", { name: report.backupName }));
  return (
    <>
      <h3 className="import__heading">{report.stopped || report.diskFull ? t("import.report.titleStopped") : t("import.report.title")}</h3>
      <ul className="import__report">
        {lines.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
      {report.diskFull ? <p className="import__text">{t("import.report.diskFull")}</p> : null}
      {report.stopped ? <p className="import__text import__text--quiet">{t("import.report.stoppedNote")}</p> : null}
      <div className="import__actions">
        <Button variant="secondary" ref={done} onClick={onClose}>
          {t("import.report.done")}
        </Button>
      </div>
    </>
  );
}
