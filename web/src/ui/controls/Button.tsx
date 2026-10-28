import type { ButtonHTMLAttributes, Ref } from "react";
import "./controls.css";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Ссылка на элемент (React 19 передаёт `ref` обычным свойством): окна переносят фокус на нужную кнопку. */
  ref?: Ref<HTMLButtonElement>;
  /**
   * accent — текстовая кнопка акцентным цветом («Отменить» в тосте); inline — внутри строки текста, цвет наследуется («Повторить»);
   * secondary — кнопка с заливкой и основным цветом текста («Выбрать…» в настройках): действие в форме, а не в потоке.
   */
  variant?: "accent" | "inline" | "secondary";
}

/** Текстовая кнопка. Видимая подпись — содержимое; доступное имя можно уточнить через aria-label, начав его с видимого текста. */
export function Button({ variant = "accent", className, type = "button", ...rest }: ButtonProps) {
  return <button type={type} className={`button button--${variant}${className ? ` ${className}` : ""}`} {...rest} />;
}
