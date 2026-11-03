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
}

/** A single-line input field (search and the like). No outline: a dark fill, focus is the caret and a lighter fill; a border only for an error. */
export function TextField({ label, icon, error, className, inputRef, ...rest }: TextFieldProps) {
  const errorId = useId();
  return (
    <div className={`text-field${className ? ` ${className}` : ""}`}>
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
      </label>
      {error ? (
        <p className="text-field__error" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
