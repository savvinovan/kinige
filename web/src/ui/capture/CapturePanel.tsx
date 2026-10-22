import { useEffect, useId, useMemo, useRef } from "react";
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
  /** Запись, к которой ⌘Enter допишет текст; у неё стоит подсказка «⌘Enter — дописать». `null` — подсказки нет. */
  appendTargetId: string | null;
  value: string;
  onChange: (value: string) => void;
  /** Enter. */
  onSend: () => void;
  /** ⌘Enter (на не-macOS Ctrl+Enter). */
  onAppend: () => void;
  /** Esc в любом месте окна. */
  onHide: () => void;
  /** Уже переведённый текст ошибки; показывается над полем, текст в поле остаётся. */
  error?: string;
}

/**
 * Содержимое окна быстрого захвата (слайс 2): сверху до трёх бледных плашек контекста (время, Markdown свёрстан, длинные
 * обрезаны по строкам, ссылки не нажимаются), под ними поле ввода как в главном окне, но без кнопок «файл», «эмодзи» и «микрофон».
 * Окно без заголовка, панель занимает его целиком. Ни тостов, ни подтверждений.
 *
 * Контент не прыгает: подсказка «⌘Enter — дописать» стоит в строке под плашкой (строка есть у каждой плашки), место под ошибку
 * над полем занято всегда. Пока в поле длинный текст, контекст сжимается сверху: старые плашки уходят за край, поле не сдвигается.
 *
 * Окно создаётся скрытым и показывается много раз, поэтому автофокуса при монтировании мало: при каждом получении фокуса
 * окном курсор снова ставится в поле. Esc скрывает окно, где бы ни стоял фокус (кроме ввода через IME).
 */
export function CapturePanel({ context, appendTargetId, value, onChange, onSend, onAppend, onHide, error }: CapturePanelProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const hintId = useId();
  const root = useRef<HTMLDivElement>(null);
  // Свежий обработчик без переподписки: родитель может передавать новую функцию на каждый рендер.
  const onHideRef = useRef(onHide);
  useEffect(() => {
    onHideRef.current = onHide;
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing) return;
      event.preventDefault();
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

  const taskDone = t("message.taskDone");
  const taskTodo = t("message.taskTodo");
  const entries = useMemo(
    () => context.map((entry) => ({ entry, html: renderContextMarkdown(entry.bodyMd, { done: taskDone, todo: taskTodo }) })),
    [context, taskDone, taskTodo],
  );
  const hintShown = appendTargetId !== null && context.some((entry) => entry.id === appendTargetId);

  return (
    <div className="capture-panel" role="group" aria-label={t("capture.label")} ref={root}>
      <div className={`capture-context${entries.length === 0 ? " capture-context--empty" : ""}`}>
        {entries.length === 0 ? (
          <p className="capture-context__empty">{t("capture.empty")}</p>
        ) : (
          <ul className="capture-context__list" aria-label={t("capture.context")}>
            {entries.map(({ entry, html }) => (
              <li className="capture-context__item" key={entry.id}>
                <article className="capture-entry">
                  <time className="capture-entry__time" dateTime={new Date(entry.createdAt).toISOString()}>
                    {formatTime(new Date(entry.createdAt), language, entry.createdTz)}
                  </time>
                  <div className="markdown capture-entry__text" dangerouslySetInnerHTML={{ __html: html }} />
                </article>
                {/* Строка под плашкой есть у каждой записи: подсказка появляется в готовом месте, ничего не сдвигая. */}
                <div className="capture-entry__hint" id={entry.id === appendTargetId ? hintId : undefined}>
                  {entry.id === appendTargetId ? t("capture.appendHint") : null}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
      <Composer
        value={value}
        onChange={onChange}
        onSubmit={onSend}
        onAppend={onAppend}
        buttons={{}}
        error={error}
        reserveErrorSpace
        describedBy={hintShown ? hintId : undefined}
        autoFocus
      />
    </div>
  );
}
