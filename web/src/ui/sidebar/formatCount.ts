// Счётчик записей чата в сайдбаре (ADR-0004): считаются все записи, кроме корзины; тихий серый бейдж справа.

/** Узкий неразрывный пробел (U+202F): разряды числа не рвутся на конце строки. */
export const THIN_NBSP = " ";

/**
 * Сколько показывать в бейдже. Пустая строка — бейдж скрыт (ноль и нечисловые значения).
 * 0 → скрыт; 1…999 → «7», «999»; 1 000…9 999 → «1 234», «9 999» (узкий неразрывный пробел);
 * 10 000…999 999 → «12 тыс.» (дробная часть отбрасывается); от миллиона → «1,2 млн», «12 млн» (отбрасывается, а не округляется:
 * бейдж не должен показывать больше, чем есть).
 */
export function formatCount(count: number): string {
  if (!Number.isFinite(count) || count < 1) return "";
  const whole = Math.floor(count);
  if (whole < 1_000) return String(whole);
  if (whole < 10_000) return `${Math.floor(whole / 1_000)}${THIN_NBSP}${String(whole % 1_000).padStart(3, "0")}`;
  if (whole < 1_000_000) return `${Math.floor(whole / 1_000)} тыс.`;
  const tenths = Math.floor(whole / 100_000); // десятые доли миллиона: 1 234 567 → 12
  const millions = Math.floor(tenths / 10);
  const fraction = tenths % 10;
  return millions >= 10 || fraction === 0 ? `${millions} млн` : `${millions},${fraction} млн`;
}

/** Полное число с узкими неразрывными пробелами и склонением: «1 234 записи», «21 запись». Для подсказки и диктора. */
export function describeCount(count: number): string {
  const whole = Math.max(0, Math.floor(count));
  const digits = String(whole).replace(/\B(?=(\d{3})+(?!\d))/g, THIN_NBSP);
  const lastTwo = whole % 100;
  const last = whole % 10;
  const noun = lastTwo >= 11 && lastTwo <= 14 ? "записей" : last === 1 ? "запись" : last >= 2 && last <= 4 ? "записи" : "записей";
  return `${digits} ${noun}`;
}
