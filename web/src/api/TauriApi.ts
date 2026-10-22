import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type KinigeApi, KinigeApiError } from "./KinigeApi";
import type { ApiError } from "./generated/ApiError";
import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { TextSize } from "./generated/TextSize";

/** Событие Rust → главное окно: записи изменились (имя совпадает с `capture::ENTRIES_CHANGED`). */
const ENTRIES_CHANGED = "entries-changed";

const CODES: readonly ErrorCode[] = [
  "empty_message",
  "message_too_large",
  "chat_not_found",
  "store_write_failed",
  "store_read_failed",
  "link_not_allowed",
  "folder_pick_failed",
  "shortcut_invalid",
  "shortcut_taken",
  "shortcut_duplicate",
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

  getDraft(chatId: string): Promise<string> {
    return call("get_draft", { chatId });
  }

  async saveDraft(chatId: string, body: string): Promise<void> {
    await call("save_draft", { chatId, body });
  }

  setShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings> {
    return call("set_shortcut", { kind, accelerator });
  }

  resetShortcut(kind: ShortcutKind): Promise<Settings> {
    return call("reset_shortcut", { kind });
  }

  async shortcutRecording(active: boolean): Promise<void> {
    await call("shortcut_recording", { active });
  }

  onEntriesChanged(handler: () => void): Promise<() => void> {
    return listen(ENTRIES_CHANGED, () => handler());
  }

  captureState(): Promise<CaptureState> {
    return call("capture_state");
  }

  captureSend(body: string): Promise<CaptureOutcome> {
    return call("capture_send", { body });
  }

  captureAppend(body: string): Promise<CaptureOutcome> {
    return call("capture_append", { body });
  }

  async captureSaveDraft(body: string): Promise<void> {
    await call("capture_save_draft", { body });
  }

  async captureHide(): Promise<void> {
    await call("capture_hide");
  }
}
