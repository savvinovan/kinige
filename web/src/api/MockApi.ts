import type { KinigeApi } from "./KinigeApi";
import type { AppInfo } from "./generated/AppInfo";

export class MockApi implements KinigeApi {
  appInfo(): Promise<AppInfo> {
    return Promise.resolve({ title: "Kinige mock" });
  }
}
