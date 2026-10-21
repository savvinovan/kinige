import { useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { renderMarkdown } from "./markdown";
import { MessageActions } from "./MessageActions";
import { useActionsPlacement } from "./useActionsPlacement";
import "./message.css";
import "./markdown.css";

export interface MessageProps {
  /** Текст сообщения в Markdown. Сырой HTML в нём не рендерится (§6). */
  markdown: string;
  /** Когда отправлено; время показывается в 24-часовом формате в поясе записи (`timeZone`), без него — в поясе окна. */
  sentAt: Date;
  /** IANA-пояс, в котором запись создана (`created_tz`). */
  timeZone?: string;
  /** «failed» — не отправилось: под текстом строка «Не отправилось. Повторить». */
  status?: "sent" | "failed";
  /** Подсвеченное сообщение (найдено поиском, переход по ссылке): другая заливка плашки. */
  highlighted?: boolean;
  onRetry?: () => void;
  /** Щелчок по ссылке в тексте: адрес уходит в системный браузер (команда `open_external_url`). Без обработчика ссылка ничего не открывает. */
  onOpenLink?: (url: string) => void;
  /** Показать ряд действий без наведения (для историй и снимков состояния «наведение»). */
  showActions?: boolean;
}

/**
 * Сообщение — плашка без хвоста и аватара; время мелко в правом нижнем углу на одной строке с концом текста.
 * Промежуток до соседних плашек задаёт лента (серия — меньше), здесь только сама плашка.
 */
export function Message({
  markdown,
  sentAt,
  timeZone,
  status = "sent",
  highlighted = false,
  onRetry,
  onOpenLink,
  showActions = false,
}: MessageProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const failed = status === "failed";
  const taskDone = t("message.taskDone");
  const taskTodo = t("message.taskTodo");
  const html = useMemo(
    () => renderMarkdown(markdown, { timeSpacer: !failed, taskLabels: { done: taskDone, todo: taskTodo } }),
    [markdown, failed, taskDone, taskTodo],
  );
  // Сообщение кончается блоком (код): распорки у последней строки нет, время стоит под блоком.
  const timeBelow = !failed && !html.includes("message__spacer");

  // Webview не уходит по ссылке: щелчок мышью, Enter на ссылке с клавиатуры (браузер превращает его в click) и средняя
  // кнопка (auxclick) отменяются; адрес открывает системный браузер через `open_external_url` (проверка схемы — в Rust).
  const handleLinkClick = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) return;
    const link = event.target.closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("href");
    // Среднюю кнопку не открываем: её щелчок «отменяется» как переход, а открытие — только по основному (click).
    if (event.type === "click" && href) onOpenLink?.(href);
  };

  // Ряд действий: справа от плашки; в узком окне — чуть выше плашки у её правого края; у верхнего края ленты — внутри.
  // Виден, пока мышь в строке ленты (и ещё короткую задержку после ухода). Подробности — в useActionsPlacement.
  const { wrap, inside, flip, shown } = useActionsPlacement(html);

  const classes = ["message"];
  if (highlighted) classes.push("message--highlighted");
  if (timeBelow) classes.push("message--time-below");

  return (
    <div
      className={[
        "message-wrap",
        showActions || shown ? "message-wrap--actions" : "",
        inside ? "message-wrap--actions-inside" : "",
        inside && flip ? "message-wrap--actions-flip" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      ref={wrap}
    >
      <article className={classes.join(" ")}>
        <div
          className="markdown"
          onClick={handleLinkClick}
          onAuxClick={handleLinkClick}
          dangerouslySetInnerHTML={{ __html: html }}
        />
        {failed ? (
          <p className="message__error">
            {t("message.notSent")}{" "}
            <Button variant="inline" onClick={onRetry} aria-label={t("message.retryLabel")}>
              {t("message.retry")}
            </Button>
          </p>
        ) : null}
        <time className="message__time" dateTime={sentAt.toISOString()}>
          {formatTime(sentAt, language, timeZone)}
        </time>
      </article>
      <MessageActions markdown={markdown} />
    </div>
  );
}
