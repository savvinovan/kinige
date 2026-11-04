import { type ListMessagesOptions, KinigeApiError } from "./KinigeApi";
import { MockApiBase } from "./MockApiBase";
import { resolveColor } from "./mockColor";
import { compareFeed, isRealDate } from "./mockData";
import type { ErrorCode } from "./generated/ErrorCode";
import type { FeedCursor } from "./generated/FeedCursor";
import type { ImportOpened } from "./generated/ImportOpened";
import type { ImportPreview } from "./generated/ImportPreview";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportReport } from "./generated/ImportReport";
import type { MessagePage } from "./generated/MessagePage";
import type { MessageView } from "./generated/MessageView";
import type { SearchFilters } from "./generated/SearchFilters";
import type { SearchHit } from "./generated/SearchHit";
import type { SearchPage } from "./generated/SearchPage";

/** The import file choice in the mock: what the dialog will "pick". */
export type MockImportFile = ImportPreview | ErrorCode | null;

/** The backup name that the mock report gives. */
export const MOCK_BACKUP_NAME = "pre-import-20261028T125000Z.db";

// Cyrillic "yo" -> "ye" (no Cyrillic literals in code: UI strings live in locales).
const YO = String.fromCharCode(0x451);
const YE = String.fromCharCode(0x435);
const normalize = (text: string) => text.toLowerCase().replaceAll(YO, YE);
const cursorOf = (m: { journalDate: string; createdAt: number; id: string }): FeedCursor => ({
  journalDate: m.journalDate,
  createdAt: m.createdAt,
  id: m.id,
});

/**
 * Slice 4 in `MockApi`: feed pages around an entry and newer ones, search (a word substring without stemming is fine in the
 * mock, the real word forms are computed by Rust) and the Telegram import with progress, stopping and a pause for tests.
 */
export class MockApiSlice4 extends MockApiBase {
  /** What the next "Choose result.json..." returns: a preview, `null` (the dialog was cancelled) or an error code. */
  importFile: MockImportFile = null;
  /** Corrections to the report that the mock builds from the chosen chats. */
  importReportPatch: Partial<ImportReport> = {};
  /** How many progress events the mock sends per import (the last one is "all"). */
  importSteps = 3;
  /** The import freezes after the first progress event until the test calls `releaseImport()`. */
  holdImport = false;
  /** The import window was closed this many times (`importTelegramClose`). */
  importClosed = 0;
  /** The chats chosen in the last run. */
  lastImportChats: readonly string[] = [];

  private importSession = 0;
  private importPreview: ImportPreview | null = null;
  private importRunning = false;
  private stopRequested = false;
  private release: (() => void) | null = null;
  private readonly progressListeners = new Set<(progress: ImportProgress) => void>();

