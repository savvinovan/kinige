import { EmojiPicker as Frimousse } from "frimousse";
import { useState, type CSSProperties, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { Icon } from "../icons/Icon";
import { EMOJI_COLUMNS, RECENT_LIMIT } from "./emojiLayout";
import { useEmojiResolver } from "./EmojiResolver";
import { RecentEmoji } from "./RecentEmoji";
import "./emoji.css";

export interface EmojiPickerProps {
  /** Недавно выбранные эмодзи, свежие первыми; показываются до 24 (пока в поиске пусто). */
  recent: readonly string[];
  /** Язык интерфейса: на нём подписи эмодзи и названия категорий; искать можно и на втором языке. */
  language: Language;
  onPick: (emoji: string) => void;
  /** Фокус в поле поиска при показе. */
  autoFocus?: boolean;
}

// Эмодзи рисует системный шрифт из токена, а не список шрифтов, который frimousse подставляет сам.
const EMOJI_FONT = { "--frimousse-emoji-font": "var(--font-family-emoji)" } as CSSProperties;

/**
 * Пикер эмодзи: поиск, «Недавние», категории, сетка. Основа — frimousse (без оформления, виртуальный список); вид — только
 * токены. Поиск идёт по подписи и тегам на обоих языках. Стрелки двигают выбор по сетке, пока фокус в поле поиска
 * (Enter выбирает); пока в поле есть текст, стрелки влево и вправо остаются за кареткой. Esc обрабатывает хозяин пикера
 * (всплывающая панель или окно чата).
 */
export function EmojiPicker({ recent, language, onPick, autoFocus }: EmojiPickerProps) {
  const { t } = useTranslation();
  const resolver = useEmojiResolver();
  const [search, setSearch] = useState("");
  const recentShown = search.trim() === "" ? [...new Set(recent)].slice(0, RECENT_LIMIT) : [];

  const onSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // frimousse слушает стрелки на документе и гасит их. Пока в поле набран запрос, влево и вправо — каретка.
    if (search !== "" && (event.key === "ArrowLeft" || event.key === "ArrowRight")) event.stopPropagation();
  };

  return (
    <Frimousse.Root
      className="emoji-picker"
      role="group"
      aria-label={t("emoji.label")}
      locale={language}
      columns={EMOJI_COLUMNS}
      resolveEmojiData={resolver}
      onEmojiSelect={({ emoji }) => onPick(emoji)}
      style={EMOJI_FONT}
    >
      <label className="emoji-picker__search-box">
        <Icon name="search" />
        <Frimousse.Search
          className="emoji-picker__search"
          aria-label={t("emoji.search")}
          placeholder={t("emoji.search")}
          autoFocus={autoFocus}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          onKeyDown={onSearchKeyDown}
        />
      </label>
      {recentShown.length > 0 ? <RecentEmoji emojis={recentShown} onPick={onPick} /> : null}
      <Frimousse.Viewport className="emoji-picker__viewport">
        <Frimousse.Loading className="emoji-picker__note">{t("emoji.loading")}</Frimousse.Loading>
        <Frimousse.Empty className="emoji-picker__note">{t("emoji.empty")}</Frimousse.Empty>
        <Frimousse.List className="emoji-picker__list" />
      </Frimousse.Viewport>
    </Frimousse.Root>
  );
}
