import { useId, type ButtonHTMLAttributes, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { Icon, type IconName } from "../icons/Icon";
import "./controls.css";

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "children"> {
  icon: IconName;
  /** Подпись для диктора и всплывающая подсказка: у кнопки без текста она обязательна. */
  label: string;
  /**
   * Кнопка только нарисована, её функции ещё нет (D1: файл, эмодзи, голосовое — слайсы 7, 3, 7a). Она остаётся в порядке
   * обхода и объявляется как недоступная (`aria-disabled`) с пояснением «Появится позже»; нажатие ничего не делает.
   */
  unavailable?: boolean;
  /** accent — круглая акцентная кнопка («Отправить»); quiet — обычная, тихая. */
  tone?: "quiet" | "accent";
  /** Мелкая кнопка для ряда действий под плашкой. */
  small?: boolean;
}

/** Кнопка со значком. Значок декоративный, имя кнопки — `label`. */
export function IconButton({
  icon,
  label,
  unavailable = false,
  tone = "quiet",
  small = false,
  className,
  type = "button",
  onClick,
  ...rest
}: IconButtonProps) {
  const { t } = useTranslation();
  const noteId = useId();
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (unavailable) event.preventDefault();
    else onClick?.(event);
  };
  const classes = ["icon-button"];
  if (tone === "accent") classes.push("icon-button--accent");
  if (small) classes.push("icon-button--small");
  if (unavailable) classes.push("icon-button--unavailable");
  if (className) classes.push(className);

  return (
    <>
      <button
        type={type}
        className={classes.join(" ")}
        aria-label={label}
        aria-disabled={unavailable ? true : undefined}
        aria-describedby={unavailable ? noteId : undefined}
        title={unavailable ? `${label}. ${t("common.comingLater")}` : label}
        onClick={handleClick}
        {...rest}
      >
        <Icon name={icon} />
      </button>
      {unavailable ? (
        <span className="visually-hidden" id={noteId}>
          {t("common.comingLater")}
        </span>
      ) : null}
    </>
  );
}
