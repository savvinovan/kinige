// Время и метка дня в ленте (ТЗ §6). День записи — её `journal_date` (его считает Rust по границе дня 04:00, §4.1),
// а не календарный день по часам окна: запись в 00:30 стоит под вчерашней меткой. Время — в часовом поясе, в котором
// запись создана (`created_tz`). Даты и числа форматирует `Intl` на языке интерфейса.

import type { Language } from "../../i18n";

/** Пауза между сообщениями, меньше которой они образуют серию (ТЗ §6): плотнее друг к другу. */
export const SERIES_PAUSE_MS = 5 * 60 * 1000;

const timeFormats = new Map<string, Intl.DateTimeFormat>();

/** Формат времени для языка и пояса; неизвестный пояс (повреждённые данные) — часовой пояс окна, а не ошибка. */
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

/** Время в 24-часовом виде («09:12») в поясе записи; без пояса — в поясе окна. */
export function formatTime(date: Date, language: Language, timeZone?: string): string {
  return timeFormat(language, timeZone).format(date);
}

/** `YYYY-MM-DD` → число дней от эпохи (календарная арифметика без часовых поясов); `null`, если строка не дата. */
function dayNumber(journalDate: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(journalDate);
  if (!match) return null;
  const time = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return Number.isNaN(time) ? null : time / 86_400_000;
}

/** Строки для «Сегодня» и «Вчера» берутся из переводов. */
export interface DayLabels {
  today: string;
  yesterday: string;
}

/**
 * Одна метка дня, без дня недели и без второй строки с той же датой: «Сегодня», «Вчера», «14 марта» / «March 14»
 * (текущий год), «14 марта 2025» / «March 14, 2025» (другие годы). Сравнение — по `journal_date`, «сегодня» присылает Rust.
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
  // Русский формат добавляет к году « г.»; метка в ленте без него («14 марта 2025»).
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return parts.map((part) => part.value).join("");
}

/** Следующее сообщение продолжает серию: тот же день записей (`journal_date`) и пауза меньше 5 минут. */
export function continuesSeries(
  previous: { createdAt: number; journalDate: string },
  next: { createdAt: number; journalDate: string },
): boolean {
  const pause = next.createdAt - previous.createdAt;
  return previous.journalDate === next.journalDate && pause >= 0 && pause < SERIES_PAUSE_MS;
}
