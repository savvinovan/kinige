import { invoke } from "@tauri-apps/api/core";
import { type KinigeApi, KinigeApiError } from "./KinigeApi";
import type { ApiError } from "./generated/ApiError";
import type { AppInfo } from "./generated/AppInfo";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { TextSize } from "./generated/TextSize";

const CODES: readonly ErrorCode[] = [
  "empty_message",
  "message_too_large",
  "chat_not_found",
  "store_write_failed",
  "store_read_failed",
  "link_not_allowed",
  "folder_pick_failed",
  "internal",
];

/** Отказ команды → `KinigeApiError`. Не наш формат (например, отказ ACL — строка) — `internal`. */
function toApiError(reason: unknown): KinigeApiError {
  const code = (reason as Partial<ApiError> | null)?.code;
  return new KinigeApiError(CODES.find((known) => known === code) ?? "internal");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw toApiError(reason);
  }
}

/** `KinigeApi` поверх команд Tauri. Имена аргументов — camelCase: Tauri сам переводит их в snake_case Rust. */
export class TauriApi implements KinigeApi {
  appInfo(): Promise<AppInfo> {
    return call("app_info");
  }

  getLanguage(): Promise<Language> {
    return call("get_language");
  }

  listChats(): Promise<ChatSummary[]> {
    return call("list_chats");
  }

  listMessages(chatId: string, options: { before?: FeedCursor; limit?: number } = {}): Promise<MessagePage> {
    return call("list_messages", { chatId, before: options.before ?? null, limit: options.limit ?? null });
  }

  sendMessage(chatId: string, body: string): Promise<MessageView> {
    return call("send_message", { chatId, body });
  }

  getSettings(): Promise<Settings> {
    return call("get_settings");
  }

  async setTextSize(size: TextSize): Promise<void> {
    await call("set_text_size", { size });
  }

  chooseMirrorFolder(): Promise<Settings> {
    return call("choose_mirror_folder");
  }

  async openExternalUrl(url: string): Promise<void> {
    await call("open_external_url", { url });
  }
}
