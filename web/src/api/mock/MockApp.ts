import { KinigeApiError } from "../KinigeApi";
import type { AppInfo } from "../generated/AppInfo";
import type { Language } from "../generated/Language";
import type { RestoreOutcome } from "../generated/RestoreOutcome";
import type { StartupState } from "../generated/StartupState";
import type { MockState } from "./MockState";

const ALLOWED_SCHEMES = ["http:", "https:", "mailto:"];

/** The app itself in `MockApi`: its name and language, links to the system browser, and the start-up recovery from a backup. */
export class MockApp {
  /** Links that were "opened in the system browser". */
  readonly openedUrls: string[] = [];
  /** Restoring from a backup was requested: the backup name. */
  restoredFrom: string | null = null;
  /** A restart for the restore was requested. */
  restartRequested = false;
  private readonly corruptListeners = new Set<() => void>();

  constructor(private readonly s: MockState) {}

  appInfo(): Promise<AppInfo> {
    return Promise.resolve({ title: "Kinige mock" });
  }

  getLanguage(): Promise<Language> {
    return Promise.resolve(this.s.language);
  }

  openExternalUrl(url: string): Promise<void> {
    let scheme = "";
    try {
      scheme = new URL(url).protocol;
    } catch {
      // a relative or empty address
    }
    if (!ALLOWED_SCHEMES.includes(scheme)) return Promise.reject(new KinigeApiError("link_not_allowed"));
    this.openedUrls.push(url);
    return Promise.resolve();
  }

  startupState(): Promise<StartupState> {
    return Promise.resolve(this.s.startup);
  }

  restoreBackup(name: string): Promise<RestoreOutcome> {
    return this.s.run("restoreBackup", name, () => {
      if (!this.s.startup.backups.some((b) => b.name === name)) throw new KinigeApiError("backup_not_found");
      this.restoredFrom = name;
      const folder = this.s.settings.mirror.folder;
      const previousMirror = folder ? `${folder}-before-restore-20261021T061200Z` : null;
      this.s.startup = { needsRecovery: false, backups: [], databaseTooNew: false, corruptionFound: false };
      return { previousMirror, settingsFileMirror: null };
    });
  }

  onDatabaseCorrupt(handler: () => void): Promise<() => void> {
    this.corruptListeners.add(handler);
    return Promise.resolve(() => {
      this.corruptListeners.delete(handler);
    });
  }

  /** Like the "integrity check found corruption" event. */
  emitDatabaseCorrupt(): void {
    for (const handler of [...this.corruptListeners]) handler();
  }

  restartForRecovery(): Promise<void> {
    this.restartRequested = true;
    return Promise.resolve();
  }
}
