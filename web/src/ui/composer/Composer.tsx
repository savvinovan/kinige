import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
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
  /** Кнопки только нарисованы (D1): объявляются как недоступные («Появится позже») и ничего не делают. */
  viewOnly?: boolean;
  /** Ошибка отправки: показывается над полем, черновик остаётся. */
  error?: string;
  placeholder?: string;
  /** Фокус в поле при показе: после отправки и при открытии чата. */
  autoFocus?: boolean;
  /** Значение растёт каждый раз, когда фокус нужно вернуть в поле (после «Отменить»: кнопка тоста исчезает вместе с фокусом). */
  focusSignal?: number;
}

/** Внутри блока кода (открыто нечётное число ограждений ```) Enter — новая строка, а не отправка (ТЗ §5). */
export function insideCodeFence(text: string, caret: number): boolean {
  const fences = text.slice(0, caret).match(/^```/gm);
  return (fences?.length ?? 0) % 2 === 1;
}

/**
 * Поле ввода как в Telegram: слева «файл», в середине обычное поле «Написать заметку…», справа «эмодзи» и «микрофон»;
 * когда в поле есть текст, «микрофон» сменяется круглой акцентной кнопкой «Отправить» (как в ChatGPT и Claude).
 * Растёт вместе с текстом до 40% высоты окна, дальше — прокрутка внутри (ТЗ §5). Подсказку про Enter не показываем.
 */
export function Composer({
  value,
  onChange,
  onSubmit,
  buttons = { attach: true, emoji: true, voice: true },
  viewOnly = false,
  error,
  placeholder,
  autoFocus,
  focusSignal = 0,
}: ComposerProps) {
  const { t } = useTranslation();
  const errorId = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (focusSignal > 0) input.current?.focus();
  }, [focusSignal]);
  const hasText = value.trim() !== "";

  // Кнопка «Отправить» делает то же, что Enter. После щелчка фокус возвращается в поле: сама кнопка исчезает, как только поле пустеет.
  const send = () => {
    onSubmit?.();
    input.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || !onSubmit) return;
    // IME (русская раскладка, диктовка): Enter подтверждает ввод, а не отправляет.
    if (event.nativeEvent.isComposing) return;
    if (insideCodeFence(event.currentTarget.value, event.currentTarget.selectionStart)) return;
    event.preventDefault();
    if (hasText) onSubmit();
  };

  return (
    <div className="composer">
      {error ? (
        <p className="composer__error" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
      <div className={`composer__bar${error ? " composer__bar--invalid" : ""}`}>
        {buttons.attach ? <IconButton icon="attach" label={t("composer.attach")} unavailable={viewOnly} /> : null}
        {/* Копия текста в ::after задаёт высоту поля: оно растёт без скриптов измерения. */}
        <div className="composer__field" data-replicated-value={value}>
          <textarea
            className="composer__input"
            ref={input}
            rows={1}
            value={value}
            placeholder={placeholder ?? t("composer.placeholder")}
            aria-label={t("composer.label")}
            autoFocus={autoFocus}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? errorId : undefined}
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={onKeyDown}
          />
        </div>
        {buttons.emoji ? <IconButton icon="emoji" label={t("composer.emoji")} unavailable={viewOnly} /> : null}
        {/* Правое место: «Отправить» при непробельном тексте, иначе «микрофон». Обе кнопки одного размера, ширина не прыгает. */}
        {hasText ? (
          <span className="composer__action" key="send">
            <IconButton icon="send" label={t("composer.send")} tone="accent" unavailable={!onSubmit} onClick={send} />
          </span>
        ) : buttons.voice ? (
          <span className="composer__action" key="voice">
            <IconButton icon="voice" label={t("composer.voice")} unavailable={viewOnly} />
          </span>
        ) : null}
      </div>
    </div>
  );
}
