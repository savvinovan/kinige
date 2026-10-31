import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type KinigeApi, KinigeApiError, type ListMessagesOptions } from "./KinigeApi";
import type { AgentAccess } from "./generated/AgentAccess";
import type { AgentLogEntry } from "./generated/AgentLogEntry";
import type { AgentLogPage } from "./generated/AgentLogPage";
import type { AgentMode } from "./generated/AgentMode";
import type { ApiError } from "./generated/ApiError";
import type { AppInfo } from "./generated/AppInfo";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { ImportOpened } from "./generated/ImportOpened";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportReport } from "./generated/ImportReport";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { SearchFilters } from "./generated/SearchFilters";
import type { SearchPage } from "./generated/SearchPage";
import type { Settings } from "./generated/Settings";
import type { SettingsFileState } from "./generated/SettingsFileState";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";

/** События Rust → главное окно (имена совпадают с `capture::ENTRIES_CHANGED`, `capture::OPEN_CHAT`, `recovery::DATABASE_CORRUPT`). */
const ENTRIES_CHANGED = "entries-changed";
const OPEN_CHAT = "open-chat";
const DATABASE_CORRUPT = "database-corrupt";
/** Настройки изменились и запись просят открыть (`settings_window::SETTINGS_CHANGED`, `OPEN_MESSAGE`). */
const SETTINGS_CHANGED = "settings-changed";
const OPEN_MESSAGE = "open-message";
/** Прогресс импорта (`import::IMPORT_PROGRESS`); приходит только главному окну. */
const IMPORT_PROGRESS = "import-progress";

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
  "message_in_trash",
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
  "mirror_folder_busy",
  "import_not_telegram",
  "import_file_unreadable",
  "import_file_too_large",
  "import_in_progress",
  "import_session_expired",
  "import_backup_failed",
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

  listMessages(chatId: string, options: ListMessagesOptions = {}): Promise<MessagePage> {
    return call("list_messages", {
      chatId,
      before: options.before ?? null,
      after: options.after ?? null,
      around: options.around ?? null,
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

  moveMessage(id: string, chatId: string): Promise<MessageView> {
    return call("move_message", { id, chatId });
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

  importTelegramOpen(): Promise<ImportOpened | null> {
    return call("import_telegram_open");
  }

  importTelegramRun(session: number, chatIds: readonly string[]): Promise<ImportReport> {
    return call("import_telegram_run", { session, chatIds });
  }

  async importTelegramStop(): Promise<void> {
    await call("import_telegram_stop");
  }

  async importTelegramClose(): Promise<void> {
    await call("import_telegram_close");
  }

  onImportProgress(handler: (progress: ImportProgress) => void): Promise<() => void> {
    return listen<ImportProgress>(IMPORT_PROGRESS, (event) => handler(event.payload));
  }

  searchMessages(
    query: string,
    options: { filters?: SearchFilters; before?: FeedCursor; limit?: number } = {},
  ): Promise<SearchPage> {
    return call("search_messages", {
      query,
      filters: options.filters ?? null,
      before: options.before ?? null,
      limit: options.limit ?? null,
    });
  }

  getAgentAccess(): Promise<AgentAccess> {
    return call("get_agent_access");
  }

  async setAgentEnabled(enabled: boolean): Promise<void> {
    await call("set_agent_enabled", { enabled });
  }

  async setAgentMode(mode: AgentMode): Promise<void> {
    await call("set_agent_mode", { mode });
  }

  async setAgentChats(chatIds: readonly string[]): Promise<void> {
    await call("set_agent_chats", { chatIds });
  }

  listAgentLog(before?: number): Promise<AgentLogPage> {
    return call("list_agent_log", { before: before ?? null });
  }

  listAgentLogEntries(logId: number): Promise<AgentLogEntry[]> {
    return call("list_agent_log_entries", { logId });
  }

  async openSettings(): Promise<void> {
    await call("open_settings");
  }

  getSettingsFile(): Promise<SettingsFileState> {
    return call("get_settings_file");
  }

  async settingsOpenEntry(id: string): Promise<void> {
    await call("settings_open_entry", { id });
  }

  onSettingsChanged(handler: () => void): Promise<() => void> {
    return listen(SETTINGS_CHANGED, () => handler());
  }

  onOpenMessage(handler: (target: { chatId: string; messageId: string }) => void): Promise<() => void> {
    return listen<{ chatId: string; messageId: string }>(OPEN_MESSAGE, (event) => handler(event.payload));
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

  captureAddRecentEmoji(emoji: string): Promise<string[]> {
    return call("capture_add_recent_emoji", { emoji });
  }
}
