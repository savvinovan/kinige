/**
 * Initials for the avatar: the first letters of the first two words of the name, in capitals. One word gives one letter.
 * An empty name gives `fallback` (the default name from the translations: "Я" / "Me"). Letters are taken by Unicode
 * characters, not by code units (emoji and rare letters are not split).
 */
export function initials(name: string, fallback: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return fallback;
  return words
    .slice(0, 2)
    .map((word) => Array.from(word)[0] ?? "")
    .join("")
    .toUpperCase();
}
