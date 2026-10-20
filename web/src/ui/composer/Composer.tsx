import { useId, type KeyboardEvent } from "react";
import { IconButton } from "../controls/IconButton";
import "./composer.css";

export interface ComposerButtons {
  /** «Прикрепить файл» (слайс 7). */
  attach?: boolean;
  /** «Эмодзи» (слайс 3). */
  emoji?: boolean;
  /** «Записать голосовое» (слайс 7a). */
  voice?: boolean;
}

interface ComposerProps {
  value: string;
  onChange: (value: string) => void;
  /** Enter без Shift (и не внутри блока кода) отправляет. В D1 отправки нет: поле показывает вид. */
  onSubmit?: () => void;
  /** Какие кнопки показывать; по умолчанию все три. В приложении кнопка появляется в слайсе, где её функция работает. */
  buttons?: ComposerButtons;
  /** Ошибка отправки: показывается над полем, черновик остаётся. */
  error?: string;
  placeholder?: string;
  /** Фокус в поле при показе: после отправки и при открытии чата. */
  autoFocus?: boolean;
}

/** Внутри блока кода (открыто нечётное число ограждений ```) Enter — новая строка, а не отправка (ТЗ §5). */
export function insideCodeFence(text: string, caret: number): boolean {
  const fences = text.slice(0, caret).match(/^```/gm);
  return (fences?.length ?? 0) % 2 === 1;
}

/**
 * Поле ввода как в Telegram: слева «файл», в середине обычное поле «Написать заметку…», справа «эмодзи» и «микрофон».
 * Растёт вместе с текстом до 40% высоты окна, дальше — прокрутка внутри (ТЗ §5). Подсказку про Enter не показываем.
 */
export function Composer({
  value,
  onChange,
  onSubmit,
  buttons = { attach: true, emoji: true, voice: true },
  error,
  placeholder = "Написать заметку…",
  autoFocus,
}: ComposerProps) {
  const errorId = useId();

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || !onSubmit) return;
    // IME (русская раскладка, диктовка): Enter подтверждает ввод, а не отправляет.
    if (event.nativeEvent.isComposing) return;
    if (insideCodeFence(event.currentTarget.value, event.currentTarget.selectionStart)) return;
    event.preventDefault();
    if (value.trim() !== "") onSubmit();
  };

  return (
    <div className="composer">
      {error ? (
        <p className="composer__error" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
      <div className={`composer__bar${error ? " composer__bar--invalid" : ""}`}>
        {buttons.attach ? <IconButton icon="attach" label="Прикрепить файл" /> : null}
        {/* Копия текста в ::after задаёт высоту поля: оно растёт без скриптов измерения. */}
        <div className="composer__field" data-replicated-value={value}>
          <textarea
            className="composer__input"
            rows={1}
            value={value}
            placeholder={placeholder}
            aria-label="Новая заметка"
            autoFocus={autoFocus}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? errorId : undefined}
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={onKeyDown}
          />
        </div>
        {buttons.emoji ? <IconButton icon="emoji" label="Эмодзи" /> : null}
        {buttons.voice ? <IconButton icon="voice" label="Записать голосовое" /> : null}
      </div>
    </div>
  );
}
