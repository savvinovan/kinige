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
  /** Текущая дата записи (`YYYY-MM-DD`): выбранный день, календарь открывается на её месяце. */
  value: string;
  /** Сегодняшний `journal_date` (`YYYY-MM-DD`): его присылает Rust, часы окна не используются. */
  today: string;
  language: Language;
  /** Уже переведённая ошибка (например «Такой даты нет»): `role="alert"` под сеткой. */
  error?: string;
  /** Дата сохраняется: выбор дня на это время не срабатывает. */
  busy?: boolean;
  /** Выбор дня (щелчок, Enter, пробел) или «Сегодня»: сразу, без кнопки подтверждения. */
  onPick: (date: string) => void;
  onClose: () => void;
  /** Заголовок окна; по умолчанию «Изменить дату» (дата записи). Поиск ставит свой. */
  title?: string;
}

/** Если и `value`, и `today` испорчены — первая календарная дата: сетка всё равно рисуется. */
const FALLBACK: Ymd = { year: 1970, month: 1, day: 1 };

/**
 * Календарь выбора даты записи в модальном окне. Своя сетка месяца из токенов: системный `<input type="date">` не берётся,
 * его вид токенами не управляется. Будущие даты разрешены; дни соседних месяцев приглушены и тоже выбираются.
 * Неделя начинается с понедельника в русском и с воскресенья в английском (`Intl.Locale`), даты считаются по компонентам (UTC).
 *
 * Клавиатура (сетка `role="grid"`, одна остановка Tab): стрелки — день и неделя, Home / End — начало и конец недели,
 * PageUp / PageDown — месяц (с Shift — год), Enter и пробел выбирают, Esc закрывает (Modal).
 */
export function CalendarDialog({ value, today, language, error, busy = false, onPick, onClose, title }: CalendarDialogProps) {
  const { t } = useTranslation();
  const titleId = useId();
  const selected = parseDate(value);
  const todayYmd = parseDate(today);
  const [cursor, setCursor] = useState<Ymd>(selected ?? todayYmd ?? FALLBACK);
  const table = useRef<HTMLTableElement>(null);
  // Фокус переносится на день курсора после перерисовки, но только если курсор сдвинула клавиатура.
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

  // При открытии фокус на выбранном дне. Modal ставит его на кнопку закрытия в своём эффекте, который идёт после эффектов
  // потомков, поэтому перенос откладывается на микрозадачу.
  useEffect(() => {
    queueMicrotask(() => dayButton(cursorDate)?.focus());
    // Только при открытии: дальше фокус ведёт курсор.
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
