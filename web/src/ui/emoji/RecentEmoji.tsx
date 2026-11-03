import { useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { EMOJI_COLUMNS } from "./emojiLayout";

interface RecentEmojiProps {
  emojis: readonly string[];
  onPick: (emoji: string) => void;
}

/**
 * The "Recent" section above the picker grid: up to 24 emoji in the same columns. A single Tab stop (roving tabindex),
 * inside it arrows, Home and End. Outside the scrollable frimousse grid: its virtualization counts rows from the top of the area and does not
 * know about foreign blocks inside.
 */
export function RecentEmoji({ emojis, onPick }: RecentEmojiProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const [requested, setActive] = useState(0);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  // The "recent" list can become shorter (search, data change): the last existing one stays active.
  const active = Math.min(requested, emojis.length - 1);

  const move = (to: number) => {
    const next = Math.max(0, Math.min(emojis.length - 1, to));
    setActive(next);
    items.current[next]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const steps: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -EMOJI_COLUMNS, ArrowDown: EMOJI_COLUMNS };
    const step = steps[event.key];
    // The starting point is the button that has focus, not the state: the state can lag behind focus by one frame.
    const current = items.current.indexOf(event.target as HTMLButtonElement);
    if (step !== undefined) {
      event.preventDefault();
      // Down from the last row and up from the first stay in place: they do not jump to the edge.
      const target = (current < 0 ? active : current) + step;
      if (target >= 0 && target < emojis.length) move(target);
    } else if (event.key === "Home") {
      event.preventDefault();
      move(0);
    } else if (event.key === "End") {
      event.preventDefault();
      move(emojis.length - 1);
    }
  };

  return (
    <section className="emoji-picker__recent" aria-labelledby={headingId}>
      <h3 className="emoji-picker__heading" id={headingId}>
        {t("emoji.recent")}
      </h3>
      <div className="emoji-picker__recent-grid" onKeyDown={onKeyDown}>
        {emojis.map((emoji, index) => (
          <button
            key={emoji}
            type="button"
            className="emoji-picker__recent-item"
            tabIndex={index === active ? 0 : -1}
            ref={(element) => {
              items.current[index] = element;
            }}
            onFocus={() => setActive(index)}
            onClick={() => onPick(emoji)}
          >
            {emoji}
          </button>
        ))}
      </div>
    </section>
  );
}
