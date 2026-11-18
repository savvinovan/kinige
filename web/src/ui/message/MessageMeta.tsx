import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { Icon } from "../icons/Icon";

interface MessageMetaProps {
  sentAt: Date;
  timeZone?: string;
  mode: "view" | "editing" | "deleted";
  favorite: boolean;
  byAgent: boolean;
  onRestore?: () => void;
}

/**
 * The bottom right corner of a bubble: the "favorite" mark, the agent mark and the time (for a deleted bubble, "Deleted · Restore").
 * It sits at the end of the last text line; on a picture that fills the bubble (`message--bleed-pill`) the same line is drawn as a pill.
 */
export function MessageMeta({ sentAt, timeZone, mode, favorite, byAgent, onRestore }: MessageMetaProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  return (
    <div className="message__meta">
      {mode === "deleted" ? (
        <span className="message__deleted">
          {t("message.deleted")} ·{" "}
          <Button variant="inline" onClick={onRestore} aria-label={t("message.restoreLabel")}>
            {t("message.restore")}
          </Button>
        </span>
      ) : (
        <>
          {favorite ? (
            <span className="message__mark" role="img" aria-label={t("message.favoriteMark")}>
              <Icon name="favorite" />
            </span>
          ) : null}
          {byAgent ? <span className="message__origin">{t("message.byAgent")} · </span> : null}
          <time className="message__time" dateTime={sentAt.toISOString()}>
            {formatTime(sentAt, language, timeZone)}
          </time>
        </>
      )}
    </div>
  );
}
