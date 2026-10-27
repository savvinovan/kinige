import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { Settings } from "../../api/generated/Settings";
import type { StartupState } from "../../api/generated/StartupState";

/** Настройки, пока журнал не открыт (база повреждена): читать их не из чего, окно «База повреждена» от них не зависит. */
export const RECOVERY_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: null, isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [],
  sidebarCollapsed: false,
  recentEmoji: [],
  backup: { folder: null, lastAt: null, count: 0, error: null },
};

/**
 * Что нужно окну до первого кадра: настройки и состояние запуска. Журнал не открыт (нужно восстановление или база новее
 * приложения) — настройки читать не из чего. База открылась, но чтение настроек само упёрлось в повреждение (порча в
 * страницах, которой открытие не видит) — это то же «повреждение найдено»: окно «База повреждена», а не пустая страница.
 * Любая другая ошибка чтения — наверх, её покажет `main.tsx`.
 */
export async function loadStartup(
  api: KinigeApi,
  startup: StartupState,
): Promise<{ settings: Settings; startup: StartupState }> {
  if (startup.needsRecovery || startup.databaseTooNew) return { settings: RECOVERY_SETTINGS, startup };
  try {
    return { settings: await api.getSettings(), startup };
  } catch (reason) {
    if (errorCode(reason) !== "database_corrupt") throw reason;
    return { settings: RECOVERY_SETTINGS, startup: { ...startup, corruptionFound: true } };
  }
}
