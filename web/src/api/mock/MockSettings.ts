import { KinigeApiError } from "../KinigeApi";
import { DEFAULT_SHORTCUTS } from "../mockData";
import type { ErrorCode } from "../generated/ErrorCode";
import type { Settings } from "../generated/Settings";
import type { SettingsFileState } from "../generated/SettingsFileState";
import type { ShortcutKind } from "../generated/ShortcutKind";
import type { TextSize } from "../generated/TextSize";
import type { MockState } from "./MockState";

/** The settings file state with no problems: a path and empty lists. */
export const MOCK_SETTINGS_FILE: SettingsFileState = {
  path: "/Users/me/Library/Application Support/me.savvinov.kinige/settings.json",
  problem: null,
  unsafeReason: null,
  issues: [],
  unknownKeys: [],
  unknownMore: 0,
  changed: { appearance: null, shortcuts: null, mirror: null, access: null },
};

/**
 * Settings in `MockApi`: the text size, the copy folder, the shortcuts, and (slice 4b, ADR-0010) the `settings.json` file state that a
 * test or a story sets as it likes, the "settings window is open" counter and opening an entry from the log.
 */
export class MockSettings {
  /** The next folder choice fails with this error. */
  failNextPick: ErrorCode | null = null;
  /** The next shortcut change or reset fails with this error. */
  failNextShortcut: ErrorCode | null = null;
  /** The next folder the system dialog will "pick"; `null` - the user cancelled. */
  pickedFolder: string | null;
  /** Shortcut recording mode records: `true` at the start, `false` at the end. */
  readonly recordingLog: boolean[] = [];
  /** What `getSettingsFile` returns: a test replaces it fully or partly. */
  settingsFile: SettingsFileState = MOCK_SETTINGS_FILE;
  /** How many times the settings window was asked to open. */
  openSettingsCount = 0;
  /** Entries that the settings window asked to open in the feed: `settingsOpenEntry` sends them to the main window. */
  readonly openedEntries: string[] = [];
  private readonly openMessageListeners = new Set<(target: { chatId: string; messageId: string }) => void>();

  constructor(
    private readonly s: MockState,
    pickedFolder: string | null | undefined,
  ) {
    this.pickedFolder = pickedFolder === undefined ? "/Users/me/Notes/Kinige" : pickedFolder;
  }

  getSettings(): Promise<Settings> {
    return this.s.run("getSettings", "", () => this.s.settings);
  }

  setTextSize(size: TextSize): Promise<void> {
    return this.s.run("setTextSize", size, () => {
      this.s.settings = { ...this.s.settings, textSize: size };
      this.s.emitSettingsChanged();
    });
  }

  chooseMirrorFolder(): Promise<Settings> {
    if (this.failNextPick) {
      const code = this.failNextPick;
      this.failNextPick = null;
      return Promise.reject(new KinigeApiError(code));
    }
    if (this.pickedFolder) {
      this.s.settings = { ...this.s.settings, mirror: { folder: this.pickedFolder, isDefault: false, error: null, pendingDays: 0 } };
      this.s.emitSettingsChanged();
    }
    return Promise.resolve(this.s.settings);
  }

  setShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings> {
    return this.changeShortcut(kind, accelerator);
  }

  resetShortcut(kind: ShortcutKind): Promise<Settings> {
    return this.changeShortcut(kind, DEFAULT_SHORTCUTS[kind]);
  }

  shortcutRecording(active: boolean): Promise<void> {
    this.recordingLog.push(active);
    return Promise.resolve();
  }

  getSettingsFile(): Promise<SettingsFileState> {
    return this.s.run("getSettingsFile", "", () => this.settingsFile);
  }

  openSettings(): Promise<void> {
    return this.s.run("openSettings", "", () => {
      this.openSettingsCount += 1;
    });
  }

  settingsOpenEntry(id: string): Promise<void> {
    return this.s.run("settingsOpenEntry", id, () => {
      const row = this.s.rows.find((r) => r.view.id === id);
      if (!row || row.deletedAt !== null) throw new KinigeApiError("message_not_found");
      this.openedEntries.push(id);
      for (const handler of [...this.openMessageListeners]) handler({ chatId: row.view.chatId, messageId: id });
    });
  }

  onOpenMessage(handler: (target: { chatId: string; messageId: string }) => void): Promise<() => void> {
    this.openMessageListeners.add(handler);
    return Promise.resolve(() => {
      this.openMessageListeners.delete(handler);
    });
  }

  private changeShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings> {
    if (this.failNextShortcut) {
      const code = this.failNextShortcut;
      this.failNextShortcut = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const other = this.s.settings.shortcuts.find((s) => s.kind !== kind);
    if (other?.accelerator === accelerator) return Promise.reject(new KinigeApiError("shortcut_duplicate"));
    this.s.settings = {
      ...this.s.settings,
      shortcuts: this.s.settings.shortcuts.map((s) =>
        s.kind === kind
          ? { kind, accelerator, isDefault: accelerator === DEFAULT_SHORTCUTS[kind], state: "assigned" as const }
          : s,
      ),
    };
    this.s.emitSettingsChanged();
    return Promise.resolve(this.s.settings);
  }
}
