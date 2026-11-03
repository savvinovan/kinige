import type { AgentAccess } from "./generated/AgentAccess";
import type { AgentLogEntry } from "./generated/AgentLogEntry";
import type { AgentLogPage } from "./generated/AgentLogPage";
import type { AgentMode } from "./generated/AgentMode";
import type { AppInfo } from "./generated/AppInfo";
import type { AttachOutcome } from "./generated/AttachOutcome";
import type { AttachmentView } from "./generated/AttachmentView";
import type { CaptureOutcome } from "./generated/CaptureOutcome";
import type { CaptureState } from "./generated/CaptureState";
import type { ChatDraft } from "./generated/ChatDraft";
import type { ChatList } from "./generated/ChatList";
import type { ChatSummary } from "./generated/ChatSummary";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { FirstWeekday } from "./generated/FirstWeekday";
import type { ForestMode } from "./generated/ForestMode";
import type { ForestTarget } from "./generated/ForestTarget";
import type { ForestView } from "./generated/ForestView";
import type { ForestYear } from "./generated/ForestYear";
import type { ImportOpened } from "./generated/ImportOpened";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportReport } from "./generated/ImportReport";
import type { Language } from "./generated/Language";
import type { MessageFlag } from "./generated/MessageFlag";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { OnThisDay } from "./generated/OnThisDay";
import type { RestoreOutcome } from "./generated/RestoreOutcome";
import type { SearchFilters } from "./generated/SearchFilters";
import type { SearchPage } from "./generated/SearchPage";
import type { Settings } from "./generated/Settings";
import type { SettingsFileState } from "./generated/SettingsFileState";
import type { ShortcutKind } from "./generated/ShortcutKind";
import type { StartupState } from "./generated/StartupState";
import type { TextSize } from "./generated/TextSize";
import type { TrashItem } from "./generated/TrashItem";
import type { WeekdaySource } from "./generated/WeekdaySource";

/** The position and size of a feed page (`KinigeApi.listMessages`). */
export interface ListMessagesOptions {
  before?: FeedCursor;
  after?: FeedCursor;
  /** The id of the entry around which a page is needed. */
  around?: string;
  limit?: number;
  tag?: string;
}

/** The Inbox id: a protocol constant, the same for all accounts (ADR-0001, §3.6). */
export const INBOX_ID = "00000000-0000-4000-8000-000000000001";

/** A command error: Rust returns a code, the text is in the translation files (`error.<code>`). */
export class KinigeApiError extends Error {
  constructor(readonly code: ErrorCode) {
    super(code);
    this.name = "KinigeApiError";
  }
}

/** The error code of any failure; anything that did not come from our commands is `internal`. */
export function errorCode(error: unknown): ErrorCode {
  return error instanceof KinigeApiError ? error.code : "internal";
}

/**
 * The only point through which the frontend talks to the backend (§3.2 of the PRD).
 * Implementations: `TauriApi` (invoke), `MockApi` (tests, Storybook).
 * Errors arrive as `KinigeApiError` with a code.
 */
export interface KinigeApi {
  appInfo(): Promise<AppInfo>;
  /** The interface language from the system's preferred languages (Rust decides). */
  getLanguage(): Promise<Language>;
  /** Chats in display order (pinned, chats, archive) and the trash size. */
  listChats(): Promise<ChatList>;
  /**
   * A feed page: without a position - the newest; entries ascending, the newest last. `before` - an older page after
   * the cursor, `after` - a newer one (slice 4), `around` - a page around an entry (a jump from search); if several
   * positions are given, `around` wins over `after`, and `after` over `before`. `tag` (without `#`, any case) keeps
   * only the entries with this tag.
   */
  listMessages(chatId: string, options?: ListMessagesOptions): Promise<MessagePage>;
  /**
   * Sending: the reply is the saved entry; no optimistic display. With `replyToId` (slice 5) it is a reply to an entry of
   * the same chat; if there is no such entry, it is in the trash or the id is wrong - `reply_target_not_found`, nothing
   * is written.
   */
  sendMessage(chatId: string, body: string, replyToId?: string | null, attachmentIds?: readonly string[]): Promise<MessageView>;
  getSettings(): Promise<Settings>;
  setTextSize(size: TextSize): Promise<void>;
  /** The system dialog for choosing the Markdown copy folder (called from Rust). Cancelling returns the previous settings. */
  chooseMirrorFolder(): Promise<Settings>;
  /** Open a link from an entry in the system browser; Rust lets through only `http`, `https`, `mailto`. */
  openExternalUrl(url: string): Promise<void>;

