import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import type { Language } from "../../api/generated/Language";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { StatusNote } from "../settings/StatusNote";
import "./recovery.css";

interface RecoveryScreenProps {
  /** startup: the database did not open at launch, the copy list and "Восстановить"; running: the check found corruption in a running app. */
  mode: "startup" | "running";
  /** The database copies in any order: the screen puts the newest first itself. */
  backups: readonly BackupInfo[];
  language: Language;
  /** A restore or restart is in progress: the button and the choice are unavailable. */
  busy?: boolean;
  /** The already translated error (for example "Копия не найдена или не открывается"): `role="alert"`. */
  error?: string;
  /** "Восстановить из копии от ...": the file name of the selected copy. */
  onRestore: (name: string) => void;
  onRestart: () => void;
}

/** "27 октября 2026, 09:12": the day and time of the copy in the window's time zone (the Russian " г." after the year is dropped). */
function formatWhen(createdAt: number, language: Language): string {
  const date = new Date(createdAt);
  const parts = new Intl.DateTimeFormat(language, { day: "numeric", month: "long", year: "numeric" }).formatToParts(date);
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return `${parts.map((part) => part.value).join("")}, ${formatTime(date, language)}`;
}

/**
 * The "База повреждена" screen: it takes the whole window and is not modal (the journal is not open). The corrupted file is not deleted but
 * set aside next to the database; an entry made after the last copy may not have been saved.
 *
 * startup: an explanation, the copy list (newest first, a radio group; the copy kind is "копия дня" / "перед обновлением"), the main
 * button "Восстановить из копии от ..." for the selected one (the newest by default). No copies: a quiet message and a hint
 * without a button. running: the explanation "Kinige перезапустится и предложит копии" and the "Перезапустить" button, no list.
 */
export function RecoveryScreen({
  mode,
  backups,
  language,
  busy = false,
  error,
  onRestore,
  onRestart,
}: RecoveryScreenProps) {
  const { t } = useTranslation();
  const sorted = [...backups].sort((a, b) => b.createdAt - a.createdAt);
  const newest = sorted[0];
  const [picked, setPicked] = useState<string | null>(null);
  // The selected copy may have vanished from the list (the list was refreshed): then the newest again.
  const selected = sorted.find((backup) => backup.name === picked) ?? newest;

  return (
    <main className="recovery" aria-labelledby="recovery-title">
      <div className="recovery__column">
        <h1 className="recovery__title" id="recovery-title">
          {t("recovery.title")}
        </h1>
        <p className="recovery__text">{mode === "running" ? t("recovery.runningText") : t("recovery.startupText")}</p>
        {/* The Markdown copy folder: the path depends on the selected copy (it stores it), so the line has no path. */}
        {mode === "startup" && newest !== undefined ? (
          <p className="recovery__text recovery__text--quiet">{t("recovery.mirrorNote")}</p>
        ) : null}

        {mode === "startup" && newest === undefined ? (
          <>
            <p className="recovery__none">{t("recovery.none")}</p>
            <p className="recovery__text recovery__text--quiet">{t("recovery.noneHint")}</p>
          </>
        ) : null}

        {mode === "startup" && selected !== undefined ? (
          <div className="recovery__list" role="radiogroup" aria-label={t("recovery.backups")}>
            {sorted.map((backup) => (
              <label className="recovery__backup" key={backup.name}>
                <input
                  type="radio"
                  className="visually-hidden"
                  name="recovery-backup"
                  value={backup.name}
                  checked={backup.name === selected.name}
                  disabled={busy}
                  autoFocus={backup.name === newest?.name}
                  onChange={() => setPicked(backup.name)}
                />
                <span className="recovery__when">{formatWhen(backup.createdAt, language)}</span>
                <span className="recovery__kind">{t(`recovery.kind.${backup.kind}`)}</span>
              </label>
            ))}
          </div>
        ) : null}

        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {error}
          </StatusNote>
        ) : null}

        {mode === "running" ? (
          <Button
            variant="secondary"
            className="recovery__primary"
            autoFocus
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onClick={() => {
              if (!busy) onRestart();
            }}
          >
            {t("recovery.restart")}
          </Button>
        ) : selected !== undefined ? (
          <Button
            variant="secondary"
            className="recovery__primary"
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onClick={() => {
              if (!busy) onRestore(selected.name);
            }}
          >
            {busy ? t("recovery.restoring") : t("recovery.restore", { when: formatWhen(selected.createdAt, language) })}
          </Button>
        ) : null}
      </div>
    </main>
  );
}
