import { useRef, type KeyboardEvent } from "react";
import "./settings.css";

interface Option<T extends string> {
  value: T;
  label: string;
}

interface SegmentedChoiceProps<T extends string> {
  options: readonly Option<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Подпись группы для диктора: id видимого заголовка (предпочтительно) или текст. Нужна одна из двух. */
  labelledBy?: string;
  label?: string;
  /** Выбор недоступен (например, доступ агентов выключен): варианты приглушены и вне обхода Tab. */
  disabled?: boolean;
}

/**
 * Выбор одного из нескольких вариантов («Обычный» / «Крупный»): группа радиокнопок (`radiogroup`) в виде сегментов.
 * В обходе Tab участвует один вариант, выбранный (roving tabindex); стрелки, Home и End переносят фокус и сразу выбирают,
 * как у обычных радиокнопок. Выбранный вариант отмечен заливкой и тонкой линией цвета акцента, а не только цветом.
 */
export function SegmentedChoice<T extends string>({ options, value, onChange, labelledBy, label, disabled = false }: SegmentedChoiceProps<T>) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  const select = (index: number) => {
    const target = options[(index + options.length) % options.length];
    if (!target) return;
    buttons.current[options.indexOf(target)]?.focus();
    onChange(target.value);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = options.findIndex((option) => option.value === value);
    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        event.preventDefault();
        select(current + 1);
        break;
      case "ArrowLeft":
      case "ArrowUp":
        event.preventDefault();
        select(current - 1);
        break;
      case "Home":
        event.preventDefault();
        select(0);
        break;
      case "End":
        event.preventDefault();
        select(options.length - 1);
        break;
    }
  };

  return (
    <div className="segmented" role="radiogroup" aria-labelledby={labelledBy} aria-label={labelledBy ? undefined : label} onKeyDown={onKeyDown}>
      {options.map((option, index) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          className="segmented__option"
          aria-checked={option.value === value}
          disabled={disabled}
          tabIndex={option.value === value ? 0 : -1}
          ref={(element) => {
            buttons.current[index] = element;
          }}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
