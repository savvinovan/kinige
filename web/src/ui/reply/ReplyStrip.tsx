import { useTranslation } from "react-i18next";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useLanguage } from "../../i18n/I18n";
import { IconButton } from "../controls/IconButton";
import { formatLongDate } from "../feed/dates";
import { Icon } from "../icons/Icon";
import "./reply.css";

/** Подпись строки, когда записи, на которую отвечают, нет на месте. */
const NOT_LIVE_LABEL = { trash: "reply.trash", missing: "reply.gone", moved: "reply.moved", live: "reply.strip" } as const;

interface ReplyStripProps {
  quote: ReplyTo;
  /** Крестик: ответ отменяется, остаётся обычная запись (текст не трогается). */
  onCancel: () => void;
}

/**
 * Строка «Ответ на запись · 31 октября 2023» над полем ввода (слайс 5): дата и первая строка записи, справа крестик. Если за
 * это время запись ушла в корзину или удалена навсегда, строка так и говорит; отправить ответ тогда нельзя (Rust вернёт
 * `reply_target_not_found`), но текст остаётся.
 */
export function ReplyStrip({ quote, onCancel }: ReplyStripProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const date = quote.journalDate === null ? "" : formatLongDate(quote.journalDate, language);
  const live = quote.state === "live";
  const label = !live ? t(NOT_LIVE_LABEL[quote.state]) : t("reply.strip", { date });
  return (
    <div className="reply-strip" role="group" aria-label={live ? t("reply.stripLabel", { date }) : label}>
      <span className="reply-strip__icon">
        <Icon name="reply" />
      </span>
      <span className="reply-strip__label">{label}</span>
      {live && quote.excerpt !== "" ? <span className="reply-strip__text">{quote.excerpt}</span> : null}
      <IconButton small icon="close" label={t("reply.cancel")} onClick={onCancel} />
    </div>
  );
}
