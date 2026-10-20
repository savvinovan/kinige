import type { ButtonHTMLAttributes } from "react";
import "./controls.css";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** accent — текстовая кнопка акцентным цветом («Отменить» в тосте); inline — внутри строки текста, цвет наследуется («Повторить»). */
  variant?: "accent" | "inline";
}

/** Текстовая кнопка. Видимая подпись — содержимое; доступное имя можно уточнить через aria-label, начав его с видимого текста. */
export function Button({ variant = "accent", className, type = "button", ...rest }: ButtonProps) {
  return <button type={type} className={`button button--${variant}${className ? ` ${className}` : ""}`} {...rest} />;
}
