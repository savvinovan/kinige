// The chat entry counter in the sidebar (ADR-0004): all entries except the trash are counted; a quiet gray badge on the right.
// Numbers are formatted by `Intl` in the interface language (ADR-0005); the fractional part is dropped, not rounded: the badge
// must not show more than there is.

import type { Language } from "../../i18n";

// The spaces are given by code, not as characters in the text: they cannot be told apart by eye and are easy to break when editing.
/** The no-break space (U+00A0): Intl separates digit groups with it in Russian. */
const NBSP = String.fromCharCode(0x00a0);
/** The narrow no-break space (U+202F): digit groups do not break at the end of a line; this was so in D1. */
export const THIN_NBSP = String.fromCharCode(0x202f);

const plain = new Map<Language, Intl.NumberFormat>();
const compact = new Map<string, Intl.NumberFormat>();

/**
 * What to show in the badge. An empty string means the badge is hidden (zero and non-numeric values).
 * 0 -> hidden; 1...9 999 -> "7", "1 234" / "1,234"; 10 000...999 999 -> "12 тыс." / "12K"; from a million -> "1,2 млн" / "1.2M"
 * (up to 10 million with one digit after the decimal separator, whole numbers beyond that).
 */
export function formatCount(count: number, language: Language): string {
  if (!Number.isFinite(count) || count < 1) return "";
  const whole = Math.floor(count);
  if (whole < 10_000) {
    let format = plain.get(language);
    if (!format) plain.set(language, (format = new Intl.NumberFormat(language, { useGrouping: true })));
    // The Russian digit group separator in Intl is U+00A0, in D1 it was U+202F: the number in the quiet badge must not "spread".
    // For English the separator is a comma, nothing to change.
    return format.format(whole).replaceAll(NBSP, THIN_NBSP);
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
