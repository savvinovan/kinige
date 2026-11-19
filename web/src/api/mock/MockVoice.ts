import { INBOX_ID, KinigeApiError } from "../KinigeApi";
import { LIMITS } from "../mockRules";
import type { MessageView } from "../generated/MessageView";
import { mockAttachment } from "./MockAttachments";
import type { MockCompose } from "./MockCompose";
import type { MockState } from "./MockState";

/** A voice note that reached "Rust": chat, size and the recorder's clock, in the order they were saved. */
export interface SentVoice {
  chatId: string;
  bytes: number;
  durationMs: number;
  replyToId: string | null;
  /** The peaks that came with the note (null when the recorder sent none). */
  waveform: Uint8Array | null;
}

/**
 * Voice notes in `MockApi` (slice 7a, ADR-0015): sending a note, the waveform peaks and the microphone settings page. The rules are the
 * ones of `Store::send_voice` (`fixtures/api-contract/attachment-errors.json`); the container is not looked at (the mock has no files).
 */
export class MockVoice {
  readonly sentVoices: SentVoice[] = [];
  /** The system's microphone privacy page was asked for this many times. */
  microphoneSettingsOpened = 0;
  /** What `readVoiceForWaveform` hands out by attachment id; an id not in here is `attachment_not_found`. */
  readonly voiceFiles = new Map<string, ArrayBuffer>();
  /** The peaks saved through `setAttachmentWaveform`, in order. */
  readonly savedWaveforms: { attachmentId: string; peaks: Uint8Array }[] = [];
  /** The window with the microphone claim (Rust's `MicrophoneClaim`): one at a time, taking it again is fine, a window frees only its own. */
  microphoneHolder: "main" | "capture" | null = null;

  constructor(
    private readonly s: MockState,
    private readonly compose: MockCompose,
  ) {}

  sendVoice(
    chatId: string,
    bytes: Uint8Array,
    durationMs: number,
    replyToId: string | null = null,
    waveform: Uint8Array | null = null,
  ): Promise<MessageView> {
    return this.s.run("sendVoice", `${chatId} ${bytes.length} ${durationMs}`, () =>
      this.save(chatId, bytes, durationMs, replyToId, waveform),
    );
  }

  readVoiceForWaveform(attachmentId: string): Promise<ArrayBuffer> {
    return this.s.run("readVoiceForWaveform", attachmentId, () => {
      const file = this.voiceFiles.get(attachmentId);
      if (!file) throw new KinigeApiError("attachment_not_found");
      return file;
    });
  }

  setAttachmentWaveform(attachmentId: string, peaks: Uint8Array): Promise<void> {
    return this.s.run("setAttachmentWaveform", attachmentId, () => {
      if (peaks.length !== 0 && peaks.length !== LIMITS.waveformPeaks) throw new KinigeApiError("attachment_unreadable");
      this.savedWaveforms.push({ attachmentId, peaks });
    });
  }

  openMicrophoneSettings(): Promise<void> {
    return this.s.run("openMicrophoneSettings", "", () => {
      this.microphoneSettingsOpened += 1;
    });
  }

  /** The capture window's recording: always a new entry in the Inbox, no reply (`capture_send_voice`). The main window hears about it. */
  captureSendVoice(bytes: Uint8Array, durationMs: number, waveform: Uint8Array | null = null): Promise<MessageView> {
    return this.s.run("captureSendVoice", `${bytes.length} ${durationMs}`, () => {
      const sent = this.save(INBOX_ID, bytes, durationMs, null, waveform);
      this.s.emitEntriesChanged();
      return sent;
    });
  }

  /** The capture window's microphone page: the same fixed page. */
  captureOpenMicrophoneSettings(): Promise<void> {
    return this.s.run("captureOpenMicrophoneSettings", "", () => {
      this.microphoneSettingsOpened += 1;
    });
  }

  holdMicrophone(window: "main" | "capture", held: boolean): Promise<boolean> {
    const method = window === "main" ? "holdMicrophone" : "captureHoldMicrophone";
    return this.s.run(method, String(held), () => {
      if (!held) {
        if (this.microphoneHolder === window) this.microphoneHolder = null;
        return true;
      }
      if (this.microphoneHolder !== null && this.microphoneHolder !== window) return false;
      this.microphoneHolder = window;
      return true;
    });
  }

  /** The checks run in Rust's order: size, length, the chat, then the reply target. */
  private save(
    chatId: string,
    bytes: Uint8Array,
    durationMs: number,
    replyToId: string | null,
    waveform: Uint8Array | null,
  ): MessageView {
    if (bytes.length > LIMITS.pasteMaxBytes) throw new KinigeApiError("attachment_too_large");
    if (durationMs < LIMITS.voiceMinMs) throw new KinigeApiError("voice_too_short");
    if (durationMs > LIMITS.voiceMaxMs) throw new KinigeApiError("voice_too_long");
    if (!this.s.chats.some((c) => c.id === chatId)) throw new KinigeApiError("chat_not_found");
    const sent = this.compose.deliver(chatId, "", replyToId, () => [], true);
    const row = this.s.row(sent.id);
    const note = mockAttachment(`att-${++this.s.counter}`, "voice-2026-11-04-213012.m4a", {
      kind: "audio",
      mime: "audio/mp4",
      byteSize: bytes.length,
      durationMs: Math.round(durationMs),
      // The same rule as the store: only a full set of peaks is kept.
      waveform: waveform && waveform.length === LIMITS.waveformPeaks ? Array.from(waveform) : null,
    });
    row.view = { ...row.view, attachments: [note] };
    this.sentVoices.push({ chatId, bytes: bytes.length, durationMs, replyToId, waveform });
    return this.s.viewOf(row);
  }
}
