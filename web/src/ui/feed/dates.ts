// Время и метка дня в ленте (ТЗ §6). Календарные дни считаются в местном часовом поясе: «Сегодня» и «Вчера»
// меняются в местную полночь. Месяцы записаны в родительном падеже здесь, а не берутся из Intl: результат не зависит
// от версии ICU в WKWebView / WebView2 / WebKitGTK.

const MONTHS_GENITIVE = [
  "января",
  "февраля",
  "марта",
  "апреля",
  "мая",
  "июня",
  "июля",
  "августа",
  "сентября",
  "октября",
  "ноября",
  "декабря",
] as const;

/** Пауза между сообщениями, меньше которой они образуют серию (ТЗ §6): плотнее друг к другу. */
export const SERIES_PAUSE_MS = 5 * 60 * 1000;

const two = (value: number): string => String(value).padStart(2, "0");

/** Время в 24-часовом формате: «09:12». */
export function formatTime(date: Date): string {
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
}

/** Календарный день (местное время) как число: сравнивать и вычитать дни без ошибок перехода на летнее время. */
const dayNumber = (date: Date): number => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000;

/** Тот же календарный день в местном времени. */
export function isSameDay(a: Date, b: Date): boolean {
  return dayNumber(a) === dayNumber(b);
}

/**
 * Одна метка дня, без дня недели и без второй строки с той же датой:
 * «Сегодня», «Вчера», «14 марта» (текущий год), «14 марта 2025» (другие годы).
 */
export function formatDayLabel(day: Date, now: Date): string {
  const daysAgo = dayNumber(now) - dayNumber(day);
  if (daysAgo === 0) return "Сегодня";
  if (daysAgo === 1) return "Вчера";
  const date = `${day.getDate()} ${MONTHS_GENITIVE[day.getMonth()]}`;
  return day.getFullYear() === now.getFullYear() ? date : `${date} ${day.getFullYear()}`;
}

/** Следующее сообщение продолжает серию: тот же день и пауза меньше 5 минут. */
export function continuesSeries(previous: Date, next: Date): boolean {
  const pause = next.getTime() - previous.getTime();
  return isSameDay(previous, next) && pause >= 0 && pause < SERIES_PAUSE_MS;
}
