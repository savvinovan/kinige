import { KinigeApiError } from "./KinigeApi";
import { MockApiSlice4a } from "./MockApiSlice4a";
import type { SettingsFileState } from "./generated/SettingsFileState";

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
 * The settings window and the `settings.json` file in `MockApi` (slice 4b, ADR-0010): the file state that a test or a story
 * sets as it likes (`settingsFile`), the "settings changed" event (window edits send it too), opening an entry from the
 * log and the "settings window is open" counter.
 */
export class MockApiSlice4b extends MockApiSlice4a {
  /** What `getSettingsFile` returns: a test replaces it fully or partly. */
  settingsFile: SettingsFileState = MOCK_SETTINGS_FILE;
  /** How many times the settings window was asked to open. */
  openSettingsCount = 0;
  /** Entries that the settings window asked to open in the feed: `settingsOpenEntry` sends them to the main window. */
  readonly openedEntries: string[] = [];
  private readonly settingsListeners = new Set<() => void>();
  private readonly openMessageListeners = new Set<(target: { chatId: string; messageId: string }) => void>();

  getSettingsFile(): Promise<SettingsFileState> {
    return this.run("getSettingsFile", "", () => this.settingsFile);
  }

  openSettings(): Promise<void> {
    return this.run("openSettings", "", () => {
      this.openSettingsCount += 1;
    });
  }

  settingsOpenEntry(id: string): Promise<void> {
    return this.run("settingsOpenEntry", id, () => {
      const row = this.rows.find((r) => r.view.id === id);
      if (!row || row.deletedAt !== null) throw new KinigeApiError("message_not_found");
      this.openedEntries.push(id);
      for (const handler of [...this.openMessageListeners]) handler({ chatId: row.view.chatId, messageId: id });
    });
  }

  onSettingsChanged(handler: () => void): Promise<() => void> {
    this.settingsListeners.add(handler);
    return Promise.resolve(() => {
      this.settingsListeners.delete(handler);
    });
  }

  onOpenMessage(handler: (target: { chatId: string; messageId: string }) => void): Promise<() => void> {
    this.openMessageListeners.add(handler);
    return Promise.resolve(() => {
      this.openMessageListeners.delete(handler);
    });
  }

  /** Like the Rust `settings-changed` event: an edit from the window, another window or the file. */
  emitSettingsChanged(): void {
    for (const handler of [...this.settingsListeners]) handler();
  }
}
