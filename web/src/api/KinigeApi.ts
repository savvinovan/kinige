import type { AppInfo } from "./generated/AppInfo";

/**
 * Единственная точка, через которую фронтенд говорит с бэкендом (§3.2 ТЗ).
 * Реализации: `TauriApi` (invoke), `MockApi` (тесты, Storybook).
 */
export interface KinigeApi {
  appInfo(): Promise<AppInfo>;
}
