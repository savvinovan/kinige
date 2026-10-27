import { useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { EMOJI_COLUMNS } from "./emojiLayout";

interface RecentEmojiProps {
  emojis: readonly string[];
  onPick: (emoji: string) => void;
}

/**
 * Секция «Недавние» над сеткой пикера: до 24 эмодзи в тех же столбцах. Одна остановка в обходе Tab (скользящий tabindex),
 * внутри — стрелки, Home и End. Вне прокручиваемой сетки frimousse: её виртуализация считает строки от верха области и не
 * знает о посторонних блоках внутри.
 */
export function RecentEmoji({ emojis, onPick }: RecentEmojiProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const [requested, setActive] = useState(0);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  // Список «недавних» может стать короче (поиск, смена данных): активным остаётся последний существующий.
  const active = Math.min(requested, emojis.length - 1);

  const move = (to: number) => {
    const next = Math.max(0, Math.min(emojis.length - 1, to));
    setActive(next);
    items.current[next]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const steps: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -EMOJI_COLUMNS, ArrowDown: EMOJI_COLUMNS };
    const step = steps[event.key];
    // Отправная точка — кнопка, на которой стоит фокус, а не состояние: оно может отстать от фокуса на один кадр.
    const current = items.current.indexOf(event.target as HTMLButtonElement);
    if (step !== undefined) {
      event.preventDefault();
      // Вниз с последней строки и вверх с первой остаются на месте: не перескакивают на край.
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
