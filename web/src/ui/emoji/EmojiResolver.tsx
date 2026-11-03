import type { EmojiDataResolver } from "frimousse";
import { createContext, useContext } from "react";
import { resolveEmojiData } from "./emojiData";

const EmojiResolverContext = createContext<EmojiDataResolver>(resolveEmojiData);

/**
 * Overriding the picker's data source for stories and tests (a deterministic set, endless loading, an error). In the app
 * there is no provider and `resolveEmojiData` from the bundle works. The value must be the same function on all renders:
 * frimousse caches the data by its identity.
 */
export const EmojiResolverProvider = EmojiResolverContext.Provider;

export function useEmojiResolver(): EmojiDataResolver {
  return useContext(EmojiResolverContext);
}
