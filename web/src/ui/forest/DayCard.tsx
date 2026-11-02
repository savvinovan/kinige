import type { CSSProperties, Ref } from "react";
import { useTranslation } from "react-i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import { useLanguage } from "../../i18n/I18n";
import { daySummary, longDay } from "./forestText";
import "./forest.css";

interface DayCardProps {
  day: ForestDay;
  /** Position inside the pane (the board puts it at the pointer or in a corner). */
  style?: CSSProperties;
  ref?: Ref<HTMLDivElement>;
  /** The card of the selected day, not of the day under the pointer. */
  pinned?: boolean;
}

/**
 * The card of a day: "Fri, October 16, 2026", "5 notes · a large tree" and the start of the day's first entry, up to two lines.
 * Only what exists (PRD §7.4): no photos line until attachments exist. It is a plain surface, no border or shadow; for screen
 * readers the board announces the day itself, so the card is hidden from them.
 */
export function DayCard({ day, style, ref, pinned = false }: DayCardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  return (
    <div className={pinned ? "day-card day-card--pinned" : "day-card"} style={style} ref={ref} aria-hidden="true">
      <p className="day-card__date">{longDay(day.date, language)}</p>
      <p className="day-card__summary">{daySummary(t, day)}</p>
      {day.excerpt ? <p className="day-card__excerpt">{day.excerpt}</p> : null}
    </div>
  );
}
