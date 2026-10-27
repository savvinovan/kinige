import { useEffect, useLayoutEffect, useRef, type KeyboardEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { EmojiPicker } from "./EmojiPicker";
import "./emoji.css";

export interface EmojiPopoverProps {
  recent: readonly string[];
  language: Language;
  onPick: (emoji: string) => void;
  /**
   * Закрыть панель. `returnFocus` — вернуть ли фокус туда, откуда пришли (Esc, Tab, выбор эмодзи): щелчок мимо фокус не
   * возвращает, человек уже выбрал другое место.
   */
  onClose: (returnFocus: boolean) => void;
  /** Кнопка «эмодзи»: панель встаёт над ней, правым краем к её правому краю. Щелчок по кнопке не считается «щелчком мимо»: кнопка сама переключает панель. */
  anchor: RefObject<HTMLElement | null>;
}

/**
 * Всплывающая панель с пикером над кнопкой «эмодзи» поля ввода. Накладывается поверх окна и ничего не сдвигает. Esc, Tab,
 * щелчок мимо, прокрутка, смена размера окна и уход из окна закрывают её; фокус при открытии уходит в поиск.
 */
export function EmojiPopover({ recent, language, onPick, onClose, anchor }: EmojiPopoverProps) {
  const { t } = useTranslation();
  const panel = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // До первой отрисовки, прямо в DOM: панель закреплена к нижнему и правому краю окна у кнопки (растёт вверх и влево) и не
  // выше места над кнопкой. Положение берётся из прямоугольника кнопки, поэтому панель не мигает в углу окна.
  useLayoutEffect(() => {
    const rect = anchor.current?.getBoundingClientRect();
    const element = panel.current;
    if (!rect || !element) return;
    element.style.bottom = `${window.innerHeight - rect.top}px`;
    element.style.right = `${window.innerWidth - rect.right}px`;
    element.style.setProperty("--emoji-popover-room", `${rect.top}px`);
  }, [anchor]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (panel.current?.contains(target) || anchor.current?.contains(target)) return;
      onCloseRef.current(false);
    };
    const closeWithoutFocus = () => onCloseRef.current(false);
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("resize", closeWithoutFocus);
    window.addEventListener("blur", closeWithoutFocus);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("resize", closeWithoutFocus);
      window.removeEventListener("blur", closeWithoutFocus);
    };
  }, [anchor]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      onClose(true);
    }
  };

  return createPortal(
    <div className="emoji-popover" role="dialog" aria-label={t("emoji.dialog")} ref={panel} onKeyDown={onKeyDown}>
      <EmojiPicker recent={recent} language={language} autoFocus onPick={onPick} />
    </div>,
    document.body,
  );
}
