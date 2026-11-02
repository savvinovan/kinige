import { useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import type { ForestYear } from "../../api/generated/ForestYear";
import { useLanguage } from "../../i18n/I18n";
import { buildBoard, monthStarts, rowDates } from "./board";
import { DayCard } from "./DayCard";
import { dayAnnouncement, monthNames, weekdayLetters } from "./forestText";
import { useRovingDay } from "./useRovingDay";
import "./forest.css";

interface ForestStripProps {
  table: ForestYear;
  /** The day at the top of the feed: outlined. */
  selected: string | null;
  onOpenDay: (date: string) => void;
}

/** The day cell an event came from, or `null` (a label, the gap between cells). */
const cellOf = (target: EventTarget) => (target as HTMLElement).closest<HTMLElement>("[data-date]");

/** In the strip the weekdays run down: up and down are the previous and next day, left and right the previous and next week. */
const KEYS = { ArrowUp: -1, ArrowDown: 1, ArrowLeft: -7, ArrowRight: 7 };

const ROWS = [0, 1, 2, 3, 4, 5, 6];

interface CardAt {
  day: ForestDay;
  /** Which half of the strip the cell is in, and its distance from that side, to put the card on the side with room. */
  left: boolean;
  offset: number;
}

/**
 * The collapsed forest: a flat weekday x week heat map like GitHub's, one cell per day, tinted by season and denser for fuller
 * days (stages 1..6), month letters on top. A DOM grid with a roving tabindex. Hover and keyboard focus show the day card, a
 * click opens the day in the feed. No zoom or pan; days after today are plain ground.
 */
export function ForestStrip({ table, selected, onOpenDay }: ForestStripProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const board = useMemo(() => buildBoard(table), [table]);
  const { rootRef, tabbable, onKeyDown, onDayFocus } = useRovingDay(board, selected, KEYS, onOpenDay);
  const [card, setCard] = useState<CardAt | null>(null);
  const months = useMemo(() => monthNames(language, "narrow"), [language]);
  const letters = useMemo(() => weekdayLetters(rowDates(board), language), [board, language]);
  const starts = useMemo(() => monthStarts(board), [board]);

  const showCard = (element: HTMLElement) => {
    const date = element.dataset.date;
    const day = date !== undefined ? board.byDate.get(date)?.day : undefined;
    const frame = element.closest<HTMLElement>(".forest-strip")?.getBoundingClientRect();
    if (!day || !frame) return setCard(null);
    const box = element.getBoundingClientRect();
    const x = box.left + box.width / 2 - frame.left;
    setCard({ day, left: x <= frame.width / 2, offset: x <= frame.width / 2 ? x : frame.width - x });
  };

  return (
    <div className="forest-strip" style={{ "--forest-cols": String(table.cols) } as CSSProperties}>
      <div
        className="forest-strip__grid"
        role="grid"
        aria-label={t("forest.strip", { year: table.year })}
        ref={rootRef}
        onKeyDown={onKeyDown}
        onClick={(event) => {
          const date = cellOf(event.target)?.dataset.date;
          if (date && board.byDate.get(date)?.interactive) onOpenDay(date);
        }}
        onPointerOver={(event) => {
          const cell = cellOf(event.target);
          if (cell) showCard(cell);
        }}
        onPointerLeave={() => setCard(null)}
        onFocus={(event) => {
          const cell = cellOf(event.target);
          if (!cell?.dataset.date) return;
          onDayFocus(cell.dataset.date);
          showCard(cell);
        }}
        onBlur={() => setCard(null)}
      >
        <div role="row" className="forest-strip__row forest-strip__row--months">
          {starts.map(({ month, col, date }) => (
            <span role="columnheader" className="forest-strip__month" key={date} style={{ gridColumnStart: col + 2 }}>
              {months[month - 1]}
            </span>
          ))}
        </div>
        {ROWS.map((row) => (
          <div role="row" className="forest-strip__row" key={row}>
            <span role="rowheader" className="forest-strip__weekday">
              {row % 2 === 0 ? letters[row] : ""}
            </span>
            {board.days
              .filter((day) => day.row === row)
              .map((day) =>
                day.date <= table.today ? (
                  <div
                    key={day.date}
                    role="gridcell"
                    className="forest-strip__day"
                    style={{ gridColumnStart: day.col + 2 }}
                    data-date={day.date}
                    data-season={day.season}
                    data-stage={day.stage}
                    data-selected={day.date === selected ? "true" : undefined}
                    aria-selected={day.date === selected}
                    aria-label={dayAnnouncement(t, language, day)}
                    tabIndex={day.date === tabbable ? 0 : -1}
                  />
                ) : (
                  <div key={day.date} className="forest-strip__day forest-strip__day--future" style={{ gridColumnStart: day.col + 2 }} aria-hidden="true" />
                ),
              )}
          </div>
        ))}
      </div>
      {card ? <DayCard day={card.day} style={card.left ? { left: card.offset } : { right: card.offset }} /> : null}
    </div>
  );
}
