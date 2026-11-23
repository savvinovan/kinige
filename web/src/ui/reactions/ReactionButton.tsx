import type { ButtonHTMLAttributes } from "react";
import { useTranslation } from "react-i18next";
import type { ReactionLimit } from "./reactionLimits";
import "./reactions.css";

interface ReactionButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-pressed" | "children" | "onClick"> {
  emoji: string;
  /** The emoji is a live reaction of the entry: the button is pressed, a click removes it. */
  pressed: boolean;
  /** Why a click would not add it (10 live, or 32 different ever): the button says so and does nothing. */
  refusal: ReactionLimit | null;
  /** The entry is read-only: the button is muted and does nothing. */
  readOnly?: boolean;
  /** `on` is the new state of the reaction. */
  onSwitch?: (on: boolean) => void;
}

/**
 * An emoji button of the hover bar and of the popover: a toggle (`aria-pressed`). A button that cannot add stays in the tab order, is
 * `aria-disabled` and says why in its tooltip and to the screen reader, as the other unavailable buttons do. WebKit exposes `aria-pressed`
 * buttons as checkboxes, which is the same thing to the person.
 */
export function ReactionButton({ emoji, pressed, refusal, readOnly = false, onSwitch, className, title, ...rest }: ReactionButtonProps) {
  const { t } = useTranslation();
  const blocked = readOnly || (!pressed && refusal !== null);
  const reason = readOnly ? t("error.read_only") : refusal === "live" ? t("reaction.limitLive") : refusal === "keys" ? t("reaction.limitKeys") : "";
  const label = t("reaction.button", { emoji });
  return (
    <button
      type="button"
      className={className ? `reaction-button ${className}` : "reaction-button"}
      aria-label={label}
      aria-pressed={pressed}
      aria-disabled={blocked || undefined}
      title={blocked && reason !== "" ? `${label}. ${reason}` : (title ?? label)}
      onClick={() => {
        if (!blocked) onSwitch?.(!pressed);
      }}
      {...rest}
    >
      {emoji}
    </button>
  );
}
