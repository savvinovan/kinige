import { useEffect, useId, useRef, useState, type ReactNode, type RefObject } from "react";
import { IconButton } from "../controls/IconButton";
import "./modal.css";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface ModalProps {
  /** The window title; `aria-labelledby` points to it. */
  title: string;
  /** The label of the close button (for the screen reader and the tooltip). */
  closeLabel: string;
  onClose: () => void;
  /**
   * Where to return focus on close. By default the element that was focused at the moment of opening; if it was removed from
   * the page by then (an item of a menu that has closed), there is nowhere to return focus, and the parent should pass the trigger here.
   */
  returnFocusRef?: RefObject<HTMLElement | null>;
  /**
   * The window can be closed: the cross, Esc and a click on the scrim. Without this (a running import) there is nothing to close
   * the window with except the buttons inside it; Tab cycles as usual.
   */
  closable?: boolean;
  children: ReactNode;
}

// `tabindex="-1"` takes an element out of the Tab order (the selected option of a radio group is the group's only stop),
// although the button selector catches it: such elements are discarded.
const focusablesIn = (root: HTMLElement) =>
  [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.getAttribute("tabindex") !== "-1");

/** All windows except the top one (the last in the DOM) get `data-covered`; the top one does not. */
function markCovered() {
  const modals = [...document.querySelectorAll<HTMLElement>(".modal")];
  modals.forEach((modal, index) => modal.toggleAttribute("data-covered", index < modals.length - 1));
}

/**
 * A modal inside the application's main window (not a second OS window): a scrim, a centered panel, a title and a close
 * button, scrollable content. Without a native `<dialog>`: `showModal` is not available everywhere (not in jsdom), and we need
 * keyboard behavior that is testable and the same in WKWebView.
 *
 * Keyboard and focus:
 * - on open, focus goes to the first control (the close button), on close it returns to the opening element;
 * - Tab and Shift+Tab cycle inside the window (the window computes the move itself, not the browser); focus that lands outside is brought back;
 * - Esc and a click on the scrim close the window.
 *
 * The appearance is a short fade and shift; with Reduce Motion the durations are zero (themes.css) and the window appears at once.
 */
export function Modal({ title, closeLabel, onClose, returnFocusRef, closable = true, children }: ModalProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  // The opening element is remembered before the panel's first render, while focus is still on it.
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  // A fresh handler without resubscribing to events: the parent may pass a new function on every render.
  const onCloseRef = useRef(onClose);
  const closableRef = useRef(closable);
  useEffect(() => {
    onCloseRef.current = onClose;
    closableRef.current = closable;
  });

  useEffect(() => {
    const root = panel.current;
    if (!root) return;
    (focusablesIn(root)[0] ?? root).focus();

    // A window over a window (the warning over "Доступ ИИ-агентов"): only the top one handles the keyboard and focus.
    const isTopmost = () => {
      const panels = document.querySelectorAll(".modal__panel");
      return panels[panels.length - 1] === root;
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (!isTopmost()) return;
      if (event.key === "Escape") {
        if (event.defaultPrevented) return;
        event.preventDefault();
        if (closableRef.current) onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      event.preventDefault();
      const items = focusablesIn(root);
      if (items.length === 0) {
        root.focus();
        return;
      }
      const index = items.indexOf(document.activeElement as HTMLElement);
      const next = event.shiftKey ? (index <= 0 ? items.length - 1 : index - 1) : index === items.length - 1 ? 0 : index + 1;
      items[next]?.focus();
    };
    // Focus must not leave the window, neither by a click nor programmatically.
    const onFocusIn = (event: FocusEvent) => {
      if (isTopmost() && event.target instanceof Node && !root.contains(event.target)) (focusablesIn(root)[0] ?? root).focus();
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("focusin", onFocusIn);
    // The lower window under the top one is marked `data-covered`: its scrollbar in WebKit showed through the top window
    // (a second "tab" next to the main scrollbar, slice 4a); CSS removes scrolling from the covered window.
    markCovered();
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("focusin", onFocusIn);
      // The top window leaves the DOM after the effect: the mark is recomputed when it is no longer there.
      queueMicrotask(markCovered);
      // The parent's ref is read exactly at the moment of closing: we need the element that is there now, not at the time of opening.
      // eslint-disable-next-line react-hooks/exhaustive-deps
      const target = returnFocusRef?.current ?? opener;
      if (target?.isConnected) target.focus();
    };
  }, [opener, returnFocusRef]);

  return (
    <div className="modal">
      <div className="modal__scrim" aria-hidden="true" onClick={closable ? onClose : undefined} />
      <div className="modal__panel" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} ref={panel}>
        <header className="modal__header">
          <h2 className="modal__title" id={titleId}>
            {title}
          </h2>
          {closable ? <IconButton icon="close" label={closeLabel} onClick={onClose} /> : null}
        </header>
        <div className="modal__body">{children}</div>
      </div>
    </div>
  );
}
