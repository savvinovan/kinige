import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { OnThisDay } from "../../api/generated/OnThisDay";
import type { OnThisDayEntry } from "../../api/generated/OnThisDayEntry";
import type { OnThisDayYear } from "../../api/generated/OnThisDayYear";
import { useLanguage } from "../../i18n/I18n";
import type { Language } from "../../i18n";
import { summaryOf, summaryText } from "../attachments/summaryText";
import { Button } from "../controls/Button";
import { IconButton } from "../controls/IconButton";
import { formatLongDate, formatTime } from "../feed/dates";
import "./on-this-day.css";

interface OnThisDayCardProps {
  card: OnThisDay;
  /** Open an entry in the feed in context (a click on an entry, "N more", an old-year row). */
  onOpen: (messageId: string) => void;
  /** "Reply" on a card entry. */
  onReply: (entry: OnThisDayEntry) => void;
  /** The cross: hide until the next journal day. */
  onDismiss: () => void;
}

/**
 * The "On this day" card (slice 5): entries of the same date in past years, from newer years to older. The three newest years in detail (up to two
 * entries, text up to three lines, time, "Reply"), below that "N more entries of this day >"; older years get one row each, "31 октября 2019 ·
 * 5 записей >". A filled card, no border or shadow; "Reply" appears on hover and focus (no motion).
 */
export function OnThisDayCard({ card, onOpen, onReply, onDismiss }: OnThisDayCardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const titleId = useId();
  return (
    <section className="on-this-day" aria-labelledby={titleId}>
      <header className="on-this-day__head">
        <h3 className="on-this-day__title" id={titleId}>
          {t("onThisDay.label")}
        </h3>
        <IconButton small icon="close" label={t("onThisDay.hide")} onClick={onDismiss} />
      </header>
      {card.years.map((year) => (
        <Year key={year.journalDate} year={year} language={language} onOpen={onOpen} onReply={onReply} />
      ))}
    </section>
  );
}

function Year({
  year,
  language,
  onOpen,
  onReply,
}: {
  year: OnThisDayYear;
  language: Language;
  onOpen: (messageId: string) => void;
  onReply: (entry: OnThisDayEntry) => void;
}) {
  const { t } = useTranslation();
  const date = formatLongDate(year.journalDate, language);
  const arrow = <span aria-hidden="true"> ›</span>;
  if (year.entries.length === 0) {
    return (
      <div className="on-this-day__year-row">
        <button type="button" className="on-this-day__link" aria-label={t("onThisDay.openDayLabel", { date })} onClick={() => onOpen(year.firstId)}>
          {t("onThisDay.compact", { count: year.total, date })}
          {arrow}
        </button>
      </div>
    );
  }
  const rest = year.total - year.entries.length;
  return (
    <div className="on-this-day__year">
      <h4 className="on-this-day__year-title">{`${t("onThisDay.yearsAgo", { count: year.yearsAgo })} · ${date}`}</h4>
      <ul className="on-this-day__entries">
        {year.entries.map((entry) => {
          const time = formatTime(new Date(entry.message.createdAt), language, entry.message.createdTz);
          return (
            <li className="on-this-day__entry" key={entry.message.id}>
              <button
                type="button"
                className="on-this-day__open"
                aria-label={t("onThisDay.openLabel", { date, time })}
                onClick={() => onOpen(entry.message.id)}
              >
                <span className="on-this-day__text">{entry.text !== "" ? entry.text : summaryText(summaryOf(entry.message.attachments), t)}</span>
                <time className="on-this-day__time" dateTime={new Date(entry.message.createdAt).toISOString()}>
                  {time}
                </time>
              </button>
              <IconButton small icon="reply" className="on-this-day__reply" label={t("onThisDay.replyLabel", { date })} onClick={() => onReply(entry)} />
            </li>
          );
        })}
      </ul>
      {rest > 0 ? (
        <Button variant="inline" className="on-this-day__more" onClick={() => onOpen(year.firstId)}>
          {t("onThisDay.more", { count: rest })}
          {arrow}
        </Button>
      ) : null}
    </div>
  );
}
