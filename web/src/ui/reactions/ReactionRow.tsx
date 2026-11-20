import { useTranslation } from "react-i18next";
import type { ReactionView } from "../../api/generated/ReactionView";
import { ReactionChip } from "./ReactionChip";
import "./reactions.css";

interface ReactionRowProps {
  reactions: readonly ReactionView[];
  /** Remove a reaction by its emoji. Without it the chips are plain labels. */
  onRemove?: (emoji: string) => void;
  /** The row is not in a bubble (a trash entry, a search row, the "On this day" card): there is no time at its end to leave room for. */
  outside?: boolean;
}

/**
 * The chips of an entry, left-aligned under its text and wrapping within the bubble. Nothing is drawn for an entry without reactions: it looks
 * exactly as it did. The time of the entry sits at the right end of this row (the bubble puts it there).
 */
export function ReactionRow({ reactions, onRemove, outside = false }: ReactionRowProps) {
  const { t } = useTranslation();
  if (reactions.length === 0) return null;
  return (
    <div className={outside ? "reaction-row reaction-row--card" : "reaction-row"} role="group" aria-label={t("reaction.row")}>
      {reactions.map((reaction) => (
        <ReactionChip key={reaction.emoji} reaction={reaction} onRemove={onRemove ? () => onRemove(reaction.emoji) : undefined} />
      ))}
    </div>
  );
}
