import { useId, useState } from "react";
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
import { ReactionPicker, type PickerAnchor } from "../reactions/ReactionPicker";
import { ReactionRow } from "../reactions/ReactionRow";
import { useReactionChoices } from "../reactions/ReactionsContext";
import "./on-this-day.css";

/** Put (`on`) or remove a reaction on a card entry. */
type CardReact = (messageId: string, emoji: string, on: boolean) => void;

interface OnThisDayCardProps {
  card: OnThisDay;
  /** Open an entry in the feed in context (a click on an entry, "N more", an old-year row). */
  onOpen: (messageId: string) => void;
  /** "Reply" on a card entry. */
  onReply: (entry: OnThisDayEntry) => void;
  /** The cross: hide until the next journal day. */
  onDismiss: () => void;
  /** Slice 5a: the smile button next to "Reply" and removable chips. Without it the card shows the chips as labels and has no button. */
  onReact?: CardReact;
}

/**
 * The "On this day" card (slice 5): entries of the same date in past years, from newer years to older. The three newest years in detail (up to two
 * entries, text up to three lines, time, "Reply"), below that "N more entries of this day >"; older years get one row each, "31 октября 2019 ·
 * 5 записей >". A filled card, no border or shadow; "Reply" appears on hover and focus (no motion).
 */
export function OnThisDayCard({ card, onOpen, onReply, onDismiss, onReact }: OnThisDayCardProps) {
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
        <Year key={year.journalDate} year={year} language={language} onOpen={onOpen} onReply={onReply} onReact={onReact} />
      ))}
    </section>
  );
}

/**
 * The smile button next to "Reply" (slice 5a): it opens the small popover with the three quick emoji and "More". The chip it puts appears under the
 * entry in the card (the owner reads the card again) and in the feed when the entry is opened. Not drawn for an entry that cannot take reactions.
 */
function EntryReaction({ entry, date, onReact }: { entry: OnThisDayEntry; date: string; onReact?: CardReact }) {
  const { t } = useTranslation();
  const language = useLanguage();
  const { quick, recent } = useReactionChoices();
  const [anchor, setAnchor] = useState<PickerAnchor | null>(null);
  const { message } = entry;
  if (!onReact || message.readOnly) return null;
  const state = { reactions: message.reactions, keysHad: message.reactionKeysHad };
  return (
    <>
      <IconButton
        small
        icon="reactionAdd"
        className="on-this-day__react"
        data-react-entry={message.id}
        label={t("onThisDay.reactLabel", { date })}
        aria-haspopup="dialog"
        onClick={(event) => setAnchor(event.currentTarget.getBoundingClientRect())}
      />
      {anchor ? (
        <ReactionPicker
          anchor={anchor}
          start="quick"
          quick={quick}
          recent={recent}
          state={state}
          language={language}
          onReact={(emoji, on) => onReact(message.id, emoji, on)}
          onClose={(returnFocus) => {
            setAnchor(null);
            // The button is looked up now: WebKit does not focus a button on click, and the card may have been redrawn meanwhile.
            if (returnFocus) document.querySelector<HTMLElement>(`[data-react-entry="${message.id}"]`)?.focus();
          }}
        />
      ) : null}
    </>
  );
}

function Year({
  year,
  language,
  onOpen,
  onReply,
  onReact,
}: {
  year: OnThisDayYear;
  language: Language;
  onOpen: (messageId: string) => void;
  onReply: (entry: OnThisDayEntry) => void;
  onReact?: CardReact;
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
              <EntryReaction entry={entry} date={date} onReact={onReact} />
              <IconButton small icon="reply" className="on-this-day__reply" label={t("onThisDay.replyLabel", { date })} onClick={() => onReply(entry)} />
              <ReactionRow
                reactions={entry.message.reactions}
                outside
                onRemove={onReact ? (emoji) => onReact(entry.message.id, emoji, false) : undefined}
              />
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
