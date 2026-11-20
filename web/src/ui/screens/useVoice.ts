import { useEffect, useRef } from "react";
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
 *
 * Closing the window (the red button, ⌘W) hides it, and a hidden window must not go on recording (KINIG-156): Rust tells the page first, and
 * the recording is stopped and kept ("Recording stopped", Send and Discard) with the microphone let go, as the capture window does when it hides.
 * Minimizing and ⌘H send no such event: they keep recording.
 */
export function useVoice({ api, selected, reply, appendSent, refreshChats, changeReply }: UseVoiceOptions) {
  const voice = useVoiceController({
    api,
    chatId: selected,
    replyToId: reply !== null && reply.state === "live" ? reply.id : null,
    onSent: (sent, chatId) => {
      appendSent(sent);
      void refreshChats();
      if (chatId === selected && sent.replyTo) changeReply(null);
    },
  });

  // The listener is set once per api; what it calls is always the recorder's current `park`.
  const park = useRef(voice.park);
  useEffect(() => {
    park.current = voice.park;
  });
  useEffect(() => {
    let closed = false;
    let off: (() => void) | undefined;
    api
      .onMainWindowHiding(() => void park.current())
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api]);

  return voice;
}
