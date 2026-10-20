import { useMemo } from "react";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { renderMarkdown } from "./markdown";
import "./message.css";
import "./markdown.css";

export interface MessageProps {
  /** Текст сообщения в Markdown. Сырой HTML в нём не рендерится (§6). */
  markdown: string;
  /** Когда отправлено; время показывается в 24-часовом формате в местном поясе. */
  sentAt: Date;
  /** «failed» — не отправилось: под текстом строка «Не отправилось. Повторить». */
  status?: "sent" | "failed";
  /** Подсвеченное сообщение (найдено поиском, переход по ссылке): рамка, заливка не меняется. */
  highlighted?: boolean;
  onRetry?: () => void;
}

/**
 * Сообщение — плашка без хвоста и аватара; время мелко в правом нижнем углу на одной строке с концом текста.
 * Промежуток до соседних плашек задаёт лента (серия — меньше), здесь только сама плашка.
 */
export function Message({ markdown, sentAt, status = "sent", highlighted = false, onRetry }: MessageProps) {
  const failed = status === "failed";
  const html = useMemo(() => renderMarkdown(markdown, { timeSpacer: !failed }), [markdown, failed]);
  // Сообщение кончается блоком (код): распорки у последней строки нет, время стоит под блоком.
  const timeBelow = !failed && !html.includes("message__spacer");

  const classes = ["message"];
  if (highlighted) classes.push("message--highlighted");
  if (timeBelow) classes.push("message--time-below");

  return (
    <article className={classes.join(" ")}>
      <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />
      {failed ? (
        <p className="message__error">
          Не отправилось.{" "}
          <Button variant="inline" onClick={onRetry} aria-label="Повторить отправку сообщения">
            Повторить
          </Button>
        </p>
      ) : null}
      <time className="message__time" dateTime={sentAt.toISOString()}>
        {formatTime(sentAt)}
      </time>
    </article>
  );
}
