import type { KinigeApi } from "../api/KinigeApi";
import { SettingsWindowView } from "../ui/prefs/SettingsWindowView";
import { useInputModality } from "../ui/useInputModality";
import { type PreferencesInitial, usePreferences } from "./usePreferences";

interface SettingsWindowProps {
  api: KinigeApi;
  initial: PreferencesInitial;
}

/** The "Настройки" (Settings) window on the real `KinigeApi` (slice 4b): `index.html#settings`, a separate OS window. */
export function SettingsWindow({ api, initial }: SettingsWindowProps) {
  useInputModality();
  const view = usePreferences(api, initial);
  return <SettingsWindowView {...view} />;
}
