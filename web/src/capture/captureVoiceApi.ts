import type { KinigeApi } from "../api/KinigeApi";
import type { VoiceApi } from "../voice/useVoiceRecorder";

/**
 * The recorder of the quick capture window (KINIG-153) is the main window's own hook; this maps what it asks of the API onto the capture
 * window's three commands. The chat and the reply the hook passes are ignored: a recording from here is always a new entry in the Inbox,
 * and the window has no command that could name another chat.
 */
export function captureVoiceApi(api: KinigeApi): VoiceApi {
  return {
    sendVoice: (_chatId, bytes, durationMs, _replyToId, waveform) => api.captureSendVoice(bytes, durationMs, waveform),
    holdMicrophone: (held) => api.captureHoldMicrophone(held),
    openMicrophoneSettings: () => api.captureOpenMicrophoneSettings(),
  };
}
