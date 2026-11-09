import { KinigeApiError } from "../KinigeApi";
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

  constructor(
    private readonly s: MockState,
    private readonly compose: MockCompose,
  ) {}

  /** The checks run in Rust's order: size, length, the chat, then the reply target. */
  sendVoice(
    chatId: string,
    bytes: Uint8Array,
    durationMs: number,
    replyToId: string | null = null,
    waveform: Uint8Array | null = null,
  ): Promise<MessageView> {
    const check = (): void => {
      if (bytes.length > LIMITS.pasteMaxBytes) throw new KinigeApiError("attachment_too_large");
      if (durationMs < LIMITS.voiceMinMs) throw new KinigeApiError("voice_too_short");
      if (durationMs > LIMITS.voiceMaxMs) throw new KinigeApiError("voice_too_long");
      if (!this.s.chats.some((c) => c.id === chatId)) throw new KinigeApiError("chat_not_found");
    };
    return this.s.run("sendVoice", `${chatId} ${bytes.length} ${durationMs}`, () => {
      check();
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
    });
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
}
