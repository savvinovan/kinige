import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { IconButton } from "../controls/IconButton";
import { EmojiInline } from "../emoji/EmojiInline";
import { EmojiPopover } from "../emoji/EmojiPopover";

/** Emoji picker in the input field (slice 3). */
export interface ComposerEmoji {
  /** Recently chosen emoji, newest first. */
  recent: readonly string[];
  language: Language;
  /** An emoji was chosen and inserted: remember it in "recent". */
  onPicked?: (emoji: string) => void;
  /**
   * Where to show the picker. `popover` (default) is a panel above the button on top of the window. `inline` is a panel in the flow above the
   * field row (quick capture window: the window grows up together with it); works only in the floating field.
   */
  placement?: "popover" | "inline";
  /** Whether the picker is open, if the owner controls it (capture window: Esc closes the picker first). Without it the state is inside the button. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

interface EmojiButtonProps extends ComposerEmoji {
  /** The input field: the emoji is inserted at its caret position. */
  field: RefObject<HTMLTextAreaElement | null>;
  value: string;
  onChange: (value: string) => void;
  /** The place for the `inline` panel; until it exists, the panel is not shown. */
  host?: HTMLElement | null;
}

/**
 * The "emoji" button of the input field: opens the picker panel above itself. The choice inserts the emoji instead of the selection (or at the caret
 * position), the caret goes after it, focus returns to the field, the panel closes.
 */
export function EmojiButton({ recent, language, onPicked, placement = "popover", open: controlled, onOpenChange, field, value, onChange, host }: EmojiButtonProps) {
  const { t } = useTranslation();
  const [inner, setInner] = useState(false);
  const open = controlled ?? inner;
  const setOpen = (next: boolean) => {
    setInner(next);
    onOpenChange?.(next);
  };
  const anchor = useRef<HTMLSpanElement>(null);
  // The caret can be placed only when the parent has already put the new value into the field.
  const caret = useRef<{ value: string; position: number } | null>(null);

  useLayoutEffect(() => {
    const pending = caret.current;
    if (pending?.value === value) field.current?.setSelectionRange(pending.position, pending.position);
    caret.current = null;
  }, [value, field]);

  const pick = (emoji: string) => {
    const input = field.current;
    // The textarea selection survives focus moving to the picker; without the field (it is not in the tree) the emoji is appended to the end.
    const start = input?.selectionStart ?? value.length;
    const end = input?.selectionEnd ?? start;
    const next = value.slice(0, start) + emoji + value.slice(end);
    caret.current = { value: next, position: start + emoji.length };
    onChange(next);
    setOpen(false);
    input?.focus();
    onPicked?.(emoji);
  };

  const close = (returnFocus: boolean) => {
    setOpen(false);
    if (returnFocus) field.current?.focus();
  };

  return (
    <span className="composer__emoji" ref={anchor}>
      <IconButton
        icon="emoji"
        label={t("composer.emoji")}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      />
      {open && placement === "inline" && host ? (
        <EmojiInline recent={recent} language={language} host={host} onPick={pick} onClose={close} />
      ) : null}
      {open && placement === "popover" ? (
        <EmojiPopover recent={recent} language={language} anchor={anchor} onPick={pick} onClose={close} />
      ) : null}
    </span>
  );
}
