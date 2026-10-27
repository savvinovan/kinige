import { useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Composer } from "../composer/Composer";
import { formatTime } from "../feed/dates";
import type { FeedMessage } from "../feed/buildFeed";
import "../message/markdown.css";
import { renderContextMarkdown } from "./contextMarkdown";
import "./capture.css";

export interface CapturePanelProps {
  /** До трёх последних записей Inbox за сегодня, самая новая последняя (ближе к полю). */
  context: readonly FeedMessage[];
  /** Запись, к которой ⌘Enter допишет текст; пока она в контексте, под полем стоит подсказка «⌘Enter — дописать». `null` — подсказки нет. */
  appendTargetId: string | null;
  value: string;
  onChange: (value: string) => void;
  /** Enter. */
  onSend: () => void;
  /** ⌘Enter (на не-macOS Ctrl+Enter). */
  onAppend: () => void;
  /** Esc в любом месте окна. */
  onHide: () => void;
  /** Уже переведённый текст ошибки; показывается под полем, текст в поле остаётся. */
  error?: string;
  /**
   * Высота содержимого вместе с прозрачным полем под тень (логические px), при каждом её изменении. Окно приложения по ней
   * подгоняет высоту окна ОС; в историях Storybook обработчика нет.
   */
  onHeight?: (height: number) => void;
  /**
   * Пикер эмодзи: кнопка «эмодзи» в поле, панель в потоке над полем (окно растёт вверх). Без него кнопки нет. `open` и
   * `onOpenChange` ведёт хозяин: Esc сначала закрывает пикер (`onOpenChange(false)`), а окно (`onHide`) — только следующим нажатием.
   */
  emoji?: {
    recent: readonly string[];
    onPicked: (emoji: string) => void;
    open: boolean;
    onOpenChange: (open: boolean) => void;
  };
}

/**
 * Содержимое окна быстрого захвата (слайс 2): компактная плавающая панель в духе Gemini и ChatGPT — скруглённая «пилюля»
 * с тонкой светлой обводкой и мягкой тенью. Внутри сверху до трёх бледных плашек контекста (время, Markdown свёрстан, длинные
 * обрезаны по строкам, ссылки не нажимаются), под ними строка поля ввода с кнопкой «Отправить» внутри, под полем — подсказка
 * «⌘Enter — дописать» или ошибка. Без записей за сегодня — только поле. Окно прозрачное и без системной рамки: форму и тень
 * рисует CSS, вокруг панели поле под тень. Ни тостов, ни подтверждений.
 *
 * Контент не прыгает: подсказка и ошибка стоят строкой под полем, окно подгоняется под содержимое с верхом на месте, поэтому
 * при их появлении оно растёт вниз, а поле остаётся там же. Пока в поле длинный текст, контекст сжимается сверху: старые
 * плашки уходят за край.
 *
 * Окно создаётся скрытым и показывается много раз, поэтому автофокуса при монтировании мало: при каждом получении фокуса
 * окном курсор снова ставится в поле. Esc скрывает окно, где бы ни стоял фокус (кроме ввода через IME).
 */
export function CapturePanel({ context, appendTargetId, value, onChange, onSend, onAppend, onHide, error, onHeight, emoji }: CapturePanelProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const root = useRef<HTMLDivElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  // Свежие обработчики без переподписки: родитель может передавать новые функции на каждый рендер.
  const onHideRef = useRef(onHide);
  const onHeightRef = useRef(onHeight);
  const emojiRef = useRef(emoji);
  useEffect(() => {
    onHideRef.current = onHide;
    onHeightRef.current = onHeight;
    emojiRef.current = emoji;
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing) return;
      event.preventDefault();
      // Открыт пикер (фокус в поле, а не в пикере: из пикера Esc до сюда не доходит): сначала закрывается он, окно остаётся.
      const picker = emojiRef.current;
      if (picker?.open) {
        picker.onOpenChange(false);
        root.current?.querySelector("textarea")?.focus();
        return;
      }
      onHideRef.current();
    };
    const focusField = () => root.current?.querySelector("textarea")?.focus();
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("focus", focusField);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("focus", focusField);
    };
  }, []);

  // Высота содержимого: сообщается при первом кадре и при каждом изменении (текст, ошибка, подсказка, размер текста, контекст).
  useEffect(() => {
    const element = shell.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    let last = 0;
    const report = () => {
      const height = Math.ceil(element.getBoundingClientRect().height);
      if (height <= 0 || height === last) return;
      last = height;
      onHeightRef.current?.(height);
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, []);

  const taskDone = t("message.taskDone");
  const taskTodo = t("message.taskTodo");
  const entries = useMemo(
    () => context.map((entry) => ({ entry, html: renderContextMarkdown(entry.bodyMd, { done: taskDone, todo: taskTodo }) })),
    [context, taskDone, taskTodo],
  );
  const hintShown = appendTargetId !== null && context.some((entry) => entry.id === appendTargetId);

  return (
    <div className="capture-shell" ref={shell}>
      <div className="capture-panel" role="group" aria-label={t("capture.label")} ref={root}>
        {entries.length > 0 ? (
          <div className="capture-context">
            <ul className="capture-context__list" aria-label={t("capture.context")}>
              {entries.map(({ entry, html }) => (
                <li className="capture-context__item" key={entry.id}>
                  <article className="capture-entry">
                    <time className="capture-entry__time" dateTime={new Date(entry.createdAt).toISOString()}>
                      {formatTime(new Date(entry.createdAt), language, entry.createdTz)}
                    </time>
                    <div className="markdown capture-entry__text" dangerouslySetInnerHTML={{ __html: html }} />
                  </article>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        <Composer
          value={value}
          onChange={onChange}
          onSubmit={onSend}
          onAppend={onAppend}
          buttons={emoji ? { emoji: true } : {}}
          emoji={
            emoji
              ? {
                  recent: emoji.recent,
                  language,
                  onPicked: emoji.onPicked,
                  placement: "inline",
                  open: emoji.open,
                  onOpenChange: emoji.onOpenChange,
                }
              : undefined
          }
          error={error}
          floating
          hint={hintShown ? t("capture.appendHint") : undefined}
          autoFocus
        />
      </div>
    </div>
  );
}
