// Calendar date arithmetic without time zones: a date is a `YYYY-MM-DD` string (`journal_date`), all calculations go through
// components via UTC, so the result does not depend on the window's zone or on daylight saving transitions.

import type { Language } from "../../i18n";

export interface Ymd {
  year: number;
  month: number; // 1–12
  day: number;
}

const DAY_MS = 86_400_000;

/** `YYYY-MM-DD` -> components; `null` if the string is not a calendar date ("2026-02-30" is not a date). */
export function parseDate(value: string): Ymd | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const time = new Date(Date.UTC(year, month - 1, day));
  // Date.UTC shifts nonexistent days into the neighboring month: a difference means "no such date".
  if (time.getUTCFullYear() !== year || time.getUTCMonth() !== month - 1 || time.getUTCDate() !== day) return null;
  return { year, month, day };
}

const pad = (value: number, length: number) => String(value).padStart(length, "0");

export function formatDate({ year, month, day }: Ymd): string {
  return `${pad(year, 4)}-${pad(month, 2)}-${pad(day, 2)}`;
}

/** A date as a UTC-midnight instant: the common form for shifts. */
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

/** Shift by months; the day is clamped to the month length (31 January + 1 month = 28 February). */
export function addMonths(date: Ymd, months: number): Ymd {
  const index = date.year * 12 + (date.month - 1) + months;
  const year = Math.floor(index / 12);
  const month = (index % 12 + 12) % 12 + 1;
  return { year, month, day: Math.min(date.day, daysInMonth(year, month)) };
}

/** Day of the week: 0 is Sunday ... 6 is Saturday. */
export function weekday(date: Ymd): number {
  return new Date(toTime(date)).getUTCDay();
}

interface WeekInfoLocale {
  getWeekInfo?: () => { firstDay: number };
  weekInfo?: { firstDay: number };
}

/**
 * The first day of the week: 0 is Sunday, 1 is Monday. Taken from `Intl.Locale` (`getWeekInfo()` or the old property
 * `weekInfo`; `firstDay` there is 1-7, where 7 is Sunday). If the runtime cannot do this, the fallback is:
 * Monday for Russian, Sunday for English.
 */
export function firstWeekday(language: Language): number {
  try {
    const locale = new Intl.Locale(language) as unknown as WeekInfoLocale;
    const info = locale.getWeekInfo?.() ?? locale.weekInfo;
    if (info) return info.firstDay % 7;
  } catch {
    // no Intl.Locale: the fallback is below
  }
  return language === "ru" ? 1 : 0;
}

/** Cells of the month grid: full weeks starting from `firstDay`; days of neighboring months are included (they have `inMonth: false`). */
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

/** The start and end of the week that contains the date. */
export function weekBounds(date: Ymd, firstDay: number): { start: Ymd; end: Ymd } {
  const offset = (weekday(date) - firstDay + 7) % 7;
  const start = addDays(date, -offset);
  return { start, end: addDays(start, 6) };
}

/** Month and year name: "октябрь 2026" / "October 2026" (the Russian " г." after the year is dropped). */
export function formatMonth(year: number, month: number, language: Language): string {
  const parts = new Intl.DateTimeFormat(language, { month: "long", year: "numeric", timeZone: "UTC" }).formatToParts(
    new Date(Date.UTC(year, month - 1, 1)),
  );
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

/** Full date for the screen reader: "вторник, 20 октября 2026 г." / "Tuesday, October 20, 2026". */
export function formatLongDate(date: Ymd, language: Language): string {
  return new Intl.DateTimeFormat(language, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(toTime(date)));
}

/** Weekday name: short for the column header, long for the screen reader. */
export function weekdayName(day: number, language: Language, style: "short" | "long"): string {
  // 2026-10-04 is a Sunday: any known week will do, only the weekday matters.
  return new Intl.DateTimeFormat(language, { weekday: style, timeZone: "UTC" }).format(new Date(Date.UTC(2026, 9, 4 + day)));
}
