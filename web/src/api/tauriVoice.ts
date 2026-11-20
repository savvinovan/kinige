import { invoke } from "@tauri-apps/api/core";
import { peaksToHex } from "../voice/waveform";
import type { MessageView } from "./generated/MessageView";
import { call, toApiError } from "./tauriCall";

// The voice note commands of the main window (slice 7a) and of the capture window (KINIG-153), apart from `TauriApi` to keep that file
// short. Raw bodies and raw answers go through `invoke` directly; the rest through `call`.

/** `send_voice`: a raw body like a paste; the chat, the recorder's clock, the reply and the peaks go in headers (ASCII; the peaks as hex). */
export function sendVoice(
  chatId: string,
  bytes: Uint8Array,
  durationMs: number,
  replyToId?: string | null,
  waveform?: Uint8Array | null,
): Promise<MessageView> {
  const headers = recordingHeaders(durationMs, waveform);
  headers["x-chat-id"] = chatId;
  if (replyToId) headers["x-reply-to"] = encodeURIComponent(replyToId);
  return sendRaw("send_voice", bytes, headers);
}

/** `capture_send_voice`: the same body and headers without the chat and the reply: the capture window writes to the Inbox only. */
export function captureSendVoice(bytes: Uint8Array, durationMs: number, waveform?: Uint8Array | null): Promise<MessageView> {
  return sendRaw("capture_send_voice", bytes, recordingHeaders(durationMs, waveform));
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

/** `open_microphone_settings` and the capture window's own copy. */
export async function openMicrophoneSettings(): Promise<void> {
  await call("open_microphone_settings");
}

export async function captureOpenMicrophoneSettings(): Promise<void> {
  await call("capture_open_microphone_settings");
}

/** `microphone_hold` and the capture window's own copy: the answer to `true` is whether the other window left the microphone to this one. */
export function holdMicrophone(held: boolean): Promise<boolean> {
  return call("microphone_hold", { held });
}

export function captureHoldMicrophone(held: boolean): Promise<boolean> {
  return call("capture_hold_microphone", { held });
}

function recordingHeaders(durationMs: number, waveform?: Uint8Array | null): Record<string, string> {
  const headers: Record<string, string> = { "x-duration-ms": String(Math.round(durationMs)) };
  if (waveform && waveform.length > 0) headers["x-waveform"] = peaksToHex(waveform);
  return headers;
}

async function sendRaw(command: string, bytes: Uint8Array, headers: Record<string, string>): Promise<MessageView> {
  try {
    return await invoke<MessageView>(command, bytes, { headers });
  } catch (reason) {
    throw toApiError(reason);
  }
}
