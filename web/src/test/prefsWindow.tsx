import { MockApi, type MockApiOptions } from "../api/MockApi";
import type { Settings } from "../api/generated/Settings";
import { DEFAULT_SETTINGS } from "../api/mockData";
import { render, screen } from "./render";
import { DEFAULT_SHORTCUTS } from "../ui/settings/demoShortcuts";
import { SettingsWindow } from "../settings/SettingsWindow";
import { loadPreferences } from "../settings/usePreferences";

/** Настройки для тестов окна: путь копии, сочетания по умолчанию. */
export const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
};

export const NOW = () => new Date(Date.UTC(2026, 9, 28, 12, 0));

/** Окно «Настройки» на `MockApi` (как в приложении, но в памяти). Чаты: «Работа», «Дневник» (`ai_excluded`). */
export async function openWindow(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({
    now: NOW,
    settings: SETTINGS,
    chats: [
      { id: "work", title: "Работа", icon: "💼" },
      { id: "diary", title: "Дневник", icon: "📓" },
    ],
    ...options,
  });
  api.agentExcluded.add("diary");
  const initial = await loadPreferences(api);
  render(<SettingsWindow api={api} initial={initial} />, { language });
  await screen.findByRole("navigation", { name: /Разделы настроек|Settings sections/ });
  return api;
}
