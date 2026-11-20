import { INBOX_ID } from "../../api/KinigeApi";
import type { MockApi } from "../../api/MockApi";
import type { StickersHost } from "./PickerTabs";

/** Storybook only: the host of the panel over a `MockApi`. A click sends into the mock Inbox; the dialogs the real window opens are no-ops here. */
export function makeStoryHost(api: MockApi): StickersHost {
  return {
    api,
    sendSticker: async (stickerId) => void (await api.sendSticker(INBOX_ID, stickerId)),
    sendGif: async (attachmentId) => void (await api.sendGifAgain(INBOX_ID, attachmentId)),
    onAddPack: () => undefined,
    onRemovePack: () => undefined,
    onChooseGif: () => undefined,
  };
}
