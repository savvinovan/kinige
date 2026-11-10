import type { KinigeApi } from "./KinigeApi";
import type { MockApiOptions } from "./mockData";
import type { AgentLogEntry } from "./generated/AgentLogEntry";
import type { AgentLogRow } from "./generated/AgentLogRow";
import type { AgentMode } from "./generated/AgentMode";
import type { AgentSocket } from "./generated/AgentSocket";
import type { AttachmentView } from "./generated/AttachmentView";
import type { ErrorCode } from "./generated/ErrorCode";
import type { ImportReport } from "./generated/ImportReport";
import type { MessageView } from "./generated/MessageView";
import type { SettingsFileState } from "./generated/SettingsFileState";
import { MockAgents } from "./mock/MockAgents";
import { MockApp } from "./mock/MockApp";
import { MockAttachments } from "./mock/MockAttachments";
import { MockCapture } from "./mock/MockCapture";
import { MockChats } from "./mock/MockChats";
import { MockCompose } from "./mock/MockCompose";
import { MockEntries } from "./mock/MockEntries";
import { MockForest } from "./mock/MockForest";
import { MockImport, type MockImportFile } from "./mock/MockImport";
import { MockOnThisDay } from "./mock/MockOnThisDay";
import { MockSearch } from "./mock/MockSearch";
import { MockSettings } from "./mock/MockSettings";
import { type MockMethod, MockState } from "./mock/MockState";
import { MockTrash } from "./mock/MockTrash";
import { MockVoice } from "./mock/MockVoice";

export { DEFAULT_SETTINGS, type MockApiOptions, mockMessage } from "./mockData";
export { MOCK_AGENT_SOCKET, MOCK_LOG_PAGE } from "./mock/MockAgents";
export { mockAttachment } from "./mock/MockAttachments";
export { MOCK_BACKUP_NAME, type MockImportFile } from "./mock/MockImport";
export { MOCK_SETTINGS_FILE } from "./mock/MockSettings";
export type { MockMethod } from "./mock/MockState";

/**
 * `KinigeApi` in memory: the same interface as `TauriApi`, for tests and Storybook. Stores chats and entries, counts counters, keeps the
 * trash, edits and revisions, can "fail" any command (`failNext`) and remembers opened links (`openedUrls`). The work is done by one
 * class per area in `mock/`, over a shared `MockState`; this class hands each command to its area and exposes the knobs and logs that
 * tests and stories read and set (each is documented in the area that owns it). The entry day is computed in a simplified way (UTC,
 * 04:00 boundary): the real rules are in Rust, and the ones repeated here are pinned by `fixtures/api-contract/`.
 */
export class MockApi implements KinigeApi {
  private readonly state: MockState;
  private readonly app: MockApp;
  private readonly chats: MockChats;
  private readonly entries: MockEntries;
  private readonly compose: MockCompose;
  private readonly trash: MockTrash;
  private readonly search: MockSearch;
  private readonly importer: MockImport;
  private readonly settings: MockSettings;
  private readonly agents: MockAgents;
  private readonly capture: MockCapture;
  private readonly onThisDay: MockOnThisDay;
  private readonly forest: MockForest;
  private readonly attachments: MockAttachments;
  private readonly voice: MockVoice;

  constructor(options: MockApiOptions = {}) {
    const state = new MockState(options);
    this.state = state;
    this.app = new MockApp(state);
    this.chats = new MockChats(state);
    this.entries = new MockEntries(state);
    this.compose = new MockCompose(state);
    this.trash = new MockTrash(state);
    this.search = new MockSearch(state);
    this.importer = new MockImport(state);
    this.settings = new MockSettings(state, options.pickedFolder);
    this.agents = new MockAgents(state);
    this.capture = new MockCapture(state);
    this.onThisDay = new MockOnThisDay(state);
    this.forest = new MockForest(state);
    this.attachments = new MockAttachments(state);
    this.voice = new MockVoice(state, this.compose);
  }

