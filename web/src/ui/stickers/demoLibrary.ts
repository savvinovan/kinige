import type { JournalGif } from "../../api/generated/JournalGif";
import type { StickerLibrary } from "../../api/generated/StickerLibrary";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import type { StickerView } from "../../api/generated/StickerView";
import { registerDemoSticker } from "./stickerArt";

// Synthetic libraries for Storybook (rules 9 and 10: no real sticker, nothing from Telegram): the pictures are drawn by `stickerArt`.

const EMOJI = ["\u{1F63A}", "\u{1F431}", "\u{1F436}", "\u{1F98A}", "\u{1F43C}", "\u{1F984}", "\u{1F438}", "\u{1F419}", "\u{1F422}", "\u{1F989}"];

/** A static sticker has one side of exactly 512 pixels. */
const SIDE = 512;

export interface DemoPack {
  title: string;
  count: number;
  source?: StickerPackView["source"];
}

/** A pack of `count` synthetic stickers; the ids are stable, so the same story draws the same pictures. */
export function demoPack(id: string, { title, count, source = "telegram_link" }: DemoPack): StickerPackView {
  const stickers: StickerView[] = Array.from({ length: count }, (_, index) => {
    const emoji = EMOJI[index % EMOJI.length] ?? null;
    const stickerId = `${id}-${index}`;
    registerDemoSticker(stickerId, emoji);
    return { id: stickerId, emoji, width: SIDE, height: SIDE };
  });
  return { id, source, title, stickers };
}

/** A library: the packs in the order given, and the `recent` first stickers of the first pack as the recent ones (the newest first). */
export function demoLibrary(packs: readonly DemoPack[], options: { recent?: number; hasToken?: boolean } = {}): StickerLibrary {
  const built = packs.map((pack, index) => demoPack(`pack-${index}`, pack));
  const recent = (built[0]?.stickers ?? []).slice(0, options.recent ?? 0).reverse();
  return { recent, packs: built, hasToken: options.hasToken ?? true };
}

/** `count` GIFs of the journal, the newest first, in a few shapes. */
export function demoGifs(count: number): JournalGif[] {
  const shapes = [
    [480, 270],
    [320, 320],
    [270, 480],
    [640, 360],
  ] as const;
  return Array.from({ length: count }, (_, index) => {
    const [width, height] = shapes[index % shapes.length] ?? shapes[0];
    return { attachmentId: `demo-gif-${index}`, width, height, byteSize: 600_000 + index * 50_000 };
  });
}
