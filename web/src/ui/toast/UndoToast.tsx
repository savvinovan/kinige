import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../controls/Button";
import "./toast.css";

/** How long the "Undo" toast stays: 5 seconds. */
export const UNDO_TOAST_MS = 5000;

interface UndoToastProps {
  /** Whether the toast is shown. The screen-reader region (`role="status"`) is always present: this way the update is announced reliably. */
  open: boolean;
  /** What happened: "Message deleted". */
  message: string;
  onUndo: () => void;
  /** Time ran out: the action stays in force. */
  onExpire: () => void;
  durationMs?: number;
}

/**
 * The "Undo" toast for 5 seconds is only for reversible actions like "Entry deleted" (slice 3). After an entry is sent
 * there is no toast: the entry just appears in the feed, routine actions have no confirmations (the user's decision of 21.10.2026).
 *
 * The toast ALWAYS overlays (`position: absolute`) and never shifts or squeezes the feed: the feed has neither reserved space nor
 * padding for it, the last bubble does not "jump". It is announced by the screen reader (`role="status"`, polite), the button is keyboard accessible.
 * The timer pauses while the mouse is over the toast or focus is inside it: the person has time to press (WCAG 2.2.1). A positioned
 * parent is required: the toast is pressed to its bottom edge, above the input field.
 */
export function UndoToast({ open, message, onUndo, onExpire, durationMs = UNDO_TOAST_MS }: UndoToastProps) {
  const { t } = useTranslation();
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
          <Button onClick={onUndo}>{t("toast.undo")}</Button>
        </div>
      ) : null}
    </div>
  );
}
