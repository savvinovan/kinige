import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { Settings } from "../../api/generated/Settings";
import type { StartupState } from "../../api/generated/StartupState";

/** Settings while the journal is not open (the database is damaged): there is nothing to read them from, the "Database is damaged" window does not depend on them. */
export const RECOVERY_SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: null, isDefault: true, error: null, pendingDays: 0 },
  shortcuts: [],
  sidebarCollapsed: false,
  forestView: "strip",
  forestMode: "forest",
  recentEmoji: [],
  backup: { folder: null, lastAt: null, count: 0, error: null },
};

/**
 * What the window needs before the first frame: the settings and the startup state. The journal is not open (a restore is
 * needed or the database is newer than the app) -- there is nothing to read the settings from. The database opened, but
 * reading the settings itself hit damage (corruption in pages that opening does not see) -- that is the same "damage
 * found": the "Database is damaged" window, not an empty page. Any other read error goes up, `main.tsx` will show it.
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
