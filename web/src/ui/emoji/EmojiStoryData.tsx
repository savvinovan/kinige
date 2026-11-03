import type { EmojiDataResolver } from "frimousse";
import { useEffect, useState, type ReactNode } from "react";
import { createEmojiResolver } from "./emojiData";
import { FALLBACK_SUPPORT } from "./emojiSupport";
import { EmojiResolverProvider } from "./EmojiResolver";
import "./emoji-stories.css";

// For Storybook stories only.

/** Real emojibase data with a fixed version limit (no glyph check): the set in the stories is the same everywhere. */
export const STORY_RESOLVER: EmojiDataResolver = createEmojiResolver(() => FALLBACK_SUPPORT);

/** The data never arrives: the "loading" state. */
export const LOADING_RESOLVER: EmojiDataResolver = () => new Promise(() => {});

/**
 * Picker data for the stories and "readiness": while the document has no emoji cell, the story is hidden (visibility), and
 * the snapshot does not catch the picker halfway between loading the data and the first paint of the grid. With `wait={false}` it is immediate.
 */
export function EmojiStoryData({ resolver = STORY_RESOLVER, wait = true, children }: { resolver?: EmojiDataResolver; wait?: boolean; children: ReactNode }) {
  const [ready, setReady] = useState(!wait);
  useEffect(() => {
    if (!wait) return;
    const check = () => {
      // Sample cells for measuring (empty aria-label) exist at once; a real cell or "nothing found" appears after the data.
      if (document.querySelector('[role="gridcell"]:not([aria-label=""]), [frimousse-empty]')) setReady(true);
    };
    check();
    const observer = new MutationObserver(check);
    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [wait]);
  return (
    <EmojiResolverProvider value={resolver}>
      <div className={ready ? "story-emoji" : "story-emoji story-emoji--pending"}>{children}</div>
    </EmojiResolverProvider>
  );
}
