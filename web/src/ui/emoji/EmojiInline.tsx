import { type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { EmojiPicker } from "./EmojiPicker";
import "./emoji.css";

export interface EmojiInlineProps {
  recent: readonly string[];
  language: Language;
  onPick: (emoji: string) => void;
  /** Close the panel; `returnFocus` returns focus to the field (Esc, Tab). */
  onClose: (returnFocus: boolean) => void;
  /** The place in the content flow (above the field row) where the panel stands. */
  host: HTMLElement;
}

/**
 * The "inline" emoji picker (quick capture window): the panel takes the place above the field row and pushes the content apart, and the window
 * adjusts to it (grows up, the bottom edge stays). Unlike the popover panel (`EmojiPopover`) it overlays nothing:
 * the capture window has no room beyond its borders. An outside click does not close it: it is closed by the "emoji" button,
 * by choosing an emoji and by Esc. On opening, focus goes to the search.
 */
export function EmojiInline({ recent, language, onPick, onClose, host }: EmojiInlineProps) {
  const { t } = useTranslation();

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      // Esc closes the picker first, the window (which listens on the document) stays.
      event.stopPropagation();
      onClose(true);
    }
  };

  return createPortal(
    <div className="emoji-inline" role="dialog" aria-label={t("emoji.dialog")} onKeyDown={onKeyDown}>
      <EmojiPicker recent={recent} language={language} autoFocus onPick={onPick} />
    </div>,
    host,
  );
}
