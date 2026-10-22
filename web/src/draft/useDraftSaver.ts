import { useCallback, useEffect, useRef } from "react";

/** Через сколько после последнего нажатия клавиши черновик уходит в Rust (§5.1: ~300 мс). */
export const DRAFT_DELAY_MS = 300;

export interface DraftSaver {
  /** Запланировать сохранение текста (последний вызов побеждает). */
  schedule(body: string): void;
  /** Сохранить сейчас то, что запланировано (при скрытии окна, потере фокуса, закрытии). Ошибки молча глотаются: черновик — удобство, не данные. */
  flush(): Promise<void>;
  /** Отменить запланированное: после отправки Rust уже очистил черновик, поздняя запись вернула бы его. */
  cancel(): void;
}

/**
 * Черновик с задержкой: пока человек печатает, в Rust не уходит ни один запрос; через `DRAFT_DELAY_MS` после последнего
 * нажатия — одно сохранение. При размонтировании запланированное сохраняется.
 */
export function useDraftSaver(save: (body: string) => Promise<void>, delayMs = DRAFT_DELAY_MS): DraftSaver {
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pending = useRef<string | null>(null);

  const flush = useCallback((): Promise<void> => {
    clearTimeout(timer.current);
    timer.current = undefined;
    const body = pending.current;
    pending.current = null;
    return body === null ? Promise.resolve() : save(body).catch(() => undefined);
  }, [save]);

  const schedule = useCallback(
    (body: string) => {
      pending.current = body;
      clearTimeout(timer.current);
      timer.current = setTimeout(() => void flush(), delayMs);
    },
    [flush, delayMs],
  );

  const cancel = useCallback(() => {
    clearTimeout(timer.current);
    timer.current = undefined;
    pending.current = null;
  }, []);

  useEffect(
    () => () => {
      void flush();
    },
    [flush],
  );

  return { schedule, flush, cancel };
}