  listMessages(chatId: string, options: ListMessagesOptions = {}): Promise<MessagePage> {
    // Not through `run`: the feed does not keep a call log (slice 3 command checks read it).
    try {
      return Promise.resolve(this.page(chatId, options));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  private page(chatId: string, options: ListMessagesOptions): MessagePage {
    const tag = options.tag?.toLowerCase();
    const all = this.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null && (tag === undefined || r.view.tags.includes(tag)))
      .map((r) => this.viewOf(r))
      .sort(compareFeed);
    const limit = options.limit ?? 200;
    let start: number;
    let end: number;
    if (options.around !== undefined) {
      const index = all.findIndex((m) => m.id === options.around);
      if (index < 0) throw new KinigeApiError("message_not_found");
      const before = Math.floor((limit - 1) / 2);
      start = Math.max(0, index - before);
      end = Math.min(all.length, index + (limit - 1 - before) + 1);
    } else if (options.after) {
      const after = options.after;
      start = all.findIndex((m) => compareFeed(m, after) > 0);
      if (start < 0) start = all.length;
      end = Math.min(all.length, start + limit);
    } else {
      end = options.before ? all.filter((m) => compareFeed(m, options.before as FeedCursor) < 0).length : all.length;
      start = Math.max(0, end - limit);
    }
    const page = all.slice(start, end);
    const first = page[0];
    const last = page.at(-1);
    return {
      messages: page,
      older: start > 0 && first ? cursorOf(first) : null,
      newer: end < all.length && last ? cursorOf(last) : null,
      today: this.journalDate(this.now()),
    };
  }

  searchMessages(
    query: string,
    options: { filters?: SearchFilters; before?: FeedCursor; limit?: number } = {},
  ): Promise<SearchPage> {
    return this.run("searchMessages", query, () => {
      const filters: SearchFilters = options.filters ?? { chatId: null, favorite: false, tasks: false, from: null, to: null };
      for (const date of [filters.from, filters.to]) {
        if (date != null && !isRealDate(date)) throw new KinigeApiError("invalid_date");
      }
      const words = normalize(query).split(/[^\p{L}\p{N}]+/u).filter((w) => w !== "");
      if (words.length === 0) return { chats: [], hits: [], older: null };
      const limit = options.limit ?? 50;
      const matches = this.rows
        .filter((r) => r.deletedAt === null)
        .map((r) => r.view)
        .filter((m) => {
          const text = normalize(m.bodyMd);
          return words.every((w) => text.includes(w));
        })
        .filter((m) => !filters.chatId || m.chatId === filters.chatId)
        .filter((m) => !filters.favorite || m.favoriteAt !== null)
        .filter((m) => !filters.tasks || m.isTask)
        .filter((m) => !filters.from || m.journalDate >= filters.from)
        .filter((m) => !filters.to || m.journalDate <= filters.to)
        .filter((m) => !options.before || compareFeed(m, options.before) < 0)
        .sort((a, b) => compareFeed(b, a));
      const page = matches.slice(0, limit);
      const hits: SearchHit[] = page.map((message) => {
        const chat = this.chats.find((c) => c.id === message.chatId);
        const terms = [
          ...new Set(
            (message.bodyMd.match(/[\p{L}\p{N}]+/gu) ?? []).filter((w) => words.some((q) => normalize(w).includes(q))),
          ),
        ];
        return {
          message,
          chatTitle: chat?.title ?? "",
          chatIcon: chat?.icon ?? null,
          chatIsInbox: chat?.isInbox ?? false,
          chatColor: chat?.color ?? resolveColor(message.chatId, null),
          snippet: message.bodyMd.replace(/\s+/g, " ").slice(0, 160),
          terms,
        };
      });
      const lastHit = page.at(-1);
      const chats = options.before
        ? []
        : this.chats.filter((c) => !c.isInbox && words.every((w) => normalize(c.title).includes(w)));
      return {
        chats,
        hits,
        older: matches.length > limit && lastHit ? cursorOf(lastHit) : null,
      };
    });
  }

  importTelegramOpen(): Promise<ImportOpened | null> {
    return this.run("importTelegramOpen", "", () => {
      const file = this.importFile;
      if (file === null) return null;
      if (typeof file === "string") throw new KinigeApiError(file);
      this.importSession += 1;
      this.importPreview = file;
      return { session: this.importSession, preview: file };
    });
  }

  async importTelegramRun(session: number, chatIds: readonly string[]): Promise<ImportReport> {
    this.calls.push(`importTelegramRun ${session} ${chatIds.join(",")}`);
    const code = this.failures.get("importTelegramRun");
    if (code) {
      this.failures.delete("importTelegramRun");
      throw new KinigeApiError(code);
    }
    if (this.importRunning) throw new KinigeApiError("import_in_progress");
    if (session !== this.importSession || !this.importPreview) throw new KinigeApiError("import_session_expired");
    this.importRunning = true;
    this.stopRequested = false;
    this.lastImportChats = chatIds;
    try {
      const chosen = this.importPreview.chats.filter((c) => chatIds.includes(c.id));
      const total = chosen.reduce((sum, c) => sum + c.total, 0);
      const bytesTotal = chosen.reduce((sum, c) => sum + c.mediaBytes, 0);
      const steps = Math.max(1, this.importSteps);
      let stopped = false;
      for (let step = 1; step <= steps; step++) {
        const done = Math.round((total * step) / steps);
        const bytesDone = Math.round((bytesTotal * step) / steps);
        for (const handler of [...this.progressListeners]) handler({ done, total, bytesDone, bytesTotal });
        if (step < steps) {
          if (this.holdImport) await new Promise<void>((resolve) => (this.release = resolve));
          if (this.stopRequested) {
            stopped = true;
            break;
          }
        }
      }
      const fresh = chosen.reduce((sum, c) => sum + (c.total - c.existing), 0);
      const report: ImportReport = {
        imported: fresh,
        skippedExisting: chosen.reduce((sum, c) => sum + c.existing, 0),
        skippedService: 0,
        skippedEmpty: 0,
        mediaPlaceholders: Math.floor(fresh / 2),
        mediaAttached: chosen.reduce((sum, c) => sum + c.mediaFiles, 0),
        mediaBytes: bytesTotal,
        mediaMissing: chosen.reduce((sum, c) => sum + c.mediaMissing, 0),
        mediaSkipped: 0,
        diskFull: false,
        chatsCreated: chosen.filter((c) => c.kind === "personal" && c.existing === 0 && c.total > 0).length,
        backupName: fresh > 0 ? MOCK_BACKUP_NAME : null,
        stopped,
        ...this.importReportPatch,
      };
      return report;
    } finally {
      this.importRunning = false;
    }
  }

  importTelegramStop(): Promise<void> {
    this.calls.push("importTelegramStop");
    this.stopRequested = true;
    this.releaseImport();
    return Promise.resolve();
  }

  importTelegramClose(): Promise<void> {
    this.calls.push("importTelegramClose");
    this.importClosed += 1;
    this.importPreview = null;
    return Promise.resolve();
  }

  onImportProgress(handler: (progress: ImportProgress) => void): Promise<() => void> {
    this.progressListeners.add(handler);
    return Promise.resolve(() => {
      this.progressListeners.delete(handler);
    });
  }

  /** Continue the import frozen by `holdImport` up to the next progress event. */
  releaseImport(): void {
    this.release?.();
    this.release = null;
  }

  /** The entry opened by a jump from a search result, for checks. */
  searchable(): readonly MessageView[] {
    return this.rows.filter((r) => r.deletedAt === null).map((r) => r.view);
  }
}
