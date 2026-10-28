import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import type { ImportProgress } from "../../api/generated/ImportProgress";
import type { ImportReport } from "../../api/generated/ImportReport";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { ProgressBar } from "../controls/ProgressBar";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import "./import.css";

export type ImportStep = "pick" | "choose" | "running" | "report";

export interface ImportDialogProps {
  step: ImportStep;
  /** Ошибка последнего действия (выбор файла, запуск). На шаге выбора файла стоит над кнопкой, на шаге чатов — над списком. */
  error?: ErrorCode | null;
  /** Идёт открытие файла: кнопка недоступна. */
  opening?: boolean;
  preview?: ImportPreview;
  /** Отмеченные чаты (id Telegram). */
  selected?: ReadonlySet<string>;
  progress?: ImportProgress;
  /** Остановка запрошена, импорт дописывает пачку. */
  stopping?: boolean;
  report?: ImportReport;
  onPick: () => void;
  onToggle: (chatId: string, on: boolean) => void;
  onRun: () => void;
  onStop: () => void;
  /** Крестик, Esc, подложка и «Готово». На шаге прогресса не вызывается: закрыть окно нечем, кроме «Остановить». */
  onClose: () => void;
}

/**
 * Окно «Импорт из Telegram» поверх главного (Modal): инструкция и выбор файла → список чатов с флажками → прогресс с
 * «Остановить» → отчёт. Отчёт — тихие строки без цветных плашек; ошибка — строка ошибки D1. Тостов и анимаций нет.
 * На шаге прогресса крестика и Esc нет: окно закрывает только «Готово» после отчёта.
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
            return (
              <li key={chat.id} className="import__chat">
                <Checkbox
                  checked={chosen.has(chat.id) && chat.total > 0}
                  disabled={chat.total === 0}
                  onChange={(event) => onToggle(chat.id, event.target.checked)}
                  description={`${t(`import.kind.${chat.kind}`)} · ${entries}${existing}`}
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

function RunningStep({ progress, stopping, error, onStop }: ImportDialogProps) {
  const { t } = useTranslation();
  const stop = useRef<HTMLButtonElement>(null);
  // Крестика на этом шаге нет, а кнопка предыдущего шага исчезла вместе с ним: фокус переходит на «Остановить».
  useEffect(() => stop.current?.focus(), []);
  const done = progress?.done ?? 0;
  const total = progress?.total ?? 0;
  const text = t("import.progressText", { done, total });
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
  const done = useRef<HTMLButtonElement>(null);
  useEffect(() => done.current?.focus(), []);
  if (!report) return null;
  const lines: string[] = [t("import.report.imported", { count: report.imported })];
  if (report.skippedExisting > 0) lines.push(t("import.report.existing", { count: report.skippedExisting }));
  if (report.skippedService > 0) lines.push(t("import.report.service", { count: report.skippedService }));
  if (report.skippedEmpty > 0) lines.push(t("import.report.empty", { count: report.skippedEmpty }));
  if (report.mediaPlaceholders > 0) lines.push(t("import.report.media", { count: report.mediaPlaceholders }));
  if (report.chatsCreated > 0) lines.push(t("import.report.chats", { count: report.chatsCreated }));
  if (report.backupName) lines.push(t("import.report.backup", { name: report.backupName }));
  return (
    <>
      <h3 className="import__heading">{report.stopped ? t("import.report.titleStopped") : t("import.report.title")}</h3>
      <ul className="import__report">
        {lines.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
      {report.stopped ? <p className="import__text import__text--quiet">{t("import.report.stoppedNote")}</p> : null}
      <div className="import__actions">
        <Button variant="secondary" ref={done} onClick={onClose}>
          {t("import.report.done")}
        </Button>
      </div>
    </>
  );
}
