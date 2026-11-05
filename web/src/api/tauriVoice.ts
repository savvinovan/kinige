import { invoke } from "@tauri-apps/api/core";
import { peaksToHex } from "../voice/waveform";
import type { MessageView } from "./generated/MessageView";
import { call, toApiError } from "./tauriCall";

// The voice note commands of the main window (slice 7a), apart from `TauriApi` to keep that file short. Raw bodies and raw answers go through
// `invoke` directly; the rest through `call`.

/** `send_voice`: a raw body like a paste; the chat, the recorder's clock, the reply and the peaks go in headers (ASCII; the peaks as hex). */
export async function sendVoice(
  chatId: string,
  bytes: Uint8Array,
  durationMs: number,
  replyToId?: string | null,
  waveform?: Uint8Array | null,
): Promise<MessageView> {
  const headers: Record<string, string> = { "x-chat-id": chatId, "x-duration-ms": String(Math.round(durationMs)) };
  if (replyToId) headers["x-reply-to"] = encodeURIComponent(replyToId);
  if (waveform && waveform.length > 0) headers["x-waveform"] = peaksToHex(waveform);
  try {
    return await invoke<MessageView>("send_voice", bytes, { headers });
  } catch (reason) {
    throw toApiError(reason);
  }
}

/** `read_voice_for_waveform`: a raw answer, the file's bytes, which arrive as an ArrayBuffer. */
export async function readVoiceForWaveform(attachmentId: string): Promise<ArrayBuffer> {
  try {
    return await invoke<ArrayBuffer>("read_voice_for_waveform", { attachmentId });
  } catch (reason) {
    throw toApiError(reason);
  }
}

/** `set_attachment_waveform`: 64 bytes (hex) or empty for "could not decode". */
export async function setAttachmentWaveform(attachmentId: string, peaks: Uint8Array): Promise<void> {
  await call("set_attachment_waveform", { attachmentId, peaks: peaksToHex(peaks) });
}
