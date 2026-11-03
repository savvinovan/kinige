import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import type { ForestYear } from "../../api/generated/ForestYear";
import { useLanguage } from "../../i18n/I18n";
import { buildBoard, rowDates } from "./board";
import { dayAnnouncement, monthNames, weekdayLetters } from "./forestText";
import { useRovingDay } from "./useRovingDay";
import "./forest.css";
import "./forest-calendar.css";

interface ForestCalendarProps {
  table: ForestYear;
  /** The day at the top of the feed: marked. */
  selected: string | null;
  onOpenDay: (date: string) => void;
}

/** As on the board: left and right are the previous and next day, up and down the previous and next week. */
const KEYS = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };

/**
 * The accessible alternative to the board (PRD §7.4): twelve ordinary month grids with the same data. A day is a button read as
 * "October 16, 5 notes, a large tree" with a dot sized by the stage; a click does what a click on a tree does. Days after today
 * are plain numbers.
 */
export function ForestCalendar({ table, selected, onOpenDay }: ForestCalendarProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const board = useMemo(() => buildBoard(table), [table]);
  const { rootRef, tabbable, onKeyDown, onDayFocus } = useRovingDay(board, selected, KEYS, onOpenDay);
  const names = useMemo(() => monthNames(language, "long"), [language]);
  const letters = useMemo(() => weekdayLetters(rowDates(board), language), [board, language]);
  const months = useMemo(() => {
    const byMonth: ForestDay[][] = Array.from({ length: 12 }, () => []);
    for (const day of table.days) byMonth[Number(day.date.slice(5, 7)) - 1]?.push(day);
    return byMonth;
  }, [table]);

  return (
    <div
      className="forest-calendar"
      role="group"
      aria-label={t("forest.calendar", { year: table.year })}
      ref={rootRef}
      onKeyDown={onKeyDown}
    >
      {months.map((days, index) => (
        <section className="forest-calendar__month" key={names[index] ?? index} aria-label={names[index]}>
          <h3 className="forest-calendar__title">{names[index]}</h3>
          <div className="forest-calendar__days">
            {letters.map((letter, row) => (
              <span className="forest-calendar__weekday" key={row} aria-hidden="true">
                {letter}
              </span>
            ))}
            {days.map((day, n) => {
              // The first day of the month goes under its weekday (its row on the board); the rest flow after it.
              const place = n === 0 ? { gridColumnStart: day.row + 1 } : undefined;
              const number = Number(day.date.slice(8));
              if (day.date > table.today) {
                return (
                  <span className="forest-calendar__day forest-calendar__day--future" key={day.date} style={place} aria-hidden="true">
                    {number}
                  </span>
                );
              }
              return (
                <button
                  type="button"
                  className="forest-calendar__day"
                  key={day.date}
                  style={place}
                  data-date={day.date}
                  data-season={day.season}
                  data-stage={day.stage}
                  data-selected={day.date === selected ? "true" : undefined}
                  aria-label={dayAnnouncement(t, language, day)}
                  aria-current={day.date === selected ? "date" : undefined}
                  tabIndex={day.date === tabbable ? 0 : -1}
                  onClick={() => onOpenDay(day.date)}
                  onFocus={() => onDayFocus(day.date)}
                >
                  <span className="forest-calendar__number" aria-hidden="true">
                    {number}
                  </span>
                  <span className="forest-calendar__dot" aria-hidden="true" />
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}
