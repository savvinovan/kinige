import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Message } from "../message/Message";
import { type FeedMessage, buildFeed } from "./buildFeed";
import { formatDayLabel } from "./dates";
import "./feed.css";

interface FeedProps {
  messages: readonly FeedMessage[];
  /** Сегодняшний `journal_date` (`YYYY-MM-DD`) для меток «Сегодня» и «Вчера»: его присылает Rust. В историях фиксируется, чтобы снимки были стабильны. */
  today: string;
  onRetry?: (messageId: string) => void;
  /** Щелчок по ссылке в записи → системный браузер. */
  onOpenLink?: (url: string) => void;
}

/**
 * Лента: метки дней («таблетки» по центру) и плашки сообщений, прижатые влево. Открывается прокрученной к низу
 * (новое внизу): область прокрутки `column-reverse` держит нижний край на месте, даже когда после загрузки шрифтов
 * меняется высота строк; короткая лента лежит у нижнего края, над composer. Над лентой нет ни заголовка, ни названия чата (§6).
 */
export function Feed({ messages, today, onRetry, onOpenLink }: FeedProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const items = useMemo(() => buildFeed(messages), [messages]);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };

  return (
    // Прокручиваемая область берёт фокус с клавиатуры, иначе без мыши ленту не пролистать.
    <div className="feed-scroll" tabIndex={0} role="region" aria-label={t("feed.label")}>
      {items.length === 0 ? (
        <p className="feed__empty">{t("feed.empty")}</p>
      ) : (
        <ol className="feed">
          {items.map((item) =>
            item.kind === "day" ? (
              <li className="feed__item feed__item--day" key={item.key}>
                <h2 className="day-label">{formatDayLabel(item.journalDate, today, language, labels)}</h2>
              </li>
            ) : (
              <li className={`feed__item${item.inSeries ? " feed__item--series" : ""}`} key={item.key}>
                <Message
                  markdown={item.message.bodyMd}
                  sentAt={new Date(item.message.createdAt)}
                  timeZone={item.message.createdTz}
                  status={item.message.status}
                  highlighted={item.message.highlighted}
                  onRetry={onRetry ? () => onRetry(item.message.id) : undefined}
                  onOpenLink={onOpenLink}
                />
              </li>
            ),
          )}
        </ol>
      )}
    </div>
  );
}
