import { useEffect, useLayoutEffect, useRef, type KeyboardEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { EmojiPicker } from "./EmojiPicker";
import "./emoji.css";

export interface EmojiPopoverProps {
  recent: readonly string[];
  language: Language;
  onPick: (emoji: string) => void;
  /**
   * Close the panel. `returnFocus` says whether to return focus to where it came from (Esc, Tab, emoji choice): a click outside does not
   * return focus, the person has already chosen another place.
   */
  onClose: (returnFocus: boolean) => void;
  /** The "emoji" button: the panel stands above it, its right edge at the button's right edge. A click on the button is not an "outside click": the button toggles the panel itself. */
  anchor: RefObject<HTMLElement | null>;
}

/**
 * A popover panel with the picker above the "emoji" button of the input field. It overlays the window and shifts nothing. Esc, Tab,
 * an outside click, scrolling, a window resize and leaving the window close it; on opening, focus goes to the search.
 */
export function EmojiPopover({ recent, language, onPick, onClose, anchor }: EmojiPopoverProps) {
  const { t } = useTranslation();
  const panel = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // Before the first paint, directly in the DOM: the panel is pinned to the bottom and right edge of the window at the button (grows up and left) and is not
  // taller than the space above the button. The position is taken from the button's rectangle, so the panel does not flash in the corner of the window.
  useLayoutEffect(() => {
    const rect = anchor.current?.getBoundingClientRect();
    const element = panel.current;
    if (!rect || !element) return;
    element.style.bottom = `${window.innerHeight - rect.top}px`;
    element.style.right = `${window.innerWidth - rect.right}px`;
    element.style.setProperty("--emoji-popover-room", `${rect.top}px`);
  }, [anchor]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (panel.current?.contains(target) || anchor.current?.contains(target)) return;
      onCloseRef.current(false);
    };
    const closeWithoutFocus = () => onCloseRef.current(false);
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("resize", closeWithoutFocus);
    window.addEventListener("blur", closeWithoutFocus);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("resize", closeWithoutFocus);
      window.removeEventListener("blur", closeWithoutFocus);
    };
  }, [anchor]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      onClose(true);
    }
  };

  return createPortal(
    <div className="emoji-popover" role="dialog" aria-label={t("emoji.dialog")} ref={panel} onKeyDown={onKeyDown}>
      <EmojiPicker recent={recent} language={language} autoFocus onPick={onPick} />
    </div>,
    document.body,
  );
}
