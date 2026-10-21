// Счётчик записей чата в сайдбаре (ADR-0004): считаются все записи, кроме корзины; тихий серый бейдж справа.
// Числа форматирует `Intl` на языке интерфейса (ADR-0005); дробная часть отбрасывается, а не округляется: бейдж не должен
// показывать больше, чем есть.

import type { Language } from "../../i18n";

const plain = new Map<Language, Intl.NumberFormat>();
const compact = new Map<string, Intl.NumberFormat>();

/**
 * Сколько показывать в бейдже. Пустая строка — бейдж скрыт (ноль и нечисловые значения).
 * 0 → скрыт; 1…9 999 → «7», «1 234» / «1,234»; 10 000…999 999 → «12 тыс.» / «12K»; от миллиона — «1,2 млн» / «1.2M»
 * (до 10 млн с одной цифрой после запятой, дальше целые).
 */
export function formatCount(count: number, language: Language): string {
  if (!Number.isFinite(count) || count < 1) return "";
  const whole = Math.floor(count);
  if (whole < 10_000) {
    let format = plain.get(language);
    if (!format) plain.set(language, (format = new Intl.NumberFormat(language, { useGrouping: true })));
    return format.format(whole);
  }
  const digits = whole >= 1_000_000 && whole < 10_000_000 ? 1 : 0;
  const key = `${language}|${digits}`;
  let format = compact.get(key);
  if (!format) {
    format = new Intl.NumberFormat(language, {
      notation: "compact",
      compactDisplay: "short",
      maximumFractionDigits: digits,
      roundingMode: "trunc",
    });
    compact.set(key, format);
  }
  return format.format(whole);
}
