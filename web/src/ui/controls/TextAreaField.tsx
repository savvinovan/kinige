import { useId, type Ref, type TextareaHTMLAttributes } from "react";
import "./controls.css";

interface TextAreaFieldProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "aria-label" | "aria-invalid"> {
  /** The field name for the screen reader. */
  label: string;
  /** The error text: shown under the field, the field is marked invalid. */
  error?: string;
  /** A quiet line under the field on the right (a character counter); read out as the field's description. */
  note?: string;
  inputRef?: Ref<HTMLTextAreaElement>;
}

/** The multi-line variant of `TextField` (KINIG-136: the description of an attachment): the same fill and focus, several lines, and a counter line. */
export function TextAreaField({ label, error, note, className, inputRef, rows = 4, ...rest }: TextAreaFieldProps) {
  const errorId = useId();
  const noteId = useId();
  const described = [error ? errorId : null, note ? noteId : null].filter(Boolean).join(" ");
  return (
    <div className={`text-field${className ? ` ${className}` : ""}`}>
      <label className={`text-field__box text-field__box--multiline${error ? " text-field__box--invalid" : ""}`}>
        <textarea
          ref={inputRef}
          className="text-field__input text-field__input--multiline"
          aria-label={label}
          aria-invalid={error ? true : undefined}
          aria-describedby={described === "" ? undefined : described}
          rows={rows}
          {...rest}
        />
      </label>
      {note ? (
        <p className="text-field__note" id={noteId}>
          {note}
        </p>
      ) : null}
      {error ? (
        <p className="text-field__error" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
