import type { TFunction } from "i18next";
import type { ReactionView } from "../../api/generated/ReactionView";
import type { Language } from "../../i18n";

const DAY_SHIFT_MS = 4 * 3_600_000;

/** The calendar day of the moment `ms` in the zone `timeZone`, with the 04:00 day boundary (the entry day's rule); an unknown zone falls back to the window's. */
function dayKey(ms: number, timeZone: string): string {
  const shifted = new Date(ms - DAY_SHIFT_MS);
  try {
    return new Intl.DateTimeFormat("en-CA", { year: "numeric", month: "2-digit", day: "2-digit", timeZone }).format(shifted);
  } catch {
    return new Intl.DateTimeFormat("en-CA", { year: "numeric", month: "2-digit", day: "2-digit" }).format(shifted);
  }
}

function longDate(ms: number, timeZone: string, language: Language): string {
  const options: Intl.DateTimeFormatOptions = { day: "numeric", month: "long", year: "numeric" };
  let parts: Intl.DateTimeFormatPart[];
  try {
    parts = new Intl.DateTimeFormat(language, { ...options, timeZone }).formatToParts(new Date(ms - DAY_SHIFT_MS));
  } catch {
    parts = new Intl.DateTimeFormat(language, options).formatToParts(new Date(ms - DAY_SHIFT_MS));
  }
  // The Russian format appends " г." to the year; the rest of the interface writes the date without it.
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

function clock(ms: number, timeZone: string, language: Language): string {
  const options: Intl.DateTimeFormatOptions = { hour: "2-digit", minute: "2-digit", hourCycle: "h23" };
  try {
    return new Intl.DateTimeFormat(language, { ...options, timeZone }).format(new Date(ms));
  } catch {
    return new Intl.DateTimeFormat(language, options).format(new Date(ms));
  }
}

/**
 * When a reaction was put, as the tooltip and the screen reader say it: "Put today at 21:05" or "Put on November 19, 2026", and, for a late
 * one, " · 1 year later". The day is the one in the zone the reaction carries; "today" is the journal day of the window clock in that zone.
 */
export function putText(reaction: ReactionView, language: Language, t: TFunction, now: number = Date.now()): string {
  const today = dayKey(reaction.putAt, reaction.putTz) === dayKey(now, reaction.putTz);
  const when = today
    ? t("reaction.putToday", { time: clock(reaction.putAt, reaction.putTz, language) })
    : t("reaction.putOn", { date: longDate(reaction.putAt, reaction.putTz, language) });
  return reaction.yearsLater >= 1 ? `${when} · ${t("reaction.yearsLater", { count: reaction.yearsLater })}` : when;
}
