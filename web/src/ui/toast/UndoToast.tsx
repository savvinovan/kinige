import { useEffect, useRef, useState } from "react";
import { Button } from "../controls/Button";
import "./toast.css";

/** Сколько тост «Отменить» висит (ТЗ §5: 5 секунд). */
export const UNDO_TOAST_MS = 5000;

interface UndoToastProps {
  /** Показан ли тост. Область для диктора (`role="status"`) есть всегда: так обновление объявляется надёжно. */
  open: boolean;
  /** Что произошло: «Сообщение удалено». */
  message: string;
  onUndo: () => void;
  /** Время вышло: действие остаётся в силе. */
  onExpire: () => void;
  durationMs?: number;
}

/**
 * Тост «Отменить» на 5 секунд. Объявляется диктором (`role="status"`, вежливо), кнопка доступна с клавиатуры.
 * Таймер стоит, пока на тост наведена мышь или в нём фокус: человек успевает нажать (WCAG 2.2.1). Нужен позиционированный
 * родитель: тост прижимается к его нижнему краю, над полем ввода.
 */
export function UndoToast({ open, message, onUndo, onExpire, durationMs = UNDO_TOAST_MS }: UndoToastProps) {
  const [paused, setPaused] = useState(false);
  const remaining = useRef(durationMs);

  useEffect(() => {
    if (!open) {
      remaining.current = durationMs;
      return;
    }
    if (paused) return;
    const startedAt = Date.now();
    const timer = setTimeout(onExpire, remaining.current);
    return () => {
      clearTimeout(timer);
      remaining.current -= Date.now() - startedAt;
    };
  }, [open, paused, durationMs, onExpire]);

  return (
    <div className="toast-region" role="status" aria-live="polite">
      {open ? (
        <div
          className="toast"
          onMouseEnter={() => setPaused(true)}
          onMouseLeave={() => setPaused(false)}
          onFocus={() => setPaused(true)}
          onBlur={() => setPaused(false)}
        >
          <span className="toast__message">{message}</span>
          <Button onClick={onUndo}>Отменить</Button>
        </div>
      ) : null}
    </div>
  );
}
