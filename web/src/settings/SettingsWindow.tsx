import type { KinigeApi } from "../api/KinigeApi";
import { SettingsWindowView } from "../ui/prefs/SettingsWindowView";
import { useInputModality } from "../ui/useInputModality";
import { type PreferencesInitial, usePreferences } from "./usePreferences";

interface SettingsWindowProps {
  api: KinigeApi;
  initial: PreferencesInitial;
}

/** Окно «Настройки» на настоящем `KinigeApi` (слайс 4b): `index.html#settings`, отдельное окно ОС. */
export function SettingsWindow({ api, initial }: SettingsWindowProps) {
  useInputModality();
  const view = usePreferences(api, initial);
  return <SettingsWindowView {...view} />;
}
