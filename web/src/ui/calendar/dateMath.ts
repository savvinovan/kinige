// Арифметика календарных дат без часовых поясов: дата — строка `YYYY-MM-DD` (`journal_date`), все расчёты идут по
// компонентам через UTC, поэтому результат не зависит от пояса окна и перехода на летнее время.

import type { Language } from "../../i18n";

export interface Ymd {
  year: number;
  month: number; // 1–12
  day: number;
}

const DAY_MS = 86_400_000;

/** `YYYY-MM-DD` → компоненты; `null`, если строка не календарная дата («2026-02-30» — не дата). */
export function parseDate(value: string): Ymd | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const time = new Date(Date.UTC(year, month - 1, day));
  // Date.UTC сдвигает несуществующие дни в соседний месяц: отличие и значит «такой даты нет».
  if (time.getUTCFullYear() !== year || time.getUTCMonth() !== month - 1 || time.getUTCDate() !== day) return null;
  return { year, month, day };
}

const pad = (value: number, length: number) => String(value).padStart(length, "0");

export function formatDate({ year, month, day }: Ymd): string {
  return `${pad(year, 4)}-${pad(month, 2)}-${pad(day, 2)}`;
}

/** Дата как момент UTC-полуночи: общий вид для сдвигов. */
const toTime = ({ year, month, day }: Ymd) => Date.UTC(year, month - 1, day);

function fromTime(time: number): Ymd {
  const date = new Date(time);
  return { year: date.getUTCFullYear(), month: date.getUTCMonth() + 1, day: date.getUTCDate() };
}

export function addDays(date: Ymd, days: number): Ymd {
  return fromTime(toTime(date) + days * DAY_MS);
}

export function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

/** Сдвиг на месяцы; день ужимается до длины месяца (31 января + 1 месяц = 28 февраля). */
export function addMonths(date: Ymd, months: number): Ymd {
  const index = date.year * 12 + (date.month - 1) + months;
  const year = Math.floor(index / 12);
  const month = (index % 12 + 12) % 12 + 1;
  return { year, month, day: Math.min(date.day, daysInMonth(year, month)) };
}

/** День недели: 0 — воскресенье … 6 — суббота. */
export function weekday(date: Ymd): number {
  return new Date(toTime(date)).getUTCDay();
}

interface WeekInfoLocale {
  getWeekInfo?: () => { firstDay: number };
  weekInfo?: { firstDay: number };
}

/**
 * Первый день недели: 0 — воскресенье, 1 — понедельник. Берётся из `Intl.Locale` (`getWeekInfo()` или старое свойство
 * `weekInfo`; `firstDay` там 1–7, где 7 — воскресенье). Если среда выполнения этого не умеет, запасной вариант:
 * понедельник для русского, воскресенье для английского.
 */
export function firstWeekday(language: Language): number {
  try {
    const locale = new Intl.Locale(language) as unknown as WeekInfoLocale;
    const info = locale.getWeekInfo?.() ?? locale.weekInfo;
    if (info) return info.firstDay % 7;
  } catch {
    // нет Intl.Locale — запасной вариант ниже
  }
  return language === "ru" ? 1 : 0;
}

/** Ячейки сетки месяца: полные недели, начиная с `firstDay`; дни соседних месяцев входят (у них `inMonth: false`). */
export function monthGrid(year: number, month: number, firstDay: number): { date: Ymd; inMonth: boolean }[][] {
  const first: Ymd = { year, month, day: 1 };
  const lead = (weekday(first) - firstDay + 7) % 7;
  const total = Math.ceil((lead + daysInMonth(year, month)) / 7) * 7;
  const start = addDays(first, -lead);
  const weeks: { date: Ymd; inMonth: boolean }[][] = [];
  for (let i = 0; i < total; i++) {
    const date = addDays(start, i);
    (weeks[Math.floor(i / 7)] ??= []).push({ date, inMonth: date.month === month });
  }
  return weeks;
}

/** Начало и конец недели, в которой лежит дата. */
export function weekBounds(date: Ymd, firstDay: number): { start: Ymd; end: Ymd } {
  const offset = (weekday(date) - firstDay + 7) % 7;
  const start = addDays(date, -offset);
  return { start, end: addDays(start, 6) };
}

/** Название месяца и года: «октябрь 2026» / «October 2026» (русское « г.» после года отброшено). */
export function formatMonth(year: number, month: number, language: Language): string {
  const parts = new Intl.DateTimeFormat(language, { month: "long", year: "numeric", timeZone: "UTC" }).formatToParts(
    new Date(Date.UTC(year, month - 1, 1)),
  );
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

/** Полная дата для диктора: «вторник, 20 октября 2026 г.» / «Tuesday, October 20, 2026». */
export function formatLongDate(date: Ymd, language: Language): string {
  return new Intl.DateTimeFormat(language, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(toTime(date)));
}

/** Название дня недели: короткое для заголовка столбца, длинное для диктора. */
export function weekdayName(day: number, language: Language, style: "short" | "long"): string {
  // 2026-10-04 — воскресенье: любая известная неделя подходит, важен только день недели.
  return new Intl.DateTimeFormat(language, { weekday: style, timeZone: "UTC" }).format(new Date(Date.UTC(2026, 9, 4 + day)));
}
