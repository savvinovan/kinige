import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { Settings } from "../../../api/generated/Settings";
import { BackupRow } from "../../settings/BackupRow";
import { MirrorStatusLine } from "../../settings/MirrorStatusLine";
import { PathRow } from "../../settings/PathRow";
import { StatusNote } from "../../settings/StatusNote";
import { ChangedByFile } from "../ChangedByFile";
import "../../settings/settings.css";

interface MirrorPageProps {
  settings: Settings;
  /** The system folder picker dialog is open. */
  choosing?: boolean;
  /** The already translated error text of the folder choice. */
  actionError?: string;
  onChooseFolder: () => void;
  changedAt: number | null;
  now?: Date;
}

/** "Копии": the Markdown copy (folder, state, the "plain text" warning) and the database file copy. */
export function MirrorPage({ settings, choosing = false, actionError, onChooseFolder, changedAt, now }: MirrorPageProps) {
  const { t } = useTranslation();
  const titleId = useId();
  const { mirror } = settings;
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.mirror")}
      </h2>
      <div className="prefs-block">
        <h3 className="settings__heading">{t("settings.mirror.title")}</h3>
        <p className="settings__description">{t("settings.mirror.description")}</p>
        <PathRow path={mirror.folder} isDefault={mirror.isDefault} busy={choosing} onChoose={onChooseFolder} />
        {actionError ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {actionError}
          </StatusNote>
        ) : null}
        <MirrorStatusLine mirror={mirror} />
        <StatusNote tone="quiet" icon="info">
          {t("settings.mirror.warning")}
        </StatusNote>
        <ChangedByFile at={changedAt} now={now} />
      </div>
      <div className="prefs-block">
        <h3 className="settings__heading">{t("settings.backup.title")}</h3>
        <p className="settings__description">{t("settings.backup.description")}</p>
        <BackupRow backup={settings.backup} now={now} />
      </div>
    </section>
  );
}