  // ---- the app ----
  appInfo: KinigeApi["appInfo"] = () => this.app.appInfo();
  getLanguage: KinigeApi["getLanguage"] = () => this.app.getLanguage();
  openExternalUrl: KinigeApi["openExternalUrl"] = (url) => this.app.openExternalUrl(url);
  startupState: KinigeApi["startupState"] = () => this.app.startupState();
  restoreBackup: KinigeApi["restoreBackup"] = (name) => this.app.restoreBackup(name);
  onDatabaseCorrupt: KinigeApi["onDatabaseCorrupt"] = (handler) => this.app.onDatabaseCorrupt(handler);
  restartForRecovery: KinigeApi["restartForRecovery"] = () => this.app.restartForRecovery();

  // ---- chats ----
  listChats: KinigeApi["listChats"] = () => this.chats.listChats();
  createChat: KinigeApi["createChat"] = (title, icon) => this.chats.createChat(title, icon);
  renameChat: KinigeApi["renameChat"] = (id, title) => this.chats.renameChat(id, title);
  setChatIcon: KinigeApi["setChatIcon"] = (id, icon) => this.chats.setChatIcon(id, icon);
  setChatColor: KinigeApi["setChatColor"] = (id, color) => this.chats.setChatColor(id, color);
  setChatPinned: KinigeApi["setChatPinned"] = (id, pinned) => this.chats.setChatPinned(id, pinned);
  setChatArchived: KinigeApi["setChatArchived"] = (id, archived) => this.chats.setChatArchived(id, archived);
  setSidebarCollapsed: KinigeApi["setSidebarCollapsed"] = (collapsed) => this.chats.setSidebarCollapsed(collapsed);
  addRecentEmoji: KinigeApi["addRecentEmoji"] = (emoji) => this.chats.addRecentEmoji(emoji);
  onOpenChat: KinigeApi["onOpenChat"] = (handler) => this.chats.onOpenChat(handler);

  // ---- entries, drafts, trash, search ----
  listMessages: KinigeApi["listMessages"] = (chatId, options) => this.entries.listMessages(chatId, options);
  editMessage: KinigeApi["editMessage"] = (id, body) => this.entries.editMessage(id, body);
  toggleTaskItem: KinigeApi["toggleTaskItem"] = (id, index, checked) => this.entries.toggleTaskItem(id, index, checked);
  setMessageFlag: KinigeApi["setMessageFlag"] = (id, flag, on) => this.entries.setMessageFlag(id, flag, on);
  setMessageDate: KinigeApi["setMessageDate"] = (id, journalDate) => this.entries.setMessageDate(id, journalDate);
  moveMessage: KinigeApi["moveMessage"] = (id, chatId) => this.entries.moveMessage(id, chatId);
  sendMessage: KinigeApi["sendMessage"] = (chatId, body, replyToId, attachmentIds) =>
    this.compose.sendMessage(chatId, body, replyToId, attachmentIds);
  getDraft: KinigeApi["getDraft"] = (chatId) => this.compose.getDraft(chatId);
  saveDraft: KinigeApi["saveDraft"] = (chatId, body, replyToId) => this.compose.saveDraft(chatId, body, replyToId);
  deleteMessage: KinigeApi["deleteMessage"] = (id) => this.trash.deleteMessage(id);
  restoreMessage: KinigeApi["restoreMessage"] = (id) => this.trash.restoreMessage(id);
  listTrash: KinigeApi["listTrash"] = () => this.trash.listTrash();
  purgeMessage: KinigeApi["purgeMessage"] = (id) => this.trash.purgeMessage(id);
  searchMessages: KinigeApi["searchMessages"] = (query, options) => this.search.searchMessages(query, options);
  onEntriesChanged: KinigeApi["onEntriesChanged"] = (handler) => this.state.onEntriesChanged(handler);

