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
        // Проверка целостности идёт сразу при старте и могла найти повреждение раньше, чем страница подписалась на событие
        // (найдено в живой проверке): состояние запуска хранит отметку, спрашиваем его ещё раз уже после подписки.
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

  // База создана более новой версией (§10.4): журнал не открыт, копии не предлагаются.
  if (startup.databaseTooNew) return <TooNewScreen />;
  if (!recovery) return <MainWindow api={api} initialSettings={settings} />;

  const restore = (name: string) => {
    setBusy(true);
    setError(null);
    api
      .restoreBackup(name)
      // Rust открыл базу из копии и запустил сервисы: настройки читаются уже по-настоящему. Обслуживание восстановленной
      // базы могло найти в ней повреждение ещё во время восстановления: событие тогда пришло в окно «startup» и было
      // проигнорировано, поэтому состояние спрашивается заново (иначе журнал открылся бы на базе, объявленной повреждённой).
      .then(() => Promise.all([api.getSettings(), api.startupState()]))
      .then(([fresh, state]) => {
        setSettings(fresh);
        setRecovery(state.corruptionFound ? { mode: "running", backups: [] } : null);
      })
      .catch((reason: unknown) => {
        // Восстановленная копия повреждена там, куда `quick_check` не заглядывает (страница настроек): хранилище уже отдано
        // приложению, снова «Восстановить» нельзя — как и при чтении первых настроек (`loadStartup`), окно «перезапустить».
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
