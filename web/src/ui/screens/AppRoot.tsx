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
import { MainWindow } from "./MainWindow";

/** Настройки, пока журнал не открыт (база повреждена): читать их не из чего, окно «База повреждена» от них не зависит. */
export const RECOVERY_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: null, isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [],
  sidebarCollapsed: false,
  recentEmoji: [],
  backup: { folder: null, lastAt: null, count: 0, error: null },
};

interface AppRootProps {
  api: KinigeApi;
  initialSettings: Settings;
  /** Состояние запуска: журнал открыт или нужно восстановление из копии. */
  startup: StartupState;
}

type Recovery = { mode: "startup" | "running"; backups: readonly BackupInfo[] };

/**
 * Корень главного окна (слайс 3): журнал или окно «База повреждена». Окно восстановления появляется при запуске (база не
 * открывается) и по событию Rust, если проверка целостности в фоне нашла повреждение работающей базы (тогда нужен
 * перезапуск: работающую базу подменять нельзя).
 */
export function AppRoot({ api, initialSettings, startup }: AppRootProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const [recovery, setRecovery] = useState<Recovery | null>(
    startup.needsRecovery ? { mode: "startup", backups: startup.backups } : null,
  );
  const [settings, setSettings] = useState(initialSettings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorCode | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    let closed = false;
    api
      .onDatabaseCorrupt(() => setRecovery({ mode: "running", backups: [] }))
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api]);

  // База создана более новой версией (§10.4): журнал не открыт, копии не предлагаются.
  if (startup.databaseTooNew) return <TooNewScreen />;
  if (!recovery) return <MainWindow api={api} initialSettings={settings} />;

  const restore = (name: string) => {
    setBusy(true);
    setError(null);
    api
      .restoreBackup(name)
      // Rust открыл базу из копии и запустил сервисы: настройки читаются уже по-настоящему.
      .then(() => api.getSettings())
      .then((fresh) => {
        setSettings(fresh);
        setRecovery(null);
      })
      .catch((reason: unknown) => setError(errorCode(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <RecoveryScreen
      mode={recovery.mode}
      backups={recovery.backups}
      mirrorFolder={startup.mirrorFolder}
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
