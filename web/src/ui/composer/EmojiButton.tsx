import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { IconButton } from "../controls/IconButton";
import { EmojiPopover } from "../emoji/EmojiPopover";

/** Эмодзи-пикер в поле ввода (слайс 3). */
export interface ComposerEmoji {
  /** Недавно выбранные эмодзи, свежие первыми. */
  recent: readonly string[];
  language: Language;
  /** Эмодзи выбран и вставлен: запомнить его в «недавних». */
  onPicked?: (emoji: string) => void;
}

interface EmojiButtonProps extends ComposerEmoji {
  /** Поле ввода: в его позицию курсора вставляется эмодзи. */
  field: RefObject<HTMLTextAreaElement | null>;
  value: string;
  onChange: (value: string) => void;
}

/**
 * Кнопка «эмодзи» поля ввода: открывает панель пикера над собой. Выбор вставляет эмодзи вместо выделения (или в позицию
 * курсора), каретка встаёт после него, фокус возвращается в поле, панель закрывается.
 */
export function EmojiButton({ recent, language, onPicked, field, value, onChange }: EmojiButtonProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  // Каретку можно поставить, только когда родитель уже подставил новое значение в поле.
  const caret = useRef<{ value: string; position: number } | null>(null);

  useLayoutEffect(() => {
    const pending = caret.current;
    if (pending?.value === value) field.current?.setSelectionRange(pending.position, pending.position);
    caret.current = null;
  }, [value, field]);

  const pick = (emoji: string) => {
    const input = field.current;
    // Выделение textarea переживает уход фокуса в пикер; без поля (его нет в дереве) эмодзи дописывается в конец.
    const start = input?.selectionStart ?? value.length;
    const end = input?.selectionEnd ?? start;
    const next = value.slice(0, start) + emoji + value.slice(end);
    caret.current = { value: next, position: start + emoji.length };
    onChange(next);
    setOpen(false);
    input?.focus();
    onPicked?.(emoji);
  };

  return (
    <span className="composer__emoji" ref={anchor}>
      <IconButton
        icon="emoji"
        label={t("composer.emoji")}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      />
      {open ? (
        <EmojiPopover
          recent={recent}
          language={language}
          anchor={anchor}
          onPick={pick}
          onClose={(returnFocus) => {
            setOpen(false);
            if (returnFocus) field.current?.focus();
          }}
        />
      ) : null}
    </span>
  );
}
