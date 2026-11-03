import type { TFunction } from "i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import type { Language } from "../../i18n";

// Words of the forest: dates, "5 notes · a large tree" and the sentence a screen reader announces. Dates and plurals come from
// `Intl` and i18next in the interface language.

/** `YYYY-MM-DD` at noon UTC, so a format in UTC shows that very day. */
function at(date: string): Date {
  const [year, month, day] = date.split("-").map(Number);
  return new Date(Date.UTC(year ?? 1970, (month ?? 1) - 1, day ?? 1));
}

/** Russian dates end with " г."; the card and the labels drop it, like the feed does. */
function withoutYearSuffix(text: string): string {
  return text.replace(/\s*г\.?$/, "");
}

/** "Fri, October 16, 2026" / "пт, 16 октября 2026". */
export function longDay(date: string, language: Language): string {
  return withoutYearSuffix(new Intl.DateTimeFormat(language, { weekday: "short", day: "numeric", month: "long", year: "numeric", timeZone: "UTC" }).format(at(date)));
}

/** "October 16" / "16 октября": for screen readers, the year is the label of the board. */
export function shortDay(date: string, language: Language): string {
  return new Intl.DateTimeFormat(language, { day: "numeric", month: "long", timeZone: "UTC" }).format(at(date));
}

/** "Oct" / "окт." for each month, January first. */
export function monthNames(language: Language, style: "narrow" | "short" | "long" = "short"): string[] {
  const format = new Intl.DateTimeFormat(language, { month: style, timeZone: "UTC" });
  return Array.from({ length: 12 }, (_, month) => format.format(new Date(Date.UTC(2001, month, 1))));
}

/** One narrow letter for the weekday of a date ("M", "Пн"-style letters are the locale's choice). */
export function weekdayLetter(date: string, language: Language): string {
  return new Intl.DateTimeFormat(language, { weekday: "narrow", timeZone: "UTC" }).format(at(date));
}

/** The weekday names of a week that starts on the weekday of `firstDate`, narrow. */
export function weekdayLetters(rowDates: readonly (string | null)[], language: Language): string[] {
  return rowDates.map((date) => (date === null ? "" : weekdayLetter(date, language)));
}

/** "5 notes" / "5 записей". */
export function notesText(t: TFunction, notes: number): string {
  return t("forest.notes", { count: notes });
}

export function stageName(t: TFunction, stage: number): string {
  const clamped = Number.isFinite(stage) ? Math.min(6, Math.max(0, Math.trunc(stage))) : 0;
  return t(`forest.stage${clamped}` as "forest.stage0");
}

/** "3 photos" / "3 фотографии": the day's live photos (slice 7). */
export function photosText(t: TFunction, photos: number): string {
  return t("forest.photos", { count: photos });
}

/** "5 notes · 3 photos · a large tree" (the photos only when there are some), or "No notes" for an empty day. */
export function daySummary(t: TFunction, day: Pick<ForestDay, "notes" | "stage"> & { photos?: number }): string {
  if (day.notes === 0) return t("forest.noNotes");
  const photos = (day.photos ?? 0) > 0 ? ` · ${photosText(t, day.photos ?? 0)}` : "";
  return `${notesText(t, day.notes)}${photos} · ${stageName(t, day.stage)}`;
}

/** What a screen reader says for a day: "October 16, 5 notes, 3 photos, a large tree". */
export function dayAnnouncement(
  t: TFunction,
  language: Language,
  day: Pick<ForestDay, "date" | "notes" | "stage"> & { photos?: number },
): string {
  const photos = (day.photos ?? 0) > 0 ? `, ${photosText(t, day.photos ?? 0)}` : "";
  const summary = day.notes > 0 ? `${notesText(t, day.notes)}${photos}, ${stageName(t, day.stage)}` : t("forest.noNotes");
  return t("forest.announce", { date: shortDay(day.date, language), summary });
}
