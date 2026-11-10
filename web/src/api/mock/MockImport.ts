import { KinigeApiError } from "../KinigeApi";
import type { ErrorCode } from "../generated/ErrorCode";
import type { ImportOpened } from "../generated/ImportOpened";
import type { ImportPreview } from "../generated/ImportPreview";
import type { ImportProgress } from "../generated/ImportProgress";
import type { ImportReport } from "../generated/ImportReport";
import type { MockState } from "./MockState";

/** The import file choice in the mock: what the dialog will "pick". */
export type MockImportFile = ImportPreview | ErrorCode | null;

/** The backup name that the mock report gives. */
export const MOCK_BACKUP_NAME = "pre-import-20261028T125000Z.db";

/** The Telegram import in `MockApi`, with progress, stopping and a pause for tests. The real import is in Rust; nothing is written here. */
export class MockImport {
  /** What the next "Choose result.json..." returns: a preview, `null` (the dialog was cancelled) or an error code. */
  file: MockImportFile = null;
  /** Corrections to the report that the mock builds from the chosen chats. */
  reportPatch: Partial<ImportReport> = {};
  /** How many progress events the mock sends per import (the last one is "all"). */
  steps = 3;
  /** The import freezes after the first progress event until the test calls `release()`. */
  hold = false;
  /** The import window was closed this many times (`importTelegramClose`). */
  closed = 0;
  /** The chats chosen in the last run. */
  lastChats: readonly string[] = [];

  private session = 0;
  private preview: ImportPreview | null = null;
  private running = false;
  private stopRequested = false;
  private releaseHold: (() => void) | null = null;
  private readonly progressListeners = new Set<(progress: ImportProgress) => void>();

  constructor(private readonly s: MockState) {}

  importTelegramOpen(): Promise<ImportOpened | null> {
    return this.s.run("importTelegramOpen", "", () => {
      const file = this.file;
      if (file === null) return null;
      if (typeof file === "string") throw new KinigeApiError(file);
      this.session += 1;
      this.preview = file;
      return { session: this.session, preview: file };
    });
  }

  async importTelegramRun(session: number, chatIds: readonly string[]): Promise<ImportReport> {
    this.s.calls.push(`importTelegramRun ${session} ${chatIds.join(",")}`);
    const code = this.s.takeFailure("importTelegramRun");
    if (code) throw new KinigeApiError(code);
    if (this.running) throw new KinigeApiError("import_in_progress");
    if (session !== this.session || !this.preview) throw new KinigeApiError("import_session_expired");
    this.running = true;
    this.stopRequested = false;
    this.lastChats = chatIds;
    try {
      const chosen = this.preview.chats.filter((c) => chatIds.includes(c.id));
      const total = chosen.reduce((sum, c) => sum + c.total, 0);
      const bytesTotal = chosen.reduce((sum, c) => sum + c.mediaBytes, 0);
      const steps = Math.max(1, this.steps);
      let stopped = false;
      for (let step = 1; step <= steps; step++) {
        const done = Math.round((total * step) / steps);
        const bytesDone = Math.round((bytesTotal * step) / steps);
        for (const handler of [...this.progressListeners]) handler({ done, total, bytesDone, bytesTotal });
        if (step < steps) {
          if (this.hold) await new Promise<void>((resolve) => (this.releaseHold = resolve));
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
        ...this.reportPatch,
      };
      return report;
    } finally {
      this.running = false;
    }
  }

  importTelegramStop(): Promise<void> {
    this.s.calls.push("importTelegramStop");
    this.stopRequested = true;
    this.release();
    return Promise.resolve();
  }

  importTelegramClose(): Promise<void> {
    this.s.calls.push("importTelegramClose");
    this.closed += 1;
    this.preview = null;
    return Promise.resolve();
  }

  onImportProgress(handler: (progress: ImportProgress) => void): Promise<() => void> {
    this.progressListeners.add(handler);
    return Promise.resolve(() => {
      this.progressListeners.delete(handler);
    });
  }

  /** Continue the import frozen by `hold` up to the next progress event. */
  release(): void {
    this.releaseHold?.();
    this.releaseHold = null;
  }
}
