import { useEffect, useRef, useState, type RefObject } from "react";

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const focusablesIn = (root: HTMLElement) =>
  [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.getAttribute("tabindex") !== "-1");

interface DialogKeys {
  onClose: () => void;
  /** ArrowLeft (-1) and ArrowRight (1) with no modifier keys. */
  onStep?: (direction: -1 | 1) => void;
}

/**
 * The keyboard of a full-window dialog (the lightbox): Escape closes, the arrows step, Tab and Shift+Tab cycle inside `root`, focus that
 * lands outside is brought back, and on close focus returns to the element that opened the dialog. On open focus goes to `root`
 * itself (no ring is drawn there); the first Tab reaches the first control.
 */
export function useDialogKeys(root: RefObject<HTMLElement | null>, { onClose, onStep }: DialogKeys) {
  // The opener is remembered before the first render of the dialog, while focus is still on it.
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const handlers = useRef({ onClose, onStep });
  useEffect(() => {
    handlers.current = { onClose, onStep };
  });

  useEffect(() => {
    const dialog = root.current;
    if (!dialog) return;
    dialog.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        handlers.current.onClose();
        return;
      }
      if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault();
        handlers.current.onStep?.(event.key === "ArrowLeft" ? -1 : 1);
        return;
      }
      if (event.key !== "Tab") return;
      event.preventDefault();
      const items = focusablesIn(dialog);
      if (items.length === 0) {
        dialog.focus();
        return;
      }
      const index = items.indexOf(document.activeElement as HTMLElement);
      const next = event.shiftKey ? (index <= 0 ? items.length - 1 : index - 1) : index === items.length - 1 ? 0 : index + 1;
      items[next]?.focus();
    };
    const onFocusIn = (event: FocusEvent) => {
      if (event.target instanceof Node && !dialog.contains(event.target)) (focusablesIn(dialog)[0] ?? dialog).focus();
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("focusin", onFocusIn);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("focusin", onFocusIn);
      if (opener?.isConnected) opener.focus();
    };
  }, [root, opener]);
}
