import { INBOX_ID, KinigeApiError } from "../KinigeApi";
import { CONTEXT_LEN, mockMessage } from "../mockData";
import { canAppendTo, joinBody, looksLikeEmoji } from "../mockRules";
import type { CaptureOutcome } from "../generated/CaptureOutcome";
import type { CaptureState } from "../generated/CaptureState";
import type { ErrorCode } from "../generated/ErrorCode";
import type { MessageView } from "../generated/MessageView";
import type { MockState } from "./MockState";

/** The capture window in `MockApi`: its state, the draft, writing or appending to the last Inbox entry, hiding and resizing. */
export class MockCapture {
  /** The next send or append from the capture window fails with this error. */
  failNext: ErrorCode | null = null;
  /** The capture window was hidden by the `captureHide` command this many times. */
  hideCount = 0;
  /** Capture window heights that the page asked for with the `captureResize` command. */
  readonly resizeLog: number[] = [];
  private draft = "";

  constructor(private readonly s: MockState) {}

  captureState(): Promise<CaptureState> {
    const now = this.s.now();
    const today = this.s.journalDate(now);
    const inbox = this.s.live(INBOX_ID);
    const context = inbox.filter((m) => m.journalDate === today).slice(-CONTEXT_LEN);
    const last = inbox[inbox.length - 1];
    return Promise.resolve({
      language: this.s.language,
      textSize: this.s.settings.textSize,
      today,
      context,
      appendTarget: last && canAppendTo(last, now.getTime()) ? last.id : null,
      draft: this.draft,
      error: null,
      recentEmoji: this.s.settings.recentEmoji,
    });
  }

  /** The emoji chosen in the capture window picker: the shared "recent" list (the same as the main window's). */
  captureAddRecentEmoji(emoji: string): Promise<string[]> {
    if (!looksLikeEmoji(emoji)) return Promise.reject(new KinigeApiError("chat_icon_invalid"));
    return Promise.resolve(this.s.addRecent(emoji));
  }

  captureSend(body: string): Promise<CaptureOutcome> {
    return this.capture(body, false);
  }

  captureAppend(body: string): Promise<CaptureOutcome> {
    return this.capture(body, true);
  }

  captureSaveDraft(body: string): Promise<void> {
    this.draft = body;
    return Promise.resolve();
  }

  captureHide(): Promise<void> {
    this.hideCount += 1;
    return Promise.resolve();
  }

  captureResize(height: number): Promise<void> {
    this.resizeLog.push(height);
    return Promise.resolve();
  }

  /** An entry that appeared "from outside" (the capture window, the clipboard): reaches the feed after the event. */
  addExternalMessage(message: MessageView): void {
    this.s.rows.push({ view: message, deletedAt: null, revisions: [] });
  }

  private capture(body: string, append: boolean): Promise<CaptureOutcome> {
    if (this.failNext) {
      const code = this.failNext;
      this.failNext = null;
      return Promise.reject(new KinigeApiError(code));
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "") return Promise.reject(new KinigeApiError("empty_message"));
    const now = this.s.now();
    const inbox = this.s.live(INBOX_ID);
    const last = inbox[inbox.length - 1];
    this.draft = "";
    if (append && last && canAppendTo(last, now.getTime())) {
      const message = this.s.setBody(this.s.row(last.id), joinBody(last.bodyMd, text));
      this.s.emitEntriesChanged();
      return Promise.resolve({ action: "appended", message });
    }
    const message = mockMessage({
      id: `mock-${++this.s.counter}`,
      chatId: INBOX_ID,
      bodyMd: text,
      journalDate: this.s.journalDate(now),
      createdAt: now.getTime(),
    });
    this.s.rows.push({ view: message, deletedAt: null, revisions: [] });
    this.s.emitEntriesChanged();
    return Promise.resolve({ action: "created", message });
  }
}
