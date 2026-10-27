import type { EmojiDataResolver } from "frimousse";
import { useEffect, useState, type ReactNode } from "react";
import { createEmojiResolver } from "./emojiData";
import { FALLBACK_SUPPORT } from "./emojiSupport";
import { EmojiResolverProvider } from "./EmojiResolver";
import "./emoji-stories.css";

// Только для историй Storybook.

/** Настоящие данные emojibase с фиксированной границей версий (без проверки глифов): набор в историях один и тот же везде. */
export const STORY_RESOLVER: EmojiDataResolver = createEmojiResolver(() => FALLBACK_SUPPORT);

/** Данные никогда не придут: состояние «загрузка». */
export const LOADING_RESOLVER: EmojiDataResolver = () => new Promise(() => {});

/**
 * Данные пикера для историй и «готовность»: пока в документе нет ни одной ячейки эмодзи, история скрыта (visibility), и
 * снимок не поймает пикер на полпути между загрузкой данных и первой отрисовкой сетки. С `wait={false}` — сразу.
 */
export function EmojiStoryData({ resolver = STORY_RESOLVER, wait = true, children }: { resolver?: EmojiDataResolver; wait?: boolean; children: ReactNode }) {
  const [ready, setReady] = useState(!wait);
  useEffect(() => {
    if (!wait) return;
    const check = () => {
      // Ячейки-образцы для замера (aria-label пуст) есть сразу; настоящая ячейка или «ничего не найдено» — после данных.
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
