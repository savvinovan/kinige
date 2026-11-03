import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

interface MessageEditorProps {
  /** The entry's source Markdown: the field's initial text. */
  initial: string;
  /** Enter: non-empty text is saved. */
  onSave: (text: string) => void;
  /** Esc. */
  onCancel: () => void;
}

/**
 * Editing right in the bubble (decision 2 of slice 3): instead of the entry text there is a field with no border and fill, of the same typeface and
 * size, so the bubble changes only its height. Enter saves (empty text and Shift+Enter do not: empty is ignored,
 * Shift+Enter inserts a line break; during IME, Enter confirms the input), Esc cancels. The caret is at the end of the text, focus is in the field.
 *
 * The height is set by an invisible copy of the text under the field (with the time spacer in it, as in a normal bubble): the field grows without
 * measuring. The hint about Enter and Esc adds nothing visually: it is in `title` and for the screen reader.
 */
export function MessageEditor({ initial, onSave, onCancel }: MessageEditorProps) {
  const { t } = useTranslation();
  const hintId = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState(initial);

  useEffect(() => {
    const field = input.current;
    if (!field) return;
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      // Esc belongs to editing: the window and the filters above the feed do not get it.
      event.preventDefault();
      event.stopPropagation();
      onCancel();
      return;
    }
    if (event.key !== "Enter" || event.shiftKey) return;
    // IME (Russian layout, dictation): Enter confirms the input, it does not save.
    if (event.nativeEvent.isComposing) return;
    event.preventDefault();
    if (value.trim() !== "") onSave(value);
  };

  return (
    <div className="message-edit">
      {/* The copy of the text sets the height; the trailing space is needed so that an empty last line is counted too. */}
      <div className="message-edit__replica" aria-hidden="true">
        {value} <span className="message__spacer" />
      </div>
      <textarea
        className="message-edit__input"
        ref={input}
        rows={1}
        value={value}
        aria-label={t("message.editLabel")}
        aria-describedby={hintId}
        title={t("message.editHint")}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <span className="visually-hidden" id={hintId}>
        {t("message.editHint")}
      </span>
    </div>
  );
}
