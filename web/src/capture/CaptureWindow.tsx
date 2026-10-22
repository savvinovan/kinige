import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../api/KinigeApi";
import type { CaptureState } from "../api/generated/CaptureState";
import type { ErrorCode } from "../api/generated/ErrorCode";
import { useDraftSaver } from "../draft/useDraftSaver";
import { applyTextSize } from "../ui/screens/MainWindow";
import { CapturePanel } from "../ui/capture/CapturePanel";

/** Rust показал окно (см. `capture::show_capture`): страница перечитывает состояние. Без IPC-события: у окна нет права слушать. */
export const SHOWN_EVENT = "kinige-capture-shown";
/** Rust просит спрятать окно (хоткей при открытом окне, закрытие, потеря фокуса): страница сохраняет черновик и прячется. */
export const HIDE_REQUEST_EVENT = "kinige-capture-hide-request";

interface CaptureWindowProps {
  api: KinigeApi;
  /** Состояние, прочитанное до первого кадра. */
  initial: CaptureState;
}

/**
 * Окно быстрого захвата (слайс 2) на `KinigeApi`: контекст за сегодня, поле ввода, Enter — новая запись, ⌘Enter — дописать,
 * Esc — спрятать. Черновик уходит в Rust через ~300 мс после последнего нажатия и при скрытии. После успеха — тишина: окно
 * просто прячется (ни тоста, ни подтверждения). Ошибка записи оставляет текст в поле, над полем — сообщение.
 */
export function CaptureWindow({ api, initial }: CaptureWindowProps) {
  const { t } = useTranslation();
  const [state, setState] = useState(initial);
  const [value, setValue] = useState(initial.draft);
  const [error, setError] = useState<ErrorCode | null>(initial.error);
  const busy = useRef(false);

  const saveDraft = useCallback((body: string) => api.captureSaveDraft(body), [api]);
  const draft = useDraftSaver(saveDraft);
  const { flush, cancel, schedule } = draft;

  /** Сохраняет черновик и прячет окно. */
  const hide = useCallback(() => {
    void flush().finally(() => void api.captureHide().catch(() => undefined));
  }, [api, flush]);

  // Показ: свежий контекст и черновик (записи могли появиться из главного окна, буфера и с другого экрана), размер текста.
  useEffect(() => {
    const refresh = () => {
      api
        .captureState()
        .then((fresh) => {
          applyTextSize(fresh.textSize);
          setState(fresh);
          setValue(fresh.draft);
          setError(fresh.error);
          // Окно показали заново: страница могла считать фокус своим всё это время (события focus нет), поэтому курсор
          // ставится в поле явно.
          requestAnimationFrame(() => document.querySelector("textarea")?.focus());
        })
        .catch((reason: unknown) => setError(errorCode(reason)));
    };
    window.addEventListener(SHOWN_EVENT, refresh);
    window.addEventListener(HIDE_REQUEST_EVENT, hide);
    return () => {
      window.removeEventListener(SHOWN_EVENT, refresh);
      window.removeEventListener(HIDE_REQUEST_EVENT, hide);
    };
  }, [api, hide]);

  const submit = (action: (body: string) => ReturnType<KinigeApi["captureSend"]>) => {
    if (busy.current || value.trim() === "") return;
    busy.current = true;
    // Пока идёт запись, отложенное сохранение черновика отменено: Rust очистит черновик сам, в той же транзакции.
    cancel();
    action(value)
      .then(() => {
        setValue("");
        setError(null);
        void api.captureHide().catch(() => undefined);
      })
      .catch((reason: unknown) => {
        // Текст остаётся в поле, над полем — ошибка; черновик возвращается в очередь сохранения.
        setError(errorCode(reason));
        schedule(value);
      })
      .finally(() => {
        busy.current = false;
      });
  };

  return (
    <CapturePanel
      context={state.context}
      appendTargetId={state.appendTarget}
      value={value}
      onChange={(next) => {
        setValue(next);
        setError(null);
        schedule(next);
      }}
      onSend={() => submit((body) => api.captureSend(body))}
      onAppend={() => submit((body) => api.captureAppend(body))}
      onHide={hide}
      error={error ? t(`error.${error}`) : undefined}
    />
  );
}