  // ---- main window (slice 2) ----
  /** The chat input draft (§5): empty text without a reply if there is none. It carries the entry being replied to as well. */
  getDraft(chatId: string): Promise<ChatDraft>;
  /**
   * Saves the draft; it is deleted only when both the text and the reply (`replyToId`) are empty. Sending clears the
   * draft in Rust, in the same transaction.
   */
  saveDraft(chatId: string, body: string, replyToId?: string | null): Promise<void>;
  /** Change your own shortcut: Rust validates the record, registers it in the system and saves it. The reply is the whole settings. */
  setShortcut(kind: ShortcutKind, accelerator: string): Promise<Settings>;
  /** "Reset": the default shortcut, the same checks. */
  resetShortcut(kind: ShortcutKind): Promise<Settings>;
  /** Recording of a new shortcut is going on (`true`) or has ended: during it the hotkeys do not open the capture window. */
  shortcutRecording(active: boolean): Promise<void>;
  /** Subscribe to the "entries changed" event (capture, appending, clipboard). Returns the unsubscribe function. */
  onEntriesChanged(handler: () => void): Promise<() => void>;

  // ---- main window (slice 3): chats, entry actions, trash ----
  /** A new chat at the end of the "Чаты" section. The name is 1-128 characters; without an emoji - 💬. */
  createChat(title: string, icon: string | null): Promise<ChatSummary>;
  renameChat(id: string, title: string): Promise<ChatSummary>;
  setChatIcon(id: string, icon: string): Promise<ChatSummary>;
  setChatPinned(id: string, pinned: boolean): Promise<ChatSummary>;
  /** The Inbox cannot be archived (`inbox_protected`). */
  setChatArchived(id: string, archived: boolean): Promise<ChatSummary>;
  /** "Delete": the entry goes to the trash. No optimistic display: the reply comes after the commit. */
  deleteMessage(id: string): Promise<void>;
  /** "Restore" from the trash: the entry returns to its place. */
  restoreMessage(id: string): Promise<void>;
  /** Text edit; the old version is kept as a revision. The reply is the entry as it looks now. */
  editMessage(id: string, body: string): Promise<MessageView>;
  /** A click on `- [ ]` in the text: the item `index` (zero-based, in text order) gets the `checked` state. */
  toggleTaskItem(id: string, index: number, checked: boolean): Promise<MessageView>;
  /** Task, done (tasks only), favorite. */
  setMessageFlag(id: string, flag: MessageFlag, on: boolean): Promise<MessageView>;
  /** "Change date": `YYYY-MM-DD`, future dates are allowed. */
  setMessageDate(id: string, journalDate: string): Promise<MessageView>;
  /** "Move to chat" (the entry menu, dragging onto a chat): the day and the text do not change, not into the trash. */
  moveMessage(id: string, chatId: string): Promise<MessageView>;
  /** The trash: recently deleted first. */
  listTrash(): Promise<TrashItem[]>;
  /** "Delete forever" from the trash (the interface asks for confirmation). */
  purgeMessage(id: string): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
  /** The emoji chosen in the picker goes first in "recent"; the reply is the whole list. */
  addRecentEmoji(emoji: string): Promise<string[]>;
  /** The tray menu: "open this chat". Returns the unsubscribe function. */
  onOpenChat(handler: (chatId: string) => void): Promise<() => void>;

