import { useTranslation } from "react-i18next";
import type { ReactionView } from "../../api/generated/ReactionView";
import { useLanguage } from "../../i18n/I18n";
import { putText } from "./reactionText";
import "./reactions.css";

interface ReactionChipProps {
  reaction: ReactionView;
  /** Remove the reaction (a click on the chip, as in Telegram). Without it the chip is a plain label: the trash, search rows, a read-only entry. */
  onRemove?: () => void;
}

/**
 * One reaction under the text of an entry: a rounded pill with the emoji and, for a reaction put a year or more after the entry, a quiet "N y".
 * The pill is a button that removes the reaction; where nothing can be changed it is a label with no button role. The tooltip and the screen
 * reader label say when it was put ("Put today at 21:05", "Put on November 19, 2026 · 1 year later"), the button's also says what a click does.
 */
export function ReactionChip({ reaction, onRemove }: ReactionChipProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const put = putText(reaction, language, t);
  const content = (
    <>
      <span className="reaction-chip__emoji">{reaction.emoji}</span>
      {reaction.yearsLater >= 1 ? <span className="reaction-chip__age">{t("reaction.yearsShort", { count: reaction.yearsLater })}</span> : null}
    </>
  );
  if (!onRemove) {
    return (
      <span className="reaction-chip" role="img" aria-label={`${reaction.emoji}, ${put}`} title={put}>
        {content}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="reaction-chip reaction-chip--button"
      aria-label={t("reaction.removeLabel", { emoji: reaction.emoji, put })}
      title={put}
      onClick={onRemove}
    >
      {content}
    </button>
  );
}
