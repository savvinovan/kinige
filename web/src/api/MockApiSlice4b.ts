import { KinigeApiError } from "./KinigeApi";
import { MockApiSlice4a } from "./MockApiSlice4a";
import type { SettingsFileState } from "./generated/SettingsFileState";

/** Состояние файла настроек без проблем: путь и пустые списки. */
export const MOCK_SETTINGS_FILE: SettingsFileState = {
  path: "/Users/me/Library/Application Support/me.savvinov.kinige/settings.json",
  problem: null,
  unsafeReason: null,
  issues: [],
  unknownKeys: [],
  changed: { appearance: null, shortcuts: null, mirror: null, access: null },
};

/**
 * Окно настроек и файл `settings.json` в `MockApi` (слайс 4b, ADR-0010): состояние файла, которое тест или история задают
 * как хотят (`settingsFile`), событие «настройки изменились» (его шлют и правки окна), открытие записи из журнала и счётчик
 * «окно настроек открыто».
 */
export class MockApiSlice4b extends MockApiSlice4a {
  /** Что вернёт `getSettingsFile`: тест подменяет целиком или частично. */
  settingsFile: SettingsFileState = MOCK_SETTINGS_FILE;
  /** Сколько раз просили открыть окно настроек. */
  openSettingsCount = 0;
  /** Записи, которые окно настроек просило открыть в ленте: `settingsOpenEntry` отправляет их главному окну. */
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

  /** Как событие Rust `settings-changed`: правка окна, другого окна или файла. */
  emitSettingsChanged(): void {
    for (const handler of [...this.settingsListeners]) handler();
  }
}
