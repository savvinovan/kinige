import type { KinigeApi } from "./KinigeApi";
import type { DemoApi, DemoChat, DemoMessage } from "./demo";
import { DEMO_CHATS, demoMessages } from "./demoData";
import type { AppInfo } from "./generated/AppInfo";

export class MockApi implements KinigeApi, DemoApi {
  /** @param now «сегодня» для демонстрационных записей; в историях Storybook задаётся фиксированно. */
  constructor(private readonly now: Date = new Date()) {}

  appInfo(): Promise<AppInfo> {
    return Promise.resolve({ title: "Kinige mock" });
  }

  listChats(): Promise<DemoChat[]> {
    return Promise.resolve(DEMO_CHATS);
  }

  listMessages(chatId: string): Promise<DemoMessage[]> {
    return Promise.resolve(chatId === "inbox" ? demoMessages(this.now) : []);
  }
}
