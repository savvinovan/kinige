import { useId, useRef, type KeyboardEvent } from "react";
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
  /**
   * ⌘Enter (на не-macOS Ctrl+Enter) — «дописать» (окно быстрого захвата, слайс 2). Работает и внутри блока кода: перевода строки
   * не вставляет. Не во время ввода через IME; в пустом поле ничего не делает. Без обработчика ⌘Enter ведёт себя как Enter.
   */
  onAppend?: () => void;
  /** Какие кнопки показывать; по умолчанию все три. В приложении кнопка появляется в слайсе, где её функция работает. */
  buttons?: ComposerButtons;
  /** Кнопки только нарисованы (D1): объявляются как недоступные («Появится позже») и ничего не делают. */
  viewOnly?: boolean;
  /** Ошибка отправки: показывается над полем, черновик остаётся. */
  error?: string;
  /**
   * Плавающая панель (окно быстрого захвата): поле без своей заливки и рамки, полей и ограничения ширины колонки: фон, рамку
   * и скругление задаёт панель вокруг. Ошибка и подсказка стоят строкой ПОД полем: когда они появляются, окно растёт вниз, а
   * поле остаётся на месте. Область `role="alert"` в дереве всегда (пустая, высоты нет), текст в неё приходит позже — так
   * диктор объявляет ошибку надёжно.
   */
  floating?: boolean;
  /** Подсказка под полем в плавающей панели («⌘Enter — дописать»): поле описано ею; пока есть ошибка, подсказки нет. */
  hint?: string;
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
 * Поле ввода как в Telegram: слева «файл», в середине обычное поле «Написать заметку…», справа «эмодзи» и «микрофон»;
 * когда в поле есть текст, «микрофон» сменяется круглой акцентной кнопкой «Отправить» (как в ChatGPT и Claude).
 * Растёт вместе с текстом до 40% высоты окна, дальше — прокрутка внутри (ТЗ §5). Подсказку про Enter не показываем.
 */
export function Composer({
  value,
  onChange,
  onSubmit,
  onAppend,
  buttons = { attach: true, emoji: true, voice: true },
  viewOnly = false,
  error,
  floating = false,
  hint,
  placeholder,
  autoFocus,
}: ComposerProps) {
  const { t } = useTranslation();
  const errorId = useId();
  const hintId = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  const hasText = value.trim() !== "";

  // Кнопка «Отправить» делает то же, что Enter. После щелчка фокус возвращается в поле: сама кнопка исчезает, как только поле пустеет.
  const send = () => {
    onSubmit?.();
    input.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    // IME (русская раскладка, диктовка): Enter подтверждает ввод, а не отправляет.
    if (event.nativeEvent.isComposing) return;
    // ⌘Enter / Ctrl+Enter: «дописать». Перевод строки не вставляется нигде, в том числе внутри блока кода.
    if (onAppend && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (hasText) onAppend();
      return;
    }
    if (!onSubmit) return;
    if (insideCodeFence(event.currentTarget.value, event.currentTarget.selectionStart)) return;
    event.preventDefault();
    if (hasText) onSubmit();
  };

  const errorLine = (
    <p className="composer__error" id={errorId} role="alert">
      {error}
    </p>
  );
  const hintShown = floating && !error && hint;

  return (
    <div className={floating ? "composer composer--floating" : "composer"}>
      {error && !floating ? errorLine : null}
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
            aria-describedby={error ? errorId : hintShown ? hintId : undefined}
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
      {floating ? (
        <div className="composer__status">
          {errorLine}
          {hintShown ? (
            <p className="composer__hint" id={hintId}>
              {hint}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
