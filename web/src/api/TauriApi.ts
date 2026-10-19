import { invoke } from "@tauri-apps/api/core";
import type { KinigeApi } from "./KinigeApi";
import type { AppInfo } from "./generated/AppInfo";

export class TauriApi implements KinigeApi {
  appInfo(): Promise<AppInfo> {
    return invoke<AppInfo>("app_info");
  }
}
