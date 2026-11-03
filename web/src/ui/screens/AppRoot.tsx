import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Settings } from "../../api/generated/Settings";
import type { StartupState } from "../../api/generated/StartupState";
import { useLanguage } from "../../i18n/I18n";
import { RecoveryScreen } from "../recovery/RecoveryScreen";
import { TooNewScreen } from "../recovery/TooNewScreen";
import { useInputModality } from "../useInputModality";
import { MainWindow } from "./MainWindow";

interface AppRootProps {
  api: KinigeApi;
  initialSettings: Settings;
  /** The startup state: the journal is open or a restore from a backup is needed. */
  startup: StartupState;
}

type Recovery = { mode: "startup" | "running"; backups: readonly BackupInfo[] };

/**
 * Root of the main window (slice 3): the journal or the "Database is damaged" window. The recovery window appears at
 * startup (the database does not open) and on a Rust event, if the background integrity check found damage in the
 * running database (then a restart is needed: a running database cannot be swapped).
 */
export function AppRoot({ api, initialSettings, startup }: AppRootProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  useInputModality();
  const [recovery, setRecovery] = useState<Recovery | null>(
    startup.needsRecovery
      ? { mode: "startup", backups: startup.backups }
      : startup.corruptionFound
        ? { mode: "running", backups: [] }
        : null,
  );
  const [settings, setSettings] = useState(initialSettings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorCode | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    let closed = false;
    const found = () => setRecovery((old) => old ?? { mode: "running", backups: [] });
    api
      .onDatabaseCorrupt(found)
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
        // The integrity check runs right at startup and could find damage before the page subscribed to the event
        // (found in a live check): the startup state keeps a flag, so we ask it again after subscribing.
        return api.startupState();
      })
      .then((state) => {
        if (!closed && state.corruptionFound) found();
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api]);

  // The database was created by a newer version (§10.4): the journal is not open, backups are not offered.
  if (startup.databaseTooNew) return <TooNewScreen />;
  if (!recovery) return <MainWindow api={api} initialSettings={settings} />;

  const restore = (name: string) => {
    setBusy(true);
    setError(null);
    api
      .restoreBackup(name)
      // Rust opened the database from the backup and started the services: settings are now read for real. Maintenance of
      // the restored database could find damage in it during the restore itself: the event then reached the "startup"
      // window and was ignored, so the state is asked again (otherwise the journal would open on a database declared damaged).
      .then(() => Promise.all([api.getSettings(), api.startupState()]))
      .then(([fresh, state]) => {
        setSettings(fresh);
        setRecovery(state.corruptionFound ? { mode: "running", backups: [] } : null);
      })
      .catch((reason: unknown) => {
        // The restored backup is damaged where `quick_check` does not look (the settings page): the store has already been
        // handed to the app, "Restore" cannot be done again -- as with reading the first settings (`loadStartup`), the window says "restart".
        if (errorCode(reason) === "database_corrupt") setRecovery({ mode: "running", backups: [] });
        else setError(errorCode(reason));
      })
      .finally(() => setBusy(false));
  };

  return (
    <RecoveryScreen
      mode={recovery.mode}
      backups={recovery.backups}
      language={language}
      busy={busy}
      error={error ? t(`error.${error}`) : undefined}
      onRestore={restore}
      onRestart={() => {
        setBusy(true);
        api.restartForRecovery().catch((reason: unknown) => {
          setError(errorCode(reason));
          setBusy(false);
        });
      }}
    />
  );
}
