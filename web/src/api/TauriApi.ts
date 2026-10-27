import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type KinigeApi, KinigeApiError } from "./KinigeApi";
import type { ApiError } from "./generated/ApiError";
import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { Settings } from "./generated/Settings";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";

/** События Rust → главное окно (имена совпадают с `capture::ENTRIES_CHANGED`, `capture::OPEN_CHAT`, `recovery::DATABASE_CORRUPT`). */
const ENTRIES_CHANGED = "entries-changed";
const OPEN_CHAT = "open-chat";
const DATABASE_CORRUPT = "database-corrupt";

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
  "message_not_found",
  "read_only",
  "inbox_protected",
  "not_in_trash",
  "invalid_date",
  "chat_title_empty",
  "chat_title_too_long",
  "chat_icon_invalid",
  "task_item_stale",
  "database_corrupt",
  "database_too_new",
  "backup_not_found",
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

  listChats(): Promise<ChatList> {
    return call("list_chats");
  }

  listMessages(chatId: string, options: { before?: FeedCursor; limit?: number; tag?: string } = {}): Promise<MessagePage> {
    return call("list_messages", {
      chatId,
      before: options.before ?? null,
      limit: options.limit ?? null,
      tag: options.tag ?? null,
    });
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

  createChat(title: string, icon: string | null): Promise<ChatSummary> {
    return call("create_chat", { title, icon });
  }

  renameChat(id: string, title: string): Promise<ChatSummary> {
    return call("rename_chat", { id, title });
  }

  setChatIcon(id: string, icon: string): Promise<ChatSummary> {
    return call("set_chat_icon", { id, icon });
  }

  setChatPinned(id: string, pinned: boolean): Promise<ChatSummary> {
    return call("set_chat_pinned", { id, pinned });
  }

  setChatArchived(id: string, archived: boolean): Promise<ChatSummary> {
    return call("set_chat_archived", { id, archived });
  }

  async deleteMessage(id: string): Promise<void> {
    await call("delete_message", { id });
  }

  async restoreMessage(id: string): Promise<void> {
    await call("restore_message", { id });
  }

  editMessage(id: string, body: string): Promise<MessageView> {
    return call("edit_message", { id, body });
  }

  toggleTaskItem(id: string, index: number, checked: boolean): Promise<MessageView> {
    return call("toggle_task_item", { id, index, checked });
  }

  setMessageFlag(id: string, flag: MessageFlag, on: boolean): Promise<MessageView> {
    return call("set_message_flag", { id, flag, on });
  }

  setMessageDate(id: string, journalDate: string): Promise<MessageView> {
    return call("set_message_date", { id, journalDate });
  }

  listTrash(): Promise<TrashItem[]> {
    return call("list_trash");
  }

  async purgeMessage(id: string): Promise<void> {
    await call("purge_message", { id });
  }

  async setSidebarCollapsed(collapsed: boolean): Promise<void> {
    await call("set_sidebar_collapsed", { collapsed });
  }

  addRecentEmoji(emoji: string): Promise<string[]> {
    return call("add_recent_emoji", { emoji });
  }

  onOpenChat(handler: (chatId: string) => void): Promise<() => void> {
    return listen<string>(OPEN_CHAT, (event) => handler(event.payload));
  }

  startupState(): Promise<StartupState> {
    return call("startup_state");
  }

  restoreBackup(name: string): Promise<RestoreOutcome> {
    return call("restore_backup", { name });
  }

  onDatabaseCorrupt(handler: () => void): Promise<() => void> {
    return listen(DATABASE_CORRUPT, () => handler());
  }

  async restartForRecovery(): Promise<void> {
    await call("restart_for_recovery");
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

  async captureResize(height: number): Promise<void> {
    await call("capture_resize", { height });
  }
}
