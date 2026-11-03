import { useEffect, useId, useRef, useState, type ClipboardEvent, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import { EmojiButton, type ComposerEmoji } from "./EmojiButton";
import "./composer.css";

export type { ComposerEmoji };

export interface ComposerButtons {
  /** "Attach file" (slice 7). */
  attach?: boolean;
  /** "Emoji" (slice 3). */
  emoji?: boolean;
  /** "Record voice message" (slice 7a). */
  voice?: boolean;
}

interface ComposerProps {
  value: string;
  onChange: (value: string) => void;
  /** Enter without Shift (and not inside a code block) submits. D1 has no submitting: the field only shows the look. */
  onSubmit?: () => void;
  /**
   * ⌘Enter (Ctrl+Enter on non-macOS) is "append" (quick capture window, slice 2). Works inside a code block too: it does not
   * insert a line break. Not during IME input; does nothing in an empty field. Without a handler ⌘Enter behaves like Enter.
   */
  onAppend?: () => void;
  /** Which buttons to show; all three by default. In the app a button appears in the slice where its function works. */
  buttons?: ComposerButtons;
  /**
   * Emoji picker. Together with `buttons.emoji` the "emoji" button is live: it opens a panel above itself, the chosen emoji
   * is inserted at the caret position (a selection is replaced), the caret goes after it, the value goes out through `onChange`.
   * Without it the button is only drawn and disabled ("Coming later"), as in D1.
   */
  emoji?: ComposerEmoji;
  /** Buttons are only drawn (D1): announced as disabled ("Coming later") and do nothing. */
  viewOnly?: boolean;
  /** Send error: shown above the field, the draft stays. */
  error?: string;
  /**
   * Floating panel (quick capture window): the field has no fill, border, margins or column width limit: the background, border
   * and rounding are set by the panel around it. The error and the hint stand in a line UNDER the field: when they appear, the window grows down and
   * the field stays in place. The `role="alert"` area is always in the tree (empty, no height), the text arrives in it later, so
   * the screen reader announces the error reliably.
   */
  floating?: boolean;
  /** Hint under the field in the floating panel ("⌘Enter - append"): the field is described by it; while there is an error, there is no hint. */
  hint?: string;
  placeholder?: string;
  /** Focus in the field on display: after sending and when a chat opens. */
  autoFocus?: boolean;
  /** The "Reply to entry" line above the field (slice 5, `ReplyStrip`); main window only. */
  replyStrip?: ReactNode;
  /** The unsent attachments (slice 7, `PendingTiles`): a row above the field, under the reply strip. Main window only. */
  tiles?: ReactNode;
  /** There is at least one tile: the entry can be sent with no text. */
  hasTiles?: boolean;
  /** The paperclip (slice 7): opens the file dialog. Without it the button is only drawn and unavailable. */
  onAttach?: () => void;
  /**
   * Files in the pasted clipboard (slice 7), only when the clipboard has no text: a screenshot or a copied image. They are not
   * pasted into the field; the handler attaches them. A paste with text is the usual text paste.
   */
  onPasteFiles?: (files: readonly File[]) => void;
}

/** Inside a code block (an odd number of ``` fences is open) Enter is a new line, not submit (spec §5). */
export function insideCodeFence(text: string, caret: number): boolean {
  const fences = text.slice(0, caret).match(/^```/gm);
  return (fences?.length ?? 0) % 2 === 1;
}

/**
 * Input field as in Telegram: "file" on the left, an ordinary "Write a note..." field in the middle, "emoji" and "microphone" on the right;
 * when the field has text, the "microphone" is replaced by a round accent "Send" button (as in ChatGPT and Claude).
 * Grows with the text up to 40% of the window height, then scrolls inside (spec §5). We do not show a hint about Enter.
 */
export function Composer({
  value,
  onChange,
  onSubmit,
  onAppend,
  buttons = { attach: true, emoji: true, voice: true },
  emoji,
  viewOnly = false,
  error,
  floating = false,
  hint,
  placeholder,
  autoFocus,
  replyStrip,
  tiles,
  hasTiles = false,
  onAttach,
  onPasteFiles,
}: ComposerProps) {
  const { t } = useTranslation();
  const errorId = useId();
  const hintId = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  const [emojiHost, setEmojiHost] = useState<HTMLDivElement | null>(null);
  const hasText = value.trim() !== "";
  // An entry may be sent with no text if it has attachments.
  const canSend = hasText || hasTiles;

  // Focus in the field on appearance, but only if nobody else has it: the feed with the field is mounted again when the search results
  // close (the query was erased), while the person is still typing in the search field - its focus must not be taken away.
  useEffect(() => {
    if (!autoFocus) return;
    const active = document.activeElement;
    const elsewhere = active && active !== document.body && active !== document.documentElement && active.closest("input, textarea, select, [contenteditable]");
    if (!elsewhere) input.current?.focus();
  }, [autoFocus]);

  // The "Send" button does the same as Enter. After a click focus returns to the field: the button itself disappears as soon as the field is empty.
  const send = () => {
    onSubmit?.();
    input.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    // IME (Russian layout, dictation): Enter confirms the input, it does not submit.
    if (event.nativeEvent.isComposing) return;
    // ⌘Enter / Ctrl+Enter: "append". A line break is inserted nowhere, including inside a code block.
    if (onAppend && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (hasText) onAppend();
      return;
    }
    if (!onSubmit) return;
    if (insideCodeFence(event.currentTarget.value, event.currentTarget.selectionStart)) return;
    event.preventDefault();
    if (canSend) onSubmit();
  };

  // A screenshot or a copied image: the clipboard has files and no text. WKWebView's handling of a Finder file is not verified.
  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    if (!onPasteFiles) return;
    const files = [...event.clipboardData.files];
    if (files.length === 0 || event.clipboardData.getData("text/plain") !== "") return;
    event.preventDefault();
    onPasteFiles(files);
  };

  const errorLine = (
    <p className="composer__error" id={errorId} role="alert">
      {error}
    </p>
  );
  const hintShown = floating && !error && hint;

  return (
    <div className={floating ? "composer composer--floating" : "composer"}>
      {error && !floating ? errorLine : null}
      {replyStrip}
      {tiles}
      {/* The place of the "inline" emoji picker (capture window): empty it has no height, with the panel it pushes the field up. */}
      {floating && emoji?.placement === "inline" ? <div className="composer__emoji-host" ref={setEmojiHost} /> : null}
      <div className={`composer__bar${error ? " composer__bar--invalid" : ""}`}>
        {buttons.attach ? (
          <IconButton icon="attach" label={t("composer.attach")} unavailable={viewOnly || !onAttach} onClick={onAttach} />
        ) : null}
        {/* The copy of the text in ::after sets the field height: it grows without measuring scripts. */}
        <div className="composer__field" data-replicated-value={value}>
          <textarea
            className="composer__input"
            ref={input}
            rows={1}
            value={value}
            placeholder={placeholder ?? t("composer.placeholder")}
            aria-label={t("composer.label")}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? errorId : hintShown ? hintId : undefined}
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={onKeyDown}
            onPaste={onPaste}
          />
        </div>
        {buttons.emoji && emoji ? (
          <EmojiButton {...emoji} field={input} value={value} onChange={onChange} host={emojiHost} />
        ) : buttons.emoji ? (
          <IconButton icon="emoji" label={t("composer.emoji")} unavailable={viewOnly} />
        ) : null}
        {/* The right slot: "Send" for non-blank text, otherwise "microphone". Both buttons are the same size, the width does not jump. */}
        {canSend ? (
          <span className="composer__action" key="send">
            <IconButton icon="send" label={t("composer.send")} tone="accent" unavailable={!onSubmit} onClick={send} />
          </span>
        ) : buttons.voice ? (
          <span className="composer__action" key="voice">
            <IconButton icon="voice" label={t("composer.voice")} unavailable={viewOnly} />
          </span>
        ) : null}
      </div>
      {floating ? (
        <div className="composer__status">
          {errorLine}
          {hintShown ? (
            <p className="composer__hint" id={hintId}>
              {hint}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