  // ---- startup and database backups (slice 3) ----
  /** The journal or the "Database is corrupted" window with a list of backups. */
  startupState(): Promise<StartupState>;
  /**
   * Restore from the backup `name` (only when the journal is not open; a repeated call during restoring is an error).
   * The reply is where the previous Markdown copy folder was moved.
   */
  restoreBackup(name: string): Promise<RestoreOutcome>;
  /** The integrity check found corruption in the running database. Returns the unsubscribe function. */
  onDatabaseCorrupt(handler: () => void): Promise<() => void>;
  /** Restart the application for the restore (after the `onDatabaseCorrupt` event). */
  restartForRecovery(): Promise<void>;

  // ---- Telegram import and search (slice 4) ----
  /**
   * "Choose result.json...": the system dialog (called from Rust, the path does not reach the frontend), reading and
   * parsing the file. Cancelling the dialog - `null`. The wrong file - `import_not_telegram`, unreadable -
   * `import_file_unreadable`, over 1 GB - `import_file_too_large`; nothing is written.
   */
  importTelegramOpen(): Promise<ImportOpened | null>;
  /**
   * "Import": the chosen chats (Telegram ids) of the parsed file. The reply comes at the end; progress is the
   * `onImportProgress` event. Someone else's or a forgotten session - `import_session_expired`, a second import -
   * `import_in_progress`.
   */
  importTelegramRun(session: number, chatIds: readonly string[]): Promise<ImportReport>;
  /** "Stop": the running import finishes the current batch and stops. */
  importTelegramStop(): Promise<void>;
  /** The import window is closed: the parsed file is forgotten. */
  importTelegramClose(): Promise<void>;
  /** The progress of the running import after each batch. Returns the unsubscribe function. */
  onImportProgress(handler: (progress: ImportProgress) => void): Promise<() => void>;
  /**
   * Entry search: the input as is (Rust decides word forms and prefix). Newer entries on top; `before` - the next page;
   * chats with a matching name come on the first one. An empty query - an empty page.
   */
  searchMessages(query: string, options?: { filters?: SearchFilters; before?: FeedCursor; limit?: number }): Promise<SearchPage>;

  // ---- AI agent access (slice 4a, ADR-0009): main window only ----
  /** The flag, the mode, the chats marked "visible to the agent", the socket state and the connection snippets. */
  getAgentAccess(): Promise<AgentAccess>;
  /** Turn the access on or off. The window shows the warning on enabling before the call. */
  setAgentEnabled(enabled: boolean): Promise<void>;
  setAgentMode(mode: AgentMode): Promise<void>;
  /** The chats visible to the agent (the whole list). A chat with `aiExcluded` and a nonexistent one are not accepted (`chat_not_found`). */
  setAgentChats(chatIds: readonly string[]): Promise<void>;
  /** The call log, newest on top; `before` - the `id` of the last row of the previous page. */
  listAgentLog(before?: number): Promise<AgentLogPage>;
  /** The entries a call touched; one deleted forever comes with `message: null`. */
  listAgentLogEntries(logId: number): Promise<AgentLogEntry[]>;

  // ---- settings window and the settings.json file (slice 4b, ADR-0010) ----
  /** Open the settings window from the main window (opening again raises the same window). */
  openSettings(): Promise<void>;
  /** The `settings.json` path, what is wrong with it and which keys have errors; for the settings window. */
  getSettingsFile(): Promise<SettingsFileState>;
  /**
   * Open an entry from the call log in the main window feed. Rust checks the entry: deleted forever or in the trash -
   * `message_not_found`; the content is not given to the settings window.
   */
  settingsOpenEntry(id: string): Promise<void>;
  /** Settings changed: by the window, another window or the file (other people's edits are seen within seconds). Returns the unsubscribe function. */
  onSettingsChanged(handler: () => void): Promise<() => void>;
  /** The settings window asks to open an entry: the main window opens the chat at that entry. Returns the unsubscribe function. */
  onOpenMessage(handler: (target: { chatId: string; messageId: string }) => void): Promise<() => void>;

