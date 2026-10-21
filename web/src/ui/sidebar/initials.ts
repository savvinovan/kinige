/**
 * Инициалы для аватара: первые буквы первых двух слов имени, заглавными. Одно слово — одна буква.
 * Пустое имя — `fallback` (имя по умолчанию из переводов: «Я» / «Me»). Буквы берутся по символам Юникода,
 * а не по кодовым единицам (эмодзи и редкие буквы не рвутся).
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
