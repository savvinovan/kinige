import { useId, type ButtonHTMLAttributes, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { Icon, type IconName } from "../icons/Icon";
import "./controls.css";

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "children"> {
  icon: IconName;
  /** The label for the screen reader and the tooltip: required for a button without text. */
  label: string;
  /**
   * The button is only drawn, its function does not exist yet (D1: file, emoji, voice: slices 7, 3, 7a). It stays in the
   * tab order and is announced as unavailable (`aria-disabled`) with the explanation "Coming later"; pressing it does nothing.
   */
  unavailable?: boolean;
  /** accent is a round accent button ("Send"); quiet is the regular, quiet one. */
  tone?: "quiet" | "accent";
  /** A small button for the actions row under a bubble. */
  small?: boolean;
}

/** A button with an icon. The icon is decorative, the button name is `label`. */
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
