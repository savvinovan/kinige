import { useEffect, useLayoutEffect, useRef, type KeyboardEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { PickerTabs, type StickersHost } from "../stickers/PickerTabs";
import { EmojiPicker } from "./EmojiPicker";
import "./emoji.css";

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Moves focus to the next (or, with `back`, the previous) control of the panel, around the ends. Controls out of the Tab order (`tabindex="-1"`) are skipped. */
function cycleFocus(root: HTMLElement, back: boolean) {
  const controls = [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.getAttribute("tabindex") !== "-1");
  const index = controls.indexOf(document.activeElement as HTMLElement);
  const next = back ? (index <= 0 ? controls.length - 1 : index - 1) : index === controls.length - 1 ? 0 : index + 1;
  controls[next]?.focus();
}

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
  /**
   * Slice 5c, the main window only: the panel gets the tabs Emoji, Stickers, GIFs. Without it (the capture window) it is the plain emoji picker.
   * With tabs, Tab walks the controls of the open tab inside the panel instead of closing it.
   */
  stickers?: StickersHost;
}

/**
 * A popover panel with the picker above the "emoji" button of the input field. It overlays the window and shifts nothing. Esc, Tab,
 * an outside click, scrolling, a window resize and leaving the window close it; on opening, focus goes to the search.
 */
export function EmojiPopover({ recent, language, onPick, onClose, anchor, stickers }: EmojiPopoverProps) {
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
      // A pack icon's menu is drawn in its own portal: a click on it is not an outside click.
      if (target instanceof Element && target.closest('[role="menu"]')) return;
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
    if (event.key === "Tab" && stickers && panel.current) {
      // The tabs make the panel a place with several controls: Tab cycles among them and never leaves the panel.
      event.preventDefault();
      event.stopPropagation();
      cycleFocus(panel.current, event.shiftKey);
    } else if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      onClose(true);
    }
  };

  return createPortal(
    <div className="emoji-popover" role="dialog" aria-label={t("emoji.dialog")} ref={panel} onKeyDown={onKeyDown}>
      {stickers ? (
        <PickerTabs recent={recent} language={language} onPick={onPick} onClose={onClose} host={stickers} />
      ) : (
        <EmojiPicker recent={recent} language={language} autoFocus onPick={onPick} />
      )}
    </div>,
    document.body,
  );
}
