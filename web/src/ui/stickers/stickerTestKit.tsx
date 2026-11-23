import { useRef } from "react";
import { type Mock, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { MockApi } from "../../api/MockApi";
import { StubEmojiData } from "../../test/emoji";
import { EmojiPopover } from "../emoji/EmojiPopover";
import { DemoMedia } from "../attachments/demo";
import type { StickersHost } from "./PickerTabs";

/** The pieces the sticker panel tests share: a MockApi with a library, a host that talks to it, and the popover on top of a button. */
export interface Panel {
  api: MockApi;
  onClose: Mock<(returnFocus: boolean) => void>;
  onPick: Mock<(emoji: string) => void>;
  /** `onAddPack`, `onRemovePack` and `onChooseGif` are `vi.fn()`s. */
  host: StickersHost;
}

/** A MockApi with the "From Telegram" pack (6 stickers) and a pack "Cats" (3), both with their emoji. */
export function seededApi(): MockApi {
  const api = new MockApi({ language: "en", now: () => new Date(Date.UTC(2026, 10, 20, 12, 0)) });
  api.stickerLibrary.seedPack("From Telegram", 6, "telegram_export");
  api.stickerLibrary.seedPack("Cats", 3);
  return api;
}

/** The host of the main window over a MockApi: the open chat and the pending reply are fixed here. */
export function makeHost(api: MockApi, replyId: string | null = null): StickersHost {
  return {
    api,
    sendSticker: async (stickerId) => void (await api.sendSticker(INBOX_ID, stickerId, replyId)),
    sendGif: async (attachmentId) => void (await api.sendGifAgain(INBOX_ID, attachmentId, replyId)),
    onAddPack: vi.fn(),
    onRemovePack: vi.fn(),
    onChooseGif: vi.fn(),
  };
}

function Harness({ panel }: { panel: Panel }) {
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <StubEmojiData>
      <DemoMedia>
        <button ref={anchor}>anchor</button>
        <EmojiPopover recent={[]} language="en" anchor={anchor} onClose={panel.onClose} onPick={panel.onPick} stickers={panel.host} />
      </DemoMedia>
    </StubEmojiData>
  );
}

export function panelElement(panel: Panel) {
  return <Harness panel={panel} />;
}
