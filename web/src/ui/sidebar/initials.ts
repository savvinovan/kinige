/** Имя по умолчанию, пока пользователь не задал своё (слайс 1: настройки). */
export const DEFAULT_PROFILE_NAME = "Я";

/**
 * Инициалы для аватара: первые буквы первых двух слов имени, заглавными. Одно слово — одна буква.
 * Пустое имя — «Я». Буквы берутся по символам Юникода, а не по кодовым единицам (эмодзи и редкие буквы не рвутся).
 */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return DEFAULT_PROFILE_NAME;
  return words
    .slice(0, 2)
    .map((word) => Array.from(word)[0] ?? "")
    .join("")
    .toLocaleUpperCase("ru");
}
