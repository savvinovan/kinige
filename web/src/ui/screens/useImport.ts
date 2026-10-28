import { useCallback, useEffect, useRef, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ImportOpened } from "../../api/generated/ImportOpened";
import type { ImportProgress } from "../../api/generated/ImportProgress";
import type { ImportReport } from "../../api/generated/ImportReport";
import type { ImportDialogProps, ImportStep } from "../import/ImportDialog";

interface UseImportOptions {
  api: KinigeApi;
  /** Импорт что-то записал (или остановлен после записи): счётчики и лента обновляются. */
  onChanged: () => void;
  /** «Готово» после отчёта: окно закрыто, открыть чат «Избранного». */
  onDone: () => void;
}

/**
 * Окно импорта из Telegram: шаги «файл → чаты → прогресс → отчёт». Файл выбирает и читает Rust; здесь только номер сессии,
 * отмеченные чаты и ход импорта. «Избранное» отмечено заранее, личные чаты — нет (решение пользователя 3). Окно закрывается
 * на любом шаге, кроме прогресса; закрытие забывает разобранный файл.
 */
export function useImport({ api, onChanged, onDone }: UseImportOptions) {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState<ImportStep>("pick");
  const [error, setError] = useState<ErrorCode | null>(null);
  const [opening, setOpening] = useState(false);
  const [opened, setOpened] = useState<ImportOpened | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [progress, setProgress] = useState<ImportProgress | undefined>(undefined);
  const [stopping, setStopping] = useState(false);
  const [report, setReport] = useState<ImportReport | undefined>(undefined);
  // Идёт импорт: второй запуск и закрытие окна в это время невозможны (кнопок нет, но события клавиатуры не должны пройти).
  const running = useRef(false);
  const picking = useRef(false);

  // Прогресс приходит событием Rust; подписка живёт, пока окно открыто (событие может прийти раньше ответа команды).
  useEffect(() => {
    if (!open) return;
    let off: (() => void) | undefined;
    let closed = false;
    api
      .onImportProgress(setProgress)
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api, open]);

  const reset = useCallback(() => {
    setOpen(false);
    setStep("pick");
    setError(null);
    setOpening(false);
    setOpened(null);
    setSelected(new Set());
    setProgress(undefined);
    setStopping(false);
    setReport(undefined);
  }, []);

  const start = useCallback(() => {
    reset();
    setOpen(true);
  }, [reset]);

  const close = useCallback(() => {
    if (running.current) return;
    // Разобранный файл забывается; ошибка забывания окну не нужна.
    void api.importTelegramClose().catch(() => undefined);
    reset();
  }, [api, reset]);

  const pick = useCallback(() => {
    if (picking.current || running.current) return;
    picking.current = true;
    setOpening(true);
    setError(null);
    api
      .importTelegramOpen()
      .then((result) => {
        // Отмена диалога ничего не меняет.
        if (result === null) return;
        setOpened(result);
        setSelected(new Set(result.preview.chats.filter((c) => c.kind === "saved" && c.total > 0).map((c) => c.id)));
        setStep("choose");
      })
      .catch((reason: unknown) => setError(errorCode(reason)))
      .finally(() => {
        picking.current = false;
        setOpening(false);
      });
  }, [api]);

  const toggle = useCallback((chatId: string, on: boolean) => {
    setSelected((was) => {
      const next = new Set(was);
      if (on) next.add(chatId);
      else next.delete(chatId);
      return next;
    });
  }, []);

  const run = useCallback(() => {
    if (running.current || !opened) return;
    const chatIds = opened.preview.chats.filter((c) => selected.has(c.id) && c.total > 0).map((c) => c.id);
    if (chatIds.length === 0) return;
    running.current = true;
    setError(null);
    setStopping(false);
    setProgress({ done: 0, total: opened.preview.chats.filter((c) => chatIds.includes(c.id)).reduce((s, c) => s + c.total, 0) });
    setStep("running");
    api
      .importTelegramRun(opened.session, chatIds)
      .then((result) => {
        setReport(result);
        setStep("report");
        onChanged();
      })
      .catch((reason: unknown) => {
        const code = errorCode(reason);
        setError(code);
        // Файл забыт: выбрать заново; иначе вернуться к списку (часть пачек могла записаться).
        setStep(code === "import_session_expired" ? "pick" : "choose");
        onChanged();
      })
      .finally(() => {
        running.current = false;
        setStopping(false);
      });
  }, [api, opened, selected, onChanged]);

  const stop = useCallback(() => {
    if (!running.current) return;
    setStopping(true);
    void api.importTelegramStop().catch(() => undefined);
  }, [api]);

  const done = useCallback(() => {
    close();
    onDone();
  }, [close, onDone]);

  const dialog: ImportDialogProps | null = open
    ? {
        step,
        error,
        opening,
        preview: opened?.preview,
        selected,
        progress,
        stopping,
        report,
        onPick: pick,
        onToggle: toggle,
        onRun: run,
        onStop: stop,
        onClose: step === "report" ? done : close,
      }
    : null;
  return { dialog, start };
}
