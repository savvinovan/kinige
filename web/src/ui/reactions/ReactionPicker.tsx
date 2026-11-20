import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { EmojiPicker } from "../emoji/EmojiPicker";
import { Button } from "../controls/Button";
import { isLive, putRefusal, type ReactionState } from "./reactionLimits";
import { ReactionButton } from "./ReactionButton";
import "./reactions.css";

/** The control a panel stands at: its rectangle in window coordinates (a point for a menu position). */
export interface PickerAnchor {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface ReactionPickerProps {
  anchor: PickerAnchor;
  /** `quick` - the three quick emoji and "More"; `full` - the emoji picker, with the recent reactions on top. */
  start: "quick" | "full";
  quick: readonly string[];
  recent: readonly string[];
  /** The entry's reactions and the keys it has had: the buttons that would add a new emoji to a full entry say so. */
  state: ReactionState;
  language: Language;
  /** A quick emoji toggles (`on` is the new state); a pick from the picker always puts. */
  onReact: (emoji: string, on: boolean) => void;
  /**
   * Close the panel. `returnFocus` says whether focus goes back to the control that opened it (Esc, Tab, a choice): a click outside does not
   * return it, the person has already chosen another place. The owner finds that control when this is called, not when the panel opened.
   */
  onClose: (returnFocus: boolean) => void;
}

const GAP = 4;
const MARGIN = 8;

/**
 * The panel for choosing a reaction: over the window, shifting nothing. In `quick` mode it is the three quick emoji and "More", which turns the same
 * panel into the full emoji picker; a pick puts the reaction and closes it. It stands below its anchor, or above it when there is no room, and
 * never leaves the window. Esc and Tab close it with focus returned; a click outside, a resize and leaving the window close it without.
 */
export function ReactionPicker({ anchor, start, quick, recent, state, language, onReact, onClose }: ReactionPickerProps) {
  const { t } = useTranslation();
  const panel = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState(start);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // Before the first paint: below the anchor, flipped above when there is no room, kept inside the window on the right.
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    const below = anchor.bottom + GAP;
    const top = below + height <= window.innerHeight - MARGIN ? below : Math.max(MARGIN, anchor.top - GAP - height);
    const left = Math.max(MARGIN, Math.min(anchor.left, window.innerWidth - width - MARGIN));
    element.style.top = `${top}px`;
    element.style.left = `${left}px`;
  }, [anchor, mode]);

  // The first quick button takes focus (the search field of the picker does it itself).
  useEffect(() => {
    if (mode === "quick") panel.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [mode]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && panel.current?.contains(event.target)) return;
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
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      onClose(true);
      return;
    }
    if (mode !== "quick" || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    event.preventDefault();
    const buttons = [...(panel.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[(at + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
  };

  const choose = (emoji: string, on: boolean) => {
    onReact(emoji, on);
    onClose(true);
  };

  return createPortal(
    <div
      className={`reaction-picker${mode === "full" ? " reaction-picker--full" : ""}`}
      role="dialog"
      aria-label={t("reaction.dialog")}
      ref={panel}
      onKeyDown={onKeyDown}
    >
      {mode === "quick" ? (
        <div className="reaction-picker__quick">
          {quick.map((emoji) => (
            <ReactionButton
              key={emoji}
              emoji={emoji}
              pressed={isLive(state, emoji)}
              refusal={putRefusal(state, emoji)}
              onSwitch={(on) => choose(emoji, on)}
            />
          ))}
          <Button variant="inline" className="reaction-picker__more" aria-label={t("reaction.moreLabel")} onClick={() => setMode("full")}>
            {t("reaction.more")}
          </Button>
        </div>
      ) : (
        <EmojiPicker recent={recent} language={language} autoFocus onPick={(emoji) => choose(emoji, true)} />
      )}
    </div>,
    document.body,
  );
}
