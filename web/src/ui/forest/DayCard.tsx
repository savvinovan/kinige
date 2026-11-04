import type { CSSProperties, Ref } from "react";
import { useTranslation } from "react-i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import { useLanguage } from "../../i18n/I18n";
import { summaryText } from "../attachments/summaryText";
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
 * The card of a day: "Fri, October 16, 2026", "5 notes · 3 photos · a large tree" and the start of the day's first entry, up to two
 * lines (a summary such as "Photo" when that entry has no text). The photos part is there only when the day has photos (PRD §7.4).
 * It is a plain surface, no border or shadow. The card
 * that follows the pointer is hidden from screen readers (the board announces the day itself on the keyboard); the pinned
 * caption of the selected day is plain text under the board and is read, because the selection also follows the feed and
 * nothing else says which day that is.
 */
export function DayCard({ day, style, ref, pinned = false }: DayCardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  return (
    <div className={pinned ? "day-card day-card--pinned" : "day-card"} style={style} ref={ref} aria-hidden={pinned ? undefined : "true"}>
      <p className="day-card__date">{longDay(day.date, language)}</p>
      <p className="day-card__summary">{daySummary(t, day)}</p>
      {day.excerpt ? (
        <p className="day-card__excerpt">{day.excerpt}</p>
      ) : day.firstSummary ? (
        // The first entry of the day has no text (a photo sent alone): it is described by what it holds.
        <p className="day-card__excerpt">{summaryText(day.firstSummary, t)}</p>
      ) : null}
    </div>
  );
}
