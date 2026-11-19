import { useId, type InputHTMLAttributes, type Ref } from "react";
import { Icon, type IconName } from "../icons/Icon";
import "./controls.css";

interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "aria-label" | "aria-invalid"> {
  /** The field name for the screen reader (there is no visible label: the meaning is clear from the icon and the placeholder). */
  label: string;
  /** The icon on the left inside the field. */
  icon?: IconName;
  /** The error text: shown under the field, the field is marked invalid. */
  error?: string;
  /** A reference to the `<input>` (⌘F puts focus in the field). */
  inputRef?: Ref<HTMLInputElement>;
  /** A fixed text after the input, not editable (KINIG-136: the extension of an attachment's name, `.jpg`). It is read out with the field's description. */
  suffix?: string;
  /** A visible caption above the field (the details dialog). The screen reader still gets `label`; the caption is not read twice. */
  caption?: string;
}

/** A single-line input field (search and the like). No outline: a dark fill, focus is the caret and a lighter fill; a border only for an error. */
export function TextField({ label, icon, error, className, inputRef, suffix, caption, ...rest }: TextFieldProps) {
  const errorId = useId();
  return (
    <div className={`text-field${className ? ` ${className}` : ""}`}>
      {caption ? (
        <span className="text-field__caption" aria-hidden="true">
          {caption}
        </span>
      ) : null}
      <label className={`text-field__box${error ? " text-field__box--invalid" : ""}`}>
        {icon ? <Icon name={icon} /> : null}
        <input
          ref={inputRef}
          className="text-field__input"
          aria-label={label}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          {...rest}
        />
        {suffix ? <span className="text-field__suffix">{suffix}</span> : null}
      </label>
      {error ? (
        <p className="text-field__error" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
