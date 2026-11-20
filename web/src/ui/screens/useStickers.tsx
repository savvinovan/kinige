import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessageView } from "../../api/generated/MessageView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import { AddPackHost } from "../stickers/AddPackHost";
import type { PickerTab, StickersHost, TabMemory } from "../stickers/PickerTabs";
import { RemovePackHost } from "../stickers/RemovePackDialog";

interface UseStickersOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash, where there is no composer and so no panel. */
  selected: string | null;
  /** The entry the draft replies to; only a live one goes with a sticker (Rust refuses a reply to anything else). */
  reply: ReplyTo | null;
  appendSent: (sent: MessageView) => void;
  refreshChats: () => Promise<void>;
  changeReply: (reply: ReplyTo | null) => void;
  /** The error above the composer. */
  onError: (code: ErrorCode | null) => void;
  /** "Choose a GIF file…": the paperclip flow. */
  onChooseGif: () => void;
}

/**
 * The Stickers and GIFs tabs of the emoji window and their two dialogs in the main window (slice 5c). A click sends at once as its own entry in the
 * open chat, with the pending reply as `sendVoice` does; the text draft and the tiles are not touched. The entry appears after Rust replies (no
 * optimistic display); an error stands above the field and the panel closes so it is seen. `host` goes to the composer's emoji button, `overlay`
 * (the add-pack dialog and the remove question) to the window's overlay.
 */
export function useStickers({ api, selected, reply, appendSent, refreshChats, changeReply, onError, onChooseGif }: UseStickersOptions) {
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<StickerPackView | null>(null);
  // The tab the panel was left on, for this window's session only: in memory, not a setting.
  const lastTab = useRef<PickerTab>("emoji");
  const tabMemory = useMemo<TabMemory>(
    () => ({
      recall: () => lastTab.current,
      remember: (tab) => {
        lastTab.current = tab;
      },
    }),
    [],
  );
  const replyId = reply !== null && reply.state === "live" ? reply.id : null;

  const deliver = useCallback(
    (send: (chatId: string) => Promise<MessageView>): Promise<void> => {
      if (selected === null) return Promise.reject(new Error("no chat"));
      return send(selected).then(
        (sent) => {
          appendSent(sent);
          void refreshChats();
          if (sent.replyTo) changeReply(null);
          onError(null);
        },
        (reason: unknown) => {
          onError(errorCode(reason));
          throw reason;
        },
      );
    },
    [selected, appendSent, refreshChats, changeReply, onError],
  );

  const host = useMemo<StickersHost | undefined>(
    () =>
      selected === null
        ? undefined
        : {
            api,
            sendSticker: (stickerId) => deliver((chatId) => api.sendSticker(chatId, stickerId, replyId)),
            sendGif: (attachmentId) => deliver((chatId) => api.sendGifAgain(chatId, attachmentId, replyId)),
            onAddPack: () => setAdding(true),
            onRemovePack: setRemoving,
            onChooseGif,
            tabMemory,
          },
    [api, selected, deliver, replyId, onChooseGif, tabMemory],
  );

  const overlay: ReactNode = (
    <>
      {adding ? <AddPackHost api={api} onClose={() => setAdding(false)} /> : null}
      {removing ? <RemovePackHost api={api} pack={removing} onClose={() => setRemoving(null)} /> : null}
    </>
  );
  return { host, overlay };
}
