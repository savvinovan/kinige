import type { KinigeApi } from "../../api/KinigeApi";
import type { MessageView } from "../../api/generated/MessageView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useVoiceController } from "../../voice/useVoiceRecorder";

interface UseVoiceOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash. */
  selected: string | null;
  /** The entry the draft replies to; only a live one is carried (Rust refuses a reply to anything else). */
  reply: ReplyTo | null;
  appendSent: (sent: MessageView) => void;
  refreshChats: () => Promise<void>;
  changeReply: (reply: ReplyTo | null) => void;
}

/**
 * The microphone of the main window (slice 7a): the recorder of `voice/` with what the window does when a note is saved. The note appears in
 * its own chat's feed like a sent entry; the reply line is gone (Rust cleared it from the saved draft, here the strip is cleared if the
 * person is still in that chat). No toast and no sound: as in Telegram, the entry simply appears.
 */
export function useVoice({ api, selected, reply, appendSent, refreshChats, changeReply }: UseVoiceOptions) {
  return useVoiceController({
    api,
    chatId: selected,
    replyToId: reply !== null && reply.state === "live" ? reply.id : null,
    onSent: (sent, chatId) => {
      appendSent(sent);
      void refreshChats();
      if (chatId === selected && sent.replyTo) changeReply(null);
    },
  });
}
