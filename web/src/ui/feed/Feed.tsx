import { useMemo } from "react";
import { Message } from "../message/Message";
import { type FeedMessage, buildFeed } from "./buildFeed";
import "./feed.css";

interface FeedProps {
  messages: readonly FeedMessage[];
  /** «Сейчас» для меток «Сегодня» и «Вчера». В историях фиксируется, чтобы снимки были стабильны. */
  now: Date;
  onRetry?: (messageId: string) => void;
}

/**
 * Лента: метки дней («таблетки» по центру) и плашки сообщений, прижатые влево. Открывается прокрученной к низу
 * (новое внизу): область прокрутки `column-reverse` держит нижний край на месте, даже когда после загрузки шрифтов
 * меняется высота строк; короткая лента лежит у нижнего края, над composer. Над лентой нет ни заголовка, ни названия чата (§6).
 */
export function Feed({ messages, now, onRetry }: FeedProps) {
  const items = useMemo(() => buildFeed(messages, now), [messages, now]);

  return (
    // Прокручиваемая область берёт фокус с клавиатуры, иначе без мыши ленту не пролистать.
    <div className="feed-scroll" tabIndex={0} role="region" aria-label="Записи">
      {items.length === 0 ? (
        <p className="feed__empty">Пока пусто. Напишите первую заметку внизу.</p>
      ) : (
        <ol className="feed">
          {items.map((item) =>
            item.kind === "day" ? (
              <li className="feed__item feed__item--day" key={item.key}>
                <h2 className="day-label">{item.label}</h2>
              </li>
            ) : (
              <li className={`feed__item${item.inSeries ? " feed__item--series" : ""}`} key={item.key}>
                <Message
                  markdown={item.message.markdown}
                  sentAt={item.message.sentAt}
                  status={item.message.status}
                  highlighted={item.message.highlighted}
                  onRetry={onRetry ? () => onRetry(item.message.id) : undefined}
                />
              </li>
            ),
          )}
        </ol>
      )}
    </div>
  );
}
