import type { Language } from "../../i18n";

const UNITS = ["byte", "kilobyte", "megabyte", "gigabyte"] as const;
const LAST = UNITS.length - 1;
const STEP = 1024;

/**
 * A file size for a file card: "512 B", "2.4 MB" (en), "2,4 МБ" (ru). Binary steps, short unit names from `Intl`; one
 * fractional digit above 1 KB. A value that would round up to 1024 moves to the next unit ("1023.97 KB" is "1.0 MB").
 */
export function formatSize(bytes: number, language: Language | string): string {
  let value = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
  let unit = 0;
  while (unit < LAST && value >= STEP) {
    value /= STEP;
    unit += 1;
  }
  if (unit > 0 && unit < LAST && Math.round(value * 10) / 10 >= STEP) {
    value /= STEP;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : 1;
  return new Intl.NumberFormat(language, {
    style: "unit",
    unit: UNITS[unit],
    // The short English name of a byte is the word "byte" (Node 26 / ICU); the narrow one is "B". Larger units: "kB", "MB", "GB" / "кБ", "МБ", "ГБ".
    unitDisplay: unit === 0 ? "narrow" : "short",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(value);
}
