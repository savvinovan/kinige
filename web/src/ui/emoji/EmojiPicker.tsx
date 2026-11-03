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
  /** Recently chosen emoji, newest first; up to 24 are shown (while the search is empty). */
  recent: readonly string[];
  /** Interface language: emoji labels and category names are in it; you can search in the second language too. */
  language: Language;
  onPick: (emoji: string) => void;
  /** Focus in the search field on display. */
  autoFocus?: boolean;
}

// Emoji are drawn by the system font from the token, not by the font list that frimousse substitutes itself.
const EMOJI_FONT = { "--frimousse-emoji-font": "var(--font-family-emoji)" } as CSSProperties;

/**
 * Emoji picker: search, "Recent", categories, grid. The base is frimousse (unstyled, virtual list); the look is tokens
 * only. Search goes by label and tags in both languages. Arrows move the selection over the grid while focus is in the search field
 * (Enter chooses); while the field has text, the left and right arrows stay with the caret. Esc is handled by the picker's owner
 * (the popover panel or the chat window).
 */
export function EmojiPicker({ recent, language, onPick, autoFocus }: EmojiPickerProps) {
  const { t } = useTranslation();
  const resolver = useEmojiResolver();
  const [search, setSearch] = useState("");
  const recentShown = search.trim() === "" ? [...new Set(recent)].slice(0, RECENT_LIMIT) : [];

  const onSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // frimousse listens to arrows on the document and swallows them. While a query is typed in the field, left and right belong to the caret.
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
