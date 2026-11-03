import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../api/generated/Language";
import { Button } from "../controls/Button";
import { IconButton } from "../controls/IconButton";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import {
  addDays,
  addMonths,
  firstWeekday,
  formatDate,
  formatLongDate,
  formatMonth,
  monthGrid,
  parseDate,
  weekBounds,
  weekdayName,
  type Ymd,
} from "./dateMath";
import "./calendar.css";

interface CalendarDialogProps {
  /** The entry's current date (`YYYY-MM-DD`): the selected day, the calendar opens on its month. */
  value: string;
  /** Today's `journal_date` (`YYYY-MM-DD`): Rust sends it, the window clock is not used. */
  today: string;
  language: Language;
  /** An already translated error (for example "No such date"): `role="alert"` under the grid. */
  error?: string;
  /** The date is being saved: picking a day does nothing meanwhile. */
  busy?: boolean;
  /** Picking a day (click, Enter, space) or "Today": immediately, without a confirm button. */
  onPick: (date: string) => void;
  onClose: () => void;
  /** The window title; by default "Change date" (the entry's date). Search sets its own. */
  title?: string;
}

/** If both `value` and `today` are corrupted, the first calendar date: the grid is still drawn. */
const FALLBACK: Ymd = { year: 1970, month: 1, day: 1 };

/**
 * A calendar for picking an entry's date in a modal window. Its own month grid built from tokens: the system `<input type="date">` is not used,
 * its look is not controlled by tokens. Future dates are allowed; days of neighboring months are muted and also selectable.
 * The week starts on Monday in Russian and on Sunday in English (`Intl.Locale`), dates are computed by components (UTC).
 *
 * Keyboard (the grid is `role="grid"`, one Tab stop): arrows move by day and week, Home / End go to the start and end of the week,
 * PageUp / PageDown move by month (with Shift, by year), Enter and space select, Esc closes (Modal).
 */
export function CalendarDialog({ value, today, language, error, busy = false, onPick, onClose, title }: CalendarDialogProps) {
  const { t } = useTranslation();
  const titleId = useId();
  const selected = parseDate(value);
  const todayYmd = parseDate(today);
  const [cursor, setCursor] = useState<Ymd>(selected ?? todayYmd ?? FALLBACK);
  const table = useRef<HTMLTableElement>(null);
  // Focus moves to the cursor day after a re-render, but only if the keyboard moved the cursor.
  const moveFocus = useRef(false);
  const first = firstWeekday(language);
  const weeks = monthGrid(cursor.year, cursor.month, first);
  const cursorDate = formatDate(cursor);

  const dayButton = (date: string) => table.current?.querySelector<HTMLButtonElement>(`button[data-date="${date}"]`);

  useEffect(() => {
    if (!moveFocus.current) return;
    moveFocus.current = false;
    dayButton(cursorDate)?.focus();
  });

  // On open, focus goes to the selected day. Modal puts it on the close button in its own effect, which runs after the effects
  // of its descendants, so the move is deferred to a microtask.
  useEffect(() => {
    queueMicrotask(() => dayButton(cursorDate)?.focus());
    // Only on open: after that the cursor drives focus.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const go = (next: Ymd) => {
    moveFocus.current = true;
    setCursor(next);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    const { start, end } = weekBounds(cursor, first);
    const next: Record<string, Ymd> = {
      ArrowLeft: addDays(cursor, -1),
      ArrowRight: addDays(cursor, 1),
      ArrowUp: addDays(cursor, -7),
      ArrowDown: addDays(cursor, 7),
      Home: start,
      End: end,
      PageUp: addMonths(cursor, event.shiftKey ? -12 : -1),
      PageDown: addMonths(cursor, event.shiftKey ? 12 : 1),
    };
    const target = next[event.key];
    if (!target) return;
    event.preventDefault();
    go(target);
  };

  const pick = (date: string) => {
    if (!busy) onPick(date);
  };

  return (
    <Modal title={title ?? t("calendar.title")} closeLabel={t("calendar.close")} onClose={onClose}>
      <div className="calendar">
        <div className="calendar__header">
          <IconButton
            icon="chevronRight"
            className="calendar__prev"
            label={t("calendar.prevMonth")}
            onClick={() => setCursor(addMonths(cursor, -1))}
          />
          <h3 className="calendar__title" id={titleId} aria-live="polite">
            {formatMonth(cursor.year, cursor.month, language)}
          </h3>
          <IconButton icon="chevronRight" label={t("calendar.nextMonth")} onClick={() => setCursor(addMonths(cursor, 1))} />
        </div>
        <table className="calendar__grid" role="grid" aria-labelledby={titleId} aria-busy={busy || undefined} ref={table} onKeyDown={onKeyDown}>
          <thead>
            <tr>
              {Array.from({ length: 7 }, (_, i) => {
                const day = (first + i) % 7;
                return (
                  <th className="calendar__weekday" scope="col" key={day} abbr={weekdayName(day, language, "long")}>
                    {weekdayName(day, language, "short")}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {weeks.map((week) => (
              <tr key={formatDate(week[0]?.date ?? cursor)}>
                {week.map(({ date, inMonth }) => {
                  const iso = formatDate(date);
                  const classes = ["calendar__day"];
                  if (!inMonth) classes.push("calendar__day--outside");
                  if (iso === today) classes.push("calendar__day--today");
                  return (
                    <td role="gridcell" aria-selected={iso === value} key={iso}>
                      <button
                        type="button"
                        className={classes.join(" ")}
                        data-date={iso}
                        tabIndex={iso === cursorDate ? 0 : -1}
                        aria-label={formatLongDate(date, language)}
                        aria-current={iso === today ? "date" : undefined}
                        onFocus={() => setCursor(date)}
                        onClick={() => pick(iso)}
                      >
                        {date.day}
                      </button>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {error}
          </StatusNote>
        ) : null}
        <div className="calendar__footer">
          <Button variant="secondary" aria-disabled={busy || undefined} onClick={() => pick(today)}>
            {t("calendar.today")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
