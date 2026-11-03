import type { ButtonHTMLAttributes, Ref } from "react";
import "./controls.css";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** A reference to the element (React 19 passes `ref` as a regular prop): windows move focus to the right button. */
  ref?: Ref<HTMLButtonElement>;
  /**
   * accent is a text button in the accent color ("Undo" in the toast); inline sits inside a line of text, the color is inherited ("Retry");
   * secondary is a button with a fill and the primary text color ("Choose..." in settings): an action in a form, not in the flow.
   */
  variant?: "accent" | "inline" | "secondary";
}

/** A text button. The visible label is the content; the accessible name can be refined via aria-label, starting it with the visible text. */
export function Button({ variant = "accent", className, type = "button", ...rest }: ButtonProps) {
  return <button type={type} className={`button button--${variant}${className ? ` ${className}` : ""}`} {...rest} />;
}