  // ---- import ----
  importTelegramOpen: KinigeApi["importTelegramOpen"] = () => this.importer.importTelegramOpen();
  importTelegramRun: KinigeApi["importTelegramRun"] = (session, chatIds) => this.importer.importTelegramRun(session, chatIds);
  importTelegramStop: KinigeApi["importTelegramStop"] = () => this.importer.importTelegramStop();
  importTelegramClose: KinigeApi["importTelegramClose"] = () => this.importer.importTelegramClose();
  onImportProgress: KinigeApi["onImportProgress"] = (handler) => this.importer.onImportProgress(handler);

  // ---- settings and the settings window ----
  getSettings: KinigeApi["getSettings"] = () => this.settings.getSettings();
  setTextSize: KinigeApi["setTextSize"] = (size) => this.settings.setTextSize(size);
  chooseMirrorFolder: KinigeApi["chooseMirrorFolder"] = () => this.settings.chooseMirrorFolder();
  setShortcut: KinigeApi["setShortcut"] = (kind, accelerator) => this.settings.setShortcut(kind, accelerator);
  resetShortcut: KinigeApi["resetShortcut"] = (kind) => this.settings.resetShortcut(kind);
  shortcutRecording: KinigeApi["shortcutRecording"] = (active) => this.settings.shortcutRecording(active);
  getSettingsFile: KinigeApi["getSettingsFile"] = () => this.settings.getSettingsFile();
  openSettings: KinigeApi["openSettings"] = () => this.settings.openSettings();
  settingsOpenEntry: KinigeApi["settingsOpenEntry"] = (id) => this.settings.settingsOpenEntry(id);
  onOpenMessage: KinigeApi["onOpenMessage"] = (handler) => this.settings.onOpenMessage(handler);
  onSettingsChanged: KinigeApi["onSettingsChanged"] = (handler) => this.state.onSettingsChanged(handler);

  // ---- AI agents ----
  getAgentAccess: KinigeApi["getAgentAccess"] = () => this.agents.getAgentAccess();
  setAgentEnabled: KinigeApi["setAgentEnabled"] = (enabled) => this.agents.setAgentEnabled(enabled);
  setAgentMode: KinigeApi["setAgentMode"] = (mode) => this.agents.setAgentMode(mode);
  setAgentChats: KinigeApi["setAgentChats"] = (chatIds) => this.agents.setAgentChats(chatIds);
  listAgentLog: KinigeApi["listAgentLog"] = (before) => this.agents.listAgentLog(before);
  listAgentLogEntries: KinigeApi["listAgentLogEntries"] = (logId) => this.agents.listAgentLogEntries(logId);

  // ---- the capture window ----
  captureState: KinigeApi["captureState"] = () => this.capture.captureState();
  captureSend: KinigeApi["captureSend"] = (body) => this.capture.captureSend(body);
  captureAppend: KinigeApi["captureAppend"] = (body) => this.capture.captureAppend(body);
  captureSaveDraft: KinigeApi["captureSaveDraft"] = (body) => this.capture.captureSaveDraft(body);
  captureHide: KinigeApi["captureHide"] = () => this.capture.captureHide();
  captureResize: KinigeApi["captureResize"] = (height) => this.capture.captureResize(height);
  captureAddRecentEmoji: KinigeApi["captureAddRecentEmoji"] = (emoji) => this.capture.captureAddRecentEmoji(emoji);

  // ---- on this day, the forest ----
  getOnThisDay: KinigeApi["getOnThisDay"] = () => this.onThisDay.getOnThisDay();
  dismissOnThisDay: KinigeApi["dismissOnThisDay"] = (date) => this.onThisDay.dismissOnThisDay(date);
  getForestYear: KinigeApi["getForestYear"] = (year, firstWeekday, source) => this.forest.getForestYear(year, firstWeekday, source);
  getForestDayTarget: KinigeApi["getForestDayTarget"] = (date, currentChatId) => this.forest.getForestDayTarget(date, currentChatId);
  setForestView: KinigeApi["setForestView"] = (view, mode) => this.forest.setForestView(view, mode);

