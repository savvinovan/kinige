import type { EmojiDataResolver } from "frimousse";
import { createContext, useContext } from "react";
import { resolveEmojiData } from "./emojiData";

const EmojiResolverContext = createContext<EmojiDataResolver>(resolveEmojiData);

/**
 * Подмена источника данных пикера для историй и тестов (детерминированный набор, вечная загрузка, ошибка). В приложении
 * провайдера нет и работает `resolveEmojiData` из бандла. Значение должно быть одной и той же функцией на всех рендерах:
 * frimousse кэширует данные по её идентичности.
 */
export const EmojiResolverProvider = EmojiResolverContext.Provider;

export function useEmojiResolver(): EmojiDataResolver {
  return useContext(EmojiResolverContext);
}
