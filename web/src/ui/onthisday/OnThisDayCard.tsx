import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { OnThisDay } from "../../api/generated/OnThisDay";
import type { OnThisDayEntry } from "../../api/generated/OnThisDayEntry";
import type { OnThisDayYear } from "../../api/generated/OnThisDayYear";
import { useLanguage } from "../../i18n/I18n";
import type { Language } from "../../i18n";
import { Button } from "../controls/Button";
import { IconButton } from "../controls/IconButton";
import { formatLongDate, formatTime } from "../feed/dates";
import "./on-this-day.css";

interface OnThisDayCardProps {
  card: OnThisDay;
  /** Открыть запись в ленте в контексте (щелчок по записи, «ещё N», строка старого года). */
  onOpen: (messageId: string) => void;
  /** «Ответить» у записи карточки. */
  onReply: (entry: OnThisDayEntry) => void;
  /** Крестик: скрыть до следующего дня журнала. */
  onDismiss: () => void;
}

/**
 * Карточка «В этот день» (слайс 5): записи этого же числа прошлых лет, от новых лет к старым. Три новых года подробно (до двух
 * записей, текст до трёх строк, время, «Ответить»), ниже «ещё N записей этого дня ›»; старые годы — по строке «31 октября 2019 ·
 * 5 записей ›». Заливка карточки, без рамки и тени; «Ответить» проявляется при наведении и фокусе (движения нет).
 */
export function OnThisDayCard({ card, onOpen, onReply, onDismiss }: OnThisDayCardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const titleId = useId();
  return (
    <section className="on-this-day" aria-labelledby={titleId}>
      <header className="on-this-day__head">
        <h2 className="on-this-day__title" id={titleId}>
          {t("onThisDay.label")}
        </h2>
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
      <h3 className="on-this-day__year-title">{`${t("onThisDay.yearsAgo", { count: year.yearsAgo })} · ${date}`}</h3>
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
                <span className="on-this-day__text">{entry.text}</span>
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