  // ---- attachments and voice notes ----
  attachPick: KinigeApi["attachPick"] = (chatId) => this.attachments.attachPick(chatId);
  attachPaste: KinigeApi["attachPaste"] = (chatId, bytes, nameHint) => this.attachments.attachPaste(chatId, bytes, nameHint);
  attachDropped: KinigeApi["attachDropped"] = (token, chatId) => this.attachments.attachDropped(token, chatId);
  removeDraftAttachment: KinigeApi["removeDraftAttachment"] = (chatId, id) => this.attachments.removeDraftAttachment(chatId, id);
  openAttachment: KinigeApi["openAttachment"] = (id) => this.attachments.openAttachment(id);
  onAttachmentsDrag: KinigeApi["onAttachmentsDrag"] = (handler) => this.attachments.onAttachmentsDrag(handler);
  onAttachmentsDropped: KinigeApi["onAttachmentsDropped"] = (handler) => this.attachments.onAttachmentsDropped(handler);
  sendVoice: KinigeApi["sendVoice"] = (chatId, bytes, ms, replyToId, waveform) => this.voice.sendVoice(chatId, bytes, ms, replyToId, waveform);
  readVoiceForWaveform: KinigeApi["readVoiceForWaveform"] = (id) => this.voice.readVoiceForWaveform(id);
  setAttachmentWaveform: KinigeApi["setAttachmentWaveform"] = (id, peaks) => this.voice.setAttachmentWaveform(id, peaks);
  openMicrophoneSettings: KinigeApi["openMicrophoneSettings"] = () => this.voice.openMicrophoneSettings();

  // ---- for tests and stories: failures, events, entries ----

  /** The next call of this command fails with an error (once). */
  failNext(method: MockMethod, code: ErrorCode): void {
    this.state.failNext(method, code);
  }

  /** Like the Rust "entries changed" event (in tests, an entry made by the capture window). */
  emitEntriesChanged(): void {
    this.state.emitEntriesChanged();
  }

  /** Like the Rust `settings-changed` event: an edit from the window, another window or the file. */
  emitSettingsChanged(): void {
    this.state.emitSettingsChanged();
  }

  /** Like the tray menu "open this chat" event. */
  emitOpenChat(chatId: string): void {
    this.chats.emitOpenChat(chatId);
  }

  /** Like the "integrity check found corruption" event. */
  emitDatabaseCorrupt(): void {
    this.app.emitDatabaseCorrupt();
  }

  /** Like the Rust events while files are dragged over the window. */
  emitAttachmentsDrag(over: boolean): void {
    this.attachments.emitAttachmentsDrag(over);
  }

  /** Like a drop of the named files on the window: the token the window claims with `attachDropped`. */
  emitAttachmentsDropped(names: string[], token?: string): string {
    return this.attachments.emitAttachmentsDropped(names, token);
  }

  /** An entry that appeared "from outside" (the capture window, the clipboard): reaches the feed after the event. */
  addExternalMessage(message: MessageView): void {
    this.capture.addExternalMessage(message);
  }

  /** An entry that already has attachments (for feed tests): ids are stable, the files are "present". */
  addMessageWithAttachments(part: Partial<MessageView> & { id: string; bodyMd: string }, attachments: AttachmentView[]): MessageView {
    return this.attachments.addMessageWithAttachments(part, attachments);
  }

  /** A new agent log row (older rows come earlier in the call order). Returns its `id`. */
  addAgentLog(row: Partial<AgentLogRow> & Pick<AgentLogRow, "tool">, entries: AgentLogEntry[] = []): number {
    return this.agents.addAgentLog(row, entries);
  }

  /** Texts before each entry edit (`edit` revisions), oldest first. */
  revisionsOf(id: string): string[] {
    return this.state.rows.find((r) => r.view.id === id)?.revisions ?? [];
  }

  /** An entry as it is now in the "database" (including from the trash). */
  messageById(id: string): MessageView | undefined {
    return this.state.rows.find((r) => r.view.id === id)?.view;
  }

