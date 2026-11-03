// Time and day label in the feed (spec §6). The day of an entry is its `journal_date` (Rust computes it by the 04:00 day boundary, §4.1),
// not the calendar day by the window clock: an entry at 00:30 stands under yesterday's label. The time is in the time zone in which
// the entry was created (`created_tz`). Dates and numbers are formatted by `Intl` in the interface language.

import type { Language } from "../../i18n";

/** A pause between messages below which they form a series (spec §6): closer to each other. */
export const SERIES_PAUSE_MS = 5 * 60 * 1000;

const timeFormats = new Map<string, Intl.DateTimeFormat>();

/** Time format for a language and zone; an unknown zone (corrupted data) falls back to the window's time zone, not an error. */
function timeFormat(language: Language, timeZone: string | undefined): Intl.DateTimeFormat {
  const key = `${language}|${timeZone ?? ""}`;
  const cached = timeFormats.get(key);
  if (cached) return cached;
  const options: Intl.DateTimeFormatOptions = { hour: "2-digit", minute: "2-digit", hourCycle: "h23" };
  let format: Intl.DateTimeFormat;
  try {
    format = new Intl.DateTimeFormat(language, { ...options, timeZone });
  } catch {
    format = new Intl.DateTimeFormat(language, options);
  }
  timeFormats.set(key, format);
  return format;
}

/** Time in 24-hour form ("09:12") in the entry's zone; without a zone, in the window's zone. */
export function formatTime(date: Date, language: Language, timeZone?: string): string {
  return timeFormat(language, timeZone).format(date);
}

/** `YYYY-MM-DD` -> number of days since the epoch (calendar arithmetic without time zones); `null` if the string is not a date. */
function dayNumber(journalDate: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(journalDate);
  if (!match) return null;
  const time = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return Number.isNaN(time) ? null : time / 86_400_000;
}

/** The strings for "Today" and "Yesterday" come from the translations. */
export interface DayLabels {
  today: string;
  yesterday: string;
}

/**
 * One day label, without the weekday and without a second line with the same date: "Today", "Yesterday", "14 марта" / "March 14"
 * (current year), "14 марта 2025" / "March 14, 2025" (other years). Compared by `journal_date`; Rust sends "today".
 */
export function formatDayLabel(journalDate: string, todayJournalDate: string, language: Language, labels: DayLabels): string {
  const day = dayNumber(journalDate);
  const today = dayNumber(todayJournalDate);
  if (day === null || today === null) return journalDate;
  if (day === today) return labels.today;
  if (day === today - 1) return labels.yesterday;
  const date = new Date(day * 86_400_000);
  const sameYear = date.getUTCFullYear() === new Date(today * 86_400_000).getUTCFullYear();
  const parts = new Intl.DateTimeFormat(language, {
    day: "numeric",
    month: "long",
    ...(sameYear ? {} : { year: "numeric" }),
    timeZone: "UTC",
  }).formatToParts(date);
  // The Russian format appends " г." to the year; the label in the feed is without it ("14 марта 2025").
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

/**
 * Full date for the "On this day" card and reply quotes: "31 октября 2023" / "October 31, 2023" (always with the year, without " г.").
 * Not a date: the string as is.
 */
export function formatLongDate(journalDate: string, language: Language): string {
  const day = dayNumber(journalDate);
  if (day === null) return journalDate;
  const parts = new Intl.DateTimeFormat(language, { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" }).formatToParts(
    new Date(day * 86_400_000),
  );
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

/** The next message continues the series: the same entry day (`journal_date`) and a pause under 5 minutes. */
export function continuesSeries(
  previous: { createdAt: number; journalDate: string },
  next: { createdAt: number; journalDate: string },
): boolean {
  const pause = next.createdAt - previous.createdAt;
  return previous.journalDate === next.journalDate && pause >= 0 && pause < SERIES_PAUSE_MS;
}
