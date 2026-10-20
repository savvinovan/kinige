import type { ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "../icons/Icon";
import "./controls.css";

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "children"> {
  icon: IconName;
  /** Подпись для диктора и всплывающая подсказка: у кнопки без текста она обязательна. */
  label: string;
}

/** Кнопка со значком. Значок декоративный, имя кнопки — `label`. */
export function IconButton({ icon, label, className, type = "button", ...rest }: IconButtonProps) {
  return (
    <button
      type={type}
      className={`icon-button${className ? ` ${className}` : ""}`}
      aria-label={label}
      title={label}
      {...rest}
    >
      <Icon name={icon} />
    </button>
  );
}