  /** Every live entry, for checks. */
  searchable(): readonly MessageView[] {
    return this.search.searchable();
  }

  /** The tiles above the field of a chat. */
  tilesOf(chatId: string): AttachmentView[] {
    return this.attachments.tilesOf(chatId);
  }

  /** Continue the import frozen by `holdImport` up to the next progress event. */
  releaseImport(): void {
    this.importer.release();
  }

  // ---- logs and knobs: read and set by tests and stories, kept by the area named in the comment ----

  get calls() { return this.state.calls; } // the command call log
  get openedUrls() { return this.app.openedUrls; }
  get restoredFrom() { return this.app.restoredFrom; }
  get restartRequested() { return this.app.restartRequested; }
  get resizeLog() { return this.capture.resizeLog; }
  get recordingLog() { return this.settings.recordingLog; }
  get lastImportChats(): readonly string[] { return this.importer.lastChats; }
  get importClosed() { return this.importer.closed; }
  get agentExcluded() { return this.agents.excluded; }
  get openSettingsCount() { return this.settings.openSettingsCount; }
  get openedEntries() { return this.settings.openedEntries; }
  get weekdaySources() { return this.forest.weekdaySources; }
  get openedAttachments() { return this.attachments.openedAttachments; }
  get sentVoices() { return this.voice.sentVoices; }
  get microphoneSettingsOpened() { return this.voice.microphoneSettingsOpened; }
  get voiceFiles() { return this.voice.voiceFiles; }
  get savedWaveforms() { return this.voice.savedWaveforms; }

  get hideCount() { return this.capture.hideCount; }
  set hideCount(count: number) { this.capture.hideCount = count; }
  get failNextSend() { return this.compose.failNextSend; }
  set failNextSend(code: ErrorCode | null) { this.compose.failNextSend = code; }
  get failNextPick() { return this.settings.failNextPick; }
  set failNextPick(code: ErrorCode | null) { this.settings.failNextPick = code; }
  get failNextShortcut() { return this.settings.failNextShortcut; }
  set failNextShortcut(code: ErrorCode | null) { this.settings.failNextShortcut = code; }
  get failNextCapture() { return this.capture.failNext; }
  set failNextCapture(code: ErrorCode | null) { this.capture.failNext = code; }
  get pickedFolder() { return this.settings.pickedFolder; }
  set pickedFolder(folder: string | null) { this.settings.pickedFolder = folder; }
  get settingsFile() { return this.settings.settingsFile; }
  set settingsFile(file: SettingsFileState) { this.settings.settingsFile = file; }
  get importFile() { return this.importer.file; }
  set importFile(file: MockImportFile) { this.importer.file = file; }
  get importReportPatch() { return this.importer.reportPatch; }
  set importReportPatch(patch: Partial<ImportReport>) { this.importer.reportPatch = patch; }
  get importSteps() { return this.importer.steps; }
  set importSteps(steps: number) { this.importer.steps = steps; }
  get holdImport() { return this.importer.hold; }
  set holdImport(hold: boolean) { this.importer.hold = hold; }
  get agentEnabled() { return this.agents.enabled; }
  set agentEnabled(enabled: boolean) { this.agents.enabled = enabled; }
  get agentMode() { return this.agents.mode; }
  set agentMode(mode: AgentMode) { this.agents.mode = mode; }
  get agentChats() { return this.agents.chats; }
  set agentChats(chatIds: string[]) { this.agents.chats = chatIds; }
  get agentSocket() { return this.agents.socket; }
  set agentSocket(socket: AgentSocket) { this.agents.socket = socket; }
  get dismissedOnThisDay() { return this.onThisDay.dismissed; }
  set dismissedOnThisDay(day: string | null) { this.onThisDay.dismissed = day; }
  get pickedFiles() { return this.attachments.pickedFiles; }
  set pickedFiles(names: string[] | null) { this.attachments.pickedFiles = names; }
}
