import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useLanguage } from "../../i18n/I18n";
import { formatLongDate } from "../feed/dates";
import "./reply.css";

interface ReplyQuoteProps {
  quote: ReplyTo;
  /** Щелчок по цитате: лента открывает старую запись в контексте. Без обработчика, у записи в корзине и у удалённой цитата не нажимается. */
  onOpen?: (quote: ReplyTo) => void;
}

/**
 * Цитата ответа в плашке (слайс 5): одна строка — подпись («3 года спустя · 31 октября 2023», при разнице меньше года «Ответ на
 * запись · 14 марта 2023») и начало старой записи. Старая запись в корзине — «Запись в корзине», удалена навсегда —
 * «Исходная запись удалена»; сам ответ остаётся. Заливка без рамки, ничего не двигается.
 */
export function ReplyQuote({ quote, onOpen }: ReplyQuoteProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const textId = useId();
  const date = quote.journalDate === null ? "" : formatLongDate(quote.journalDate, language);
  const years = quote.yearsLater;
  const live = quote.state === "live";
  const label = !live
    ? t(quote.state === "trash" ? "reply.trash" : "reply.gone")
    : years > 0
      ? t("reply.quoteYears", { count: years, date })
      : t("reply.quotePlain", { date });
  // Для диктора: «3 года спустя, ответ на запись 31 октября 2023»; начало старой записи читается как описание.
  const spoken = !live ? label : years > 0 ? t("reply.quoteYearsLabel", { count: years, date }) : t("reply.quotePlainLabel", { date });
  const text = live && quote.excerpt !== "" ? (
    <span className="reply-quote__text" id={textId}>
      {quote.excerpt}
    </span>
  ) : null;
  const body = (
    <>
      <span className="reply-quote__label">{label}</span>
      {text}
    </>
  );
  if (live && onOpen) {
    return (
      <button type="button" className="reply-quote reply-quote--link" aria-label={spoken} aria-describedby={text ? textId : undefined} onClick={() => onOpen(quote)}>
        {body}
      </button>
    );
  }
  return <div className="reply-quote">{body}</div>;
}
