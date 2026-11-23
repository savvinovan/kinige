import { invoke } from "@tauri-apps/api/core";
import { KinigeApiError } from "./KinigeApi";
import type { ApiError } from "./generated/ApiError";
import type { ErrorCode } from "./generated/ErrorCode";

/** Rust -> main window events (the names match `capture::ENTRIES_CHANGED`, `capture::OPEN_CHAT`, `recovery::DATABASE_CORRUPT`). */
export const ENTRIES_CHANGED = "entries-changed";
export const OPEN_CHAT = "open-chat";
export const DATABASE_CORRUPT = "database-corrupt";
/** The main window is being hidden by a close (`main_window::MAIN_WINDOW_HIDING`): a recording is stopped and kept first (KINIG-156). */
export const MAIN_WINDOW_HIDING = "main-window-hiding";
/** Settings changed and an entry is asked to be opened (`settings_window::SETTINGS_CHANGED`, `OPEN_MESSAGE`). */
export const SETTINGS_CHANGED = "settings-changed";
export const OPEN_MESSAGE = "open-message";
/** Import progress (`import::IMPORT_PROGRESS`); it reaches the main window only. */
export const IMPORT_PROGRESS = "import-progress";
/** Sticker pack download progress (`stickers::STICKER_PACK_PROGRESS`, slice 5c); it reaches the main window only. */
export const STICKER_PACK_PROGRESS = "sticker-pack-progress";
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
  "voice_too_short",
  "voice_too_long",
  "folders_not_attached",
  "drop_not_supported",
  "attachment_details_invalid",
  "reaction_invalid",
  "reaction_limit",
  "sticker_not_found",
  "sticker_unreadable",
  "sticker_entry_read_only",
  "sticker_pack_link_invalid",
  "sticker_pack_no_token",
  "sticker_pack_not_found",
  "sticker_pack_token_refused",
  "sticker_pack_unreachable",
  "sticker_pack_wait",
  "sticker_pack_only_animated",
  "sticker_pack_too_large",
  "sticker_pack_busy",
  "sticker_pack_bad_response",
  "telegram_token_invalid",
  "secret_store_unavailable",
  "secret_store_unsupported",
  "secret_store_denied",
  "sticker_pack_cancelled",
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