  // ---- "On this day" (slice 5) ----
  /** The Inbox "On this day" card; `null` - there are no entries of this date in past years or the card is hidden for today. */
  getOnThisDay(): Promise<OnThisDay | null>;
  /** "Hide until tomorrow": `date` is the card's `today`; until the end of this journal day the card is not shown (locally). */
  dismissOnThisDay(date: string): Promise<void>;

  // ---- the year's forest (slice 6): main window only ----
  /**
   * The table of one year: every day with its cell, season, notes, stage and the start of its first entry, plus the
   * current journal day and the year of the oldest entry. `weekdaySource` is sent once per session, only for the debug
   * log. A year outside 1000..9999 is `invalid_date`.
   */
  getForestYear(year: number, firstWeekday: FirstWeekday, weekdaySource?: WeekdaySource): Promise<ForestYear>;
  /**
   * The entry a click on a day opens: the open chat if it has entries that day, otherwise the chat with the most entries,
   * the Inbox on a tie; `null` for an empty day. `currentChatId` is `null` when no chat is open.
   */
  getForestDayTarget(date: string, currentChatId: string | null): Promise<ForestTarget | null>;
  /** Remembers how much of the forest is shown and whether it is the board or the calendar (local, not synced). */
  setForestView(view: ForestView, mode: ForestMode): Promise<void>;

  // ---- attachments (slice 7): main window only ----
  /**
   * The paperclip: the system file dialog (called from Rust, paths do not reach the frontend); every chosen file becomes a tile
   * of the chat's draft. Cancelling attaches nothing. The first error stops the rest and comes back in `error` together with the
   * tiles that made it.
   */
  attachPick(chatId: string): Promise<AttachOutcome>;
  /**
   * A paste: the bytes of an image or a file from the clipboard. `nameHint` is only a hint and is cleaned in Rust (a nameless
   * paste is named `pasted-HHMMSS.<ext>`). More than 64 MiB - `attachment_too_large`.
   */
  attachPaste(chatId: string, bytes: Uint8Array, nameHint?: string): Promise<AttachmentView>;
  /** Claims a drop: `token` comes from `onAttachmentsDropped`, is single-use and expires. Folders are skipped and counted. */
  attachDropped(token: string, chatId: string): Promise<AttachOutcome>;
  /** The x on a tile: the draft attachment and its file go. */
  removeDraftAttachment(chatId: string, attachmentId: string): Promise<void>;
  /** Open an attachment in its system app (a clone; documents are opened, everything else is shown in the file manager). */
  openAttachment(attachmentId: string): Promise<void>;
  /** Files are dragged over the window (`true`) or left it / were dropped (`false`): the "Drop to attach" overlay. */
  onAttachmentsDrag(handler: (over: boolean) => void): Promise<() => void>;
  /** Files were dropped on the window: the one-time token for `attachDropped`, never a path. */
  onAttachmentsDropped(handler: (token: string) => void): Promise<() => void>;

  // ---- capture window (slice 2): a separate set of commands, Inbox only ----
  /** Everything the window needs on showing: language, text size, today's context, the append target, the draft, an error. */
  captureState(): Promise<CaptureState>;
  /** Enter: a new entry in the Inbox. */
  captureSend(body: string): Promise<CaptureOutcome>;
  /** Cmd+Enter: append to the last entry (if Rust decided it is allowed) or a new entry; the reply says what was done. */
  captureAppend(body: string): Promise<CaptureOutcome>;
  captureSaveDraft(body: string): Promise<void>;
  /** Hide the window (after sending, Esc, loss of focus). */
  captureHide(): Promise<void>;
  /**
   * Fit the window height to the content (logical px, together with the transparent margin for the shadow). Rust clamps
   * the value to the range `layout.capture.window-min-height … window-max-height`; the width and the top of the window do not change.
   */
  captureResize(height: number): Promise<void>;
  /** The emoji chosen in the capture window picker goes first in the shared "recent" list; the reply is the whole list. */
  captureAddRecentEmoji(emoji: string): Promise<string[]>;
}
