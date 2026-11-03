import { useTranslation } from "react-i18next";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useLanguage } from "../../i18n/I18n";
import { summaryText } from "../attachments/summaryText";
import { IconButton } from "../controls/IconButton";
import { formatLongDate } from "../feed/dates";
import { Icon } from "../icons/Icon";
import "./reply.css";

/** The line label when the entry being replied to is not in place. */
const NOT_LIVE_LABEL = { trash: "reply.trash", missing: "reply.gone", moved: "reply.moved", live: "reply.strip" } as const;

interface ReplyStripProps {
  quote: ReplyTo;
  /** The cross: the reply is cancelled, an ordinary entry remains (the text is not touched). */
  onCancel: () => void;
}

/**
 * The line "Reply to the entry of 31 October 2023" above the input field (slice 5): the date and the first line of the entry, a cross on the right. If in
 * the meantime the entry went to the trash or was deleted for good, the line says so; the reply cannot be sent then (Rust returns
 * `reply_target_not_found`), but the text stays.
 */
export function ReplyStrip({ quote, onCancel }: ReplyStripProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const date = quote.journalDate === null ? "" : formatLongDate(quote.journalDate, language);
  const live = quote.state === "live";
  const label = !live ? t(NOT_LIVE_LABEL[quote.state]) : t("reply.strip", { date });
  // An old entry with no text is described by what it holds: "Photo", "3 photos", a file name (slice 7).
  const preview = quote.excerpt !== "" ? quote.excerpt : summaryText(quote.attachments, t);
  return (
    <div className="reply-strip" role="group" aria-label={live ? t("reply.stripLabel", { date }) : label}>
      <span className="reply-strip__icon">
        <Icon name="reply" />
      </span>
      <span className="reply-strip__label">{label}</span>
      {live && preview !== "" ? <span className="reply-strip__text">{preview}</span> : null}
      <IconButton small icon="close" label={t("reply.cancel")} onClick={onCancel} />
    </div>
  );
}
