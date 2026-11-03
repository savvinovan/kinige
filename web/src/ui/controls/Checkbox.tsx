import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import "./controls.css";

interface CheckboxProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "children"> {
  /** The visible label to the right of the square; it also gives the checkbox its name for the screen reader. */
  children: ReactNode;
  /** A second line in a quiet color under the label (how many entries, how many already exist). */
  description?: ReactNode;
}

/**
 * A checkbox: a real `<input type="checkbox">` (space, focus and the screen reader work on their own), drawn as the task square from the feed:
 * a gray border, and when checked an accent fill with a tick. The focus ring appears only from the keyboard. The label is cut
 * with an ellipsis if it does not fit.
 */
export function Checkbox({ children, description, className, ...rest }: CheckboxProps) {
  const descriptionId = useId();
  const labelId = useId();
  return (
    <label className={`checkbox${className ? ` ${className}` : ""}`}>
      <input
        type="checkbox"
        className="checkbox__input"
        aria-labelledby={labelId}
        aria-describedby={description ? descriptionId : undefined}
        {...rest}
      />
      <span className="checkbox__box" aria-hidden="true" />
      <span className="checkbox__text">
        <span className="checkbox__label" id={labelId}>
          {children}
        </span>
        {description ? (
          <span className="checkbox__description" id={descriptionId}>
            {description}
          </span>
        ) : null}
      </span>
    </label>
  );
}
