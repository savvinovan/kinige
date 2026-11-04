import { invoke } from "@tauri-apps/api/core";
import { KinigeApiError } from "./KinigeApi";
import type { ApiError } from "./generated/ApiError";
import type { ErrorCode } from "./generated/ErrorCode";

/** Rust -> main window events (the names match `capture::ENTRIES_CHANGED`, `capture::OPEN_CHAT`, `recovery::DATABASE_CORRUPT`). */
export const ENTRIES_CHANGED = "entries-changed";
export const OPEN_CHAT = "open-chat";
export const DATABASE_CORRUPT = "database-corrupt";
/** Settings changed and an entry is asked to be opened (`settings_window::SETTINGS_CHANGED`, `OPEN_MESSAGE`). */
export const SETTINGS_CHANGED = "settings-changed";
export const OPEN_MESSAGE = "open-message";
/** Import progress (`import::IMPORT_PROGRESS`); it reaches the main window only. */
export const IMPORT_PROGRESS = "import-progress";
/** Files dragged over the window and dropped on it (`attachments::ATTACH_DRAG`, `ATTACH_DROPPED`); the main window only. */
export const ATTACHMENTS_DRAG = "attachments-drag";
export const ATTACHMENTS_DROPPED = "attachments-dropped";

const CODES = [
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
  "reply_target_not_found",
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
  "unsupported",
  "attachment_too_large",
  "attachment_limit",
  "attachment_unreadable",
  "disk_full",
  "attachment_not_found",
  "folders_not_attached",
  "internal",
] as const satisfies readonly ErrorCode[];

// Every `ErrorCode` of the generated type must be listed above: a missing one makes this line fail to compile.
const allCodesListed: [Exclude<ErrorCode, (typeof CODES)[number]>] extends [never] ? true : never = true;
void allCodesListed;

/** A command failure -> `KinigeApiError`. Not our format (for example, an ACL denial is a string) -> `internal`. */
export function toApiError(reason: unknown): KinigeApiError {
  const code = (reason as Partial<ApiError> | null)?.code;
  return new KinigeApiError(CODES.find((known) => known === code) ?? "internal");
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw toApiError(reason);
  }
}
