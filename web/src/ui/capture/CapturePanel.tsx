import { useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { summaryOf, summaryText } from "../attachments/summaryText";
import { Composer } from "../composer/Composer";
import { formatTime } from "../feed/dates";
import type { FeedMessage } from "../feed/buildFeed";
import "../message/markdown.css";
import { renderContextMarkdown } from "./contextMarkdown";
import "./capture.css";

export interface CapturePanelProps {
  /** Up to the last three Inbox entries of today, the newest last (closer to the field). */
  context: readonly FeedMessage[];
  /** The entry that ⌘Enter will append text to; while it is in the context, the hint "⌘Enter -- append" stands under the field. `null` means no hint. */
  appendTargetId: string | null;
  value: string;
  onChange: (value: string) => void;
  /** Enter. */
  onSend: () => void;
  /** ⌘Enter (Ctrl+Enter on non-macOS). */
  onAppend: () => void;
  /** Esc anywhere in the window. */
  onHide: () => void;
  /** The already translated error text; shown under the field, the text in the field stays. */
  error?: string;
  /**
   * The content height together with the transparent shadow margin (logical px), on every change. The app fits the
   * OS window height to it; in Storybook stories there is no handler.
   */
  onHeight?: (height: number) => void;
  /**
   * The emoji picker: an "emoji" button in the field, the panel in the flow above the field (the window grows upward). Without it there is no button. `open` and
   * `onOpenChange` are driven by the owner: Esc closes the picker first (`onOpenChange(false)`), and the window (`onHide`) only on the next press.
   */
  emoji?: {
    recent: readonly string[];
    onPicked: (emoji: string) => void;
    open: boolean;
    onOpenChange: (open: boolean) => void;
  };
}

/**
 * The content of the quick capture window (slice 2): a compact floating panel in the spirit of Gemini and ChatGPT, a rounded "pill"
 * with a thin light outline and a soft shadow. Inside, at the top, up to three pale context bubbles (time, Markdown rendered, long ones
 * clipped by lines, links not clickable), below them the input field row with the "Send" button inside, under the field the hint
 * "⌘Enter -- append" or an error. With no entries today, only the field. The window is transparent and has no system frame: the shape and shadow
 * are drawn by CSS, with a shadow margin around the panel. No toasts or confirmations.
 *
 * Content does not jump: the hint and the error stand in a row under the field, the window fits the content with its top in place, so
 * when they appear it grows downward and the field stays where it was. While the field holds long text, the context shrinks from the top: old
 * bubbles go beyond the edge.
 *
 * The window is created hidden and shown many times, so autofocus on mount is not enough: every time the window gets focus
 * the caret is put back into the field. Esc hides the window wherever focus is (except IME input).
 */
export function CapturePanel({ context, appendTargetId, value, onChange, onSend, onAppend, onHide, error, onHeight, emoji }: CapturePanelProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const root = useRef<HTMLDivElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  // Fresh handlers without resubscribing: the parent may pass new functions on every render.
  const onHideRef = useRef(onHide);
  const onHeightRef = useRef(onHeight);
  const emojiRef = useRef(emoji);
  useEffect(() => {
    onHideRef.current = onHide;
    onHeightRef.current = onHeight;
    emojiRef.current = emoji;
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing) return;
      event.preventDefault();
      // The picker is open (focus is in the field, not in the picker: Esc from the picker does not reach here): it closes first, the window stays.
      const picker = emojiRef.current;
      if (picker?.open) {
        picker.onOpenChange(false);
        root.current?.querySelector("textarea")?.focus();
        return;
      }
      onHideRef.current();
    };
    const focusField = () => root.current?.querySelector("textarea")?.focus();
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("focus", focusField);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("focus", focusField);
    };
  }, []);

  // The content height: reported on the first frame and on every change (text, error, hint, text size, context).
  useEffect(() => {
    const element = shell.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    let last = 0;
    const report = () => {
      const height = Math.ceil(element.getBoundingClientRect().height);
      if (height <= 0 || height === last) return;
      last = height;
      onHeightRef.current?.(height);
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, []);

  const taskDone = t("message.taskDone");
  const taskTodo = t("message.taskTodo");
  const entries = useMemo(
    () => context.map((entry) => ({ entry, html: renderContextMarkdown(entry.bodyMd, { done: taskDone, todo: taskTodo }) })),
    [context, taskDone, taskTodo],
  );
  const hintShown = appendTargetId !== null && context.some((entry) => entry.id === appendTargetId);

  return (
    <div className="capture-shell" ref={shell}>
      <div className="capture-panel" role="group" aria-label={t("capture.label")} ref={root}>
        {entries.length > 0 ? (
          <div className="capture-context">
            <ul className="capture-context__list" aria-label={t("capture.context")}>
              {entries.map(({ entry, html }) => (
                <li className="capture-context__item" key={entry.id}>
                  <article className="capture-entry">
                    <time className="capture-entry__time" dateTime={new Date(entry.createdAt).toISOString()}>
                      {formatTime(new Date(entry.createdAt), language, entry.createdTz)}
                    </time>
                    {entry.bodyMd.trim() === "" && (entry.attachments?.length ?? 0) > 0 ? (
                      // An entry with no text (a photo sent alone) is described by what it holds (slice 7).
                      <div className="markdown capture-entry__text">{summaryText(summaryOf(entry.attachments ?? []), t)}</div>
                    ) : (
                      <div className="markdown capture-entry__text" dangerouslySetInnerHTML={{ __html: html }} />
                    )}
                  </article>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        <Composer
          value={value}
          onChange={onChange}
          onSubmit={onSend}
          onAppend={onAppend}
          buttons={emoji ? { emoji: true } : {}}
          emoji={
            emoji
              ? {
                  recent: emoji.recent,
                  language,
                  onPicked: emoji.onPicked,
                  placement: "inline",
                  open: emoji.open,
                  onOpenChange: emoji.onOpenChange,
                }
              : undefined
          }
          error={error}
          floating
          hint={hintShown ? t("capture.appendHint") : undefined}
          autoFocus
        />
      </div>
    </div>
  );
}
