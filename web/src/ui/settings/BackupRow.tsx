import { Fragment, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import type { BackupStatus } from "../../api/generated/BackupStatus";
import { useLanguage } from "../../i18n/I18n";
import { formatDayLabel, formatTime } from "../feed/dates";
import { StatusNote } from "./StatusNote";
import "./backup-row.css";
import "./settings.css";

interface BackupRowProps {
  backup: BackupStatus;
  /** "Now" for the day mark ("сегодня", "вчера"); by default the window clock. Passed in stories and tests so that screenshots are stable. */
  now?: Date;
}

const two = (value: number) => String(value).padStart(2, "0");

/** The calendar day by the window's local clock: `YYYY-MM-DD`. */
const localDay = (date: Date) => `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;

// A line break is offered after each path separator (as in the Markdown copy folder row).
const segments = (path: string) => path.split(/(?<=[\\/])/);

/**
 * The database file copy state for "Settings": "Последняя копия: сегодня, 09:12" (or "ещё не снималась"), the number of copies, the copies
 * folder and an error. `integrity_failed` is a prominent warning in an error-colored frame: the check found database corruption,
 * the day's copy is not taken. Other errors are an ordinary error line. All errors are `role="alert"`.
 * The folder is only shown: it cannot be chosen (the copies lie next to the database).
 */
export function BackupRow({ backup, now }: BackupRowProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const folderId = useId();
  const [windowNow] = useState(() => new Date());
  const reference = now ?? windowNow;

  let last = t("settings.backup.never");
  if (backup.lastAt !== null) {
    const at = new Date(backup.lastAt);
    const day = formatDayLabel(localDay(at), localDay(reference), language, {
      today: t("settings.backup.today"),
      yesterday: t("settings.backup.yesterday"),
    });
    last = t("settings.backup.last", { when: `${day}, ${formatTime(at, language)}` });
  }

  const error = backup.error;
  return (
    <div className="backup-row">
      <p className="backup-row__last">{last}</p>
      {backup.count > 0 ? <p className="backup-row__count">{t("settings.backup.count", { count: backup.count })}</p> : null}
      {backup.folder !== null ? (
        <div className="path-row" role="group" aria-labelledby={folderId}>
          <span className="path-row__label" id={folderId}>
            {t("settings.backup.folder")}
          </span>
          <div className="path-row__field">
            <code className="path-row__path" title={backup.folder}>
              {segments(backup.folder).map((part, index) => (
                <Fragment key={index}>
                  {part}
                  <wbr />
                </Fragment>
              ))}
            </code>
          </div>
        </div>
      ) : null}
      {error === "integrity_failed" ? (
        <div className="backup-row__warning">
          <StatusNote tone="error" icon="alert" role="alert">
            {t("settings.backup.error.integrity_failed")}
          </StatusNote>
        </div>
      ) : error !== null ? (
        <StatusNote tone="error" icon="alert" role="alert">
          {t(`settings.backup.error.${error}`)}
        </StatusNote>
      ) : null}
    </div>
  );
}
