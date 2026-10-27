/**
 * Столбцов в сетке пикера. frimousse принимает число, а не CSS-переменную, поэтому оно стоит и здесь, и в токене
 * `emoji.columns`; тест `emojiTokens.test.ts` сверяет оба.
 */
export const EMOJI_COLUMNS = 9;

/** Сколько «недавних» эмодзи показывает пикер (ТЗ слайса 3). */
export const RECENT_LIMIT = 24;
