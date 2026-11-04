import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useLanguage } from "../../i18n/I18n";
import { summaryText } from "../attachments/summaryText";
import { formatLongDate } from "../feed/dates";
import "./reply.css";

interface ReplyQuoteProps {
  quote: ReplyTo;
  /** Click on the quote: the feed opens the old entry in context. Without a handler, for an entry in the trash and a deleted one the quote is not clickable. */
  onOpen?: (quote: ReplyTo) => void;
}

/**
 * Reply quote in the bubble (slice 5): one line, a label ("3 years later · 31 October 2023", or "Reply to the entry of
 * 14 March 2023" when the difference is less than a year) and the start of the old entry. An old entry in the trash gives "Entry in the trash", deleted for good gives
 * "Original entry deleted"; the reply itself stays. A fill without a border, nothing moves.
 */
export function ReplyQuote({ quote, onOpen }: ReplyQuoteProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const textId = useId();
  const date = quote.journalDate === null ? "" : formatLongDate(quote.journalDate, language);
  const years = quote.yearsLater;
  const live = quote.state === "live";
  const label = !live
    ? t(quote.state === "trash" ? "reply.trash" : quote.state === "moved" ? "reply.moved" : "reply.gone")
    : years > 0
      ? t("reply.quoteYears", { count: years, date })
      : t("reply.quotePlain", { date });
  // For the screen reader: "3 years later, reply to the entry of 31 October 2023"; the start of the old entry is read as a description.
  const spoken = !live ? label : years > 0 ? t("reply.quoteYearsLabel", { count: years, date }) : t("reply.quotePlainLabel", { date });
  // An old entry with no text is described by what it holds: "Photo", "3 photos", a file name (slice 7).
  const preview = quote.excerpt !== "" ? quote.excerpt : summaryText(quote.attachments, t);
  const text = live && preview !== "" ? (
    <span className="reply-quote__text" id={textId}>
      {preview}
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
