import { type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { EmojiPicker } from "./EmojiPicker";
import "./emoji.css";

export interface EmojiInlineProps {
  recent: readonly string[];
  language: Language;
  onPick: (emoji: string) => void;
  /** Закрыть панель; `returnFocus` — вернуть фокус в поле (Esc, Tab). */
  onClose: (returnFocus: boolean) => void;
  /** Место в потоке содержимого (над строкой поля), куда встаёт панель. */
  host: HTMLElement;
}

/**
 * Пикер эмодзи «в потоке» (окно быстрого захвата): панель занимает место над строкой поля и раздвигает содержимое, а окно
 * подгоняется под него (растёт вверх, нижний край на месте). В отличие от всплывающей панели (`EmojiPopover`) ничего не
 * накладывает поверх: у окна захвата нет места за границами. Щелчок мимо её не закрывает: она закрывается кнопкой «эмодзи»,
 * выбором эмодзи и Esc. Фокус при открытии уходит в поиск.
 */
export function EmojiInline({ recent, language, onPick, onClose, host }: EmojiInlineProps) {
  const { t } = useTranslation();

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      // Esc сначала закрывает пикер, окно (слушает документ) остаётся.
      event.stopPropagation();
      onClose(true);
    }
  };

  return createPortal(
    <div className="emoji-inline" role="dialog" aria-label={t("emoji.dialog")} onKeyDown={onKeyDown}>
      <EmojiPicker recent={recent} language={language} autoFocus onPick={onPick} />
    </div>,
    host,
  );
}
