import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

interface MessageEditorProps {
  /** Исходный Markdown записи: начальный текст поля. */
  initial: string;
  /** Enter: непустой текст сохраняется. */
  onSave: (text: string) => void;
  /** Esc. */
  onCancel: () => void;
}

/**
 * Правка прямо в плашке (решение 2 слайса 3): вместо текста записи стоит поле без рамки и заливки, той же гарнитуры и
 * размера, так что плашка меняет только высоту. Enter сохраняет (пустой текст и Shift+Enter — нет: пустой игнорируется,
 * Shift+Enter переводит строку; во время IME Enter подтверждает ввод), Esc отменяет. Каретка в конце текста, фокус в поле.
 *
 * Высоту задаёт невидимая копия текста под полем (в ней же распорка под время, как в обычной плашке): поле растёт без
 * измерений. Подсказка про Enter и Esc визуально ничего не добавляет: она в `title` и для диктора.
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
      // Esc принадлежит правке: окно и фильтры над лентой его не получают.
      event.preventDefault();
      event.stopPropagation();
      onCancel();
      return;
    }
    if (event.key !== "Enter" || event.shiftKey) return;
    // IME (русская раскладка, диктовка): Enter подтверждает ввод, а не сохраняет.
    if (event.nativeEvent.isComposing) return;
    event.preventDefault();
    if (value.trim() !== "") onSave(value);
  };

  return (
    <div className="message-edit">
      {/* Копия текста задаёт высоту; пробел в конце нужен, чтобы пустая последняя строка тоже считалась. */}
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
