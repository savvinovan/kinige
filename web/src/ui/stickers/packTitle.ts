import type { TFunction } from "i18next";
import type { StickerPackView } from "../../api/generated/StickerPackView";

/**
 * What a pack is called in the panel: the pack made from the Telegram export is stored under an English title and shown in the interface language;
 * a pack added by link keeps the title Telegram gave it (cleaned and capped at 128 characters in Rust; the panel cuts a long one with an ellipsis).
 */
export function packTitle(pack: Pick<StickerPackView, "source" | "title">, t: TFunction): string {
  return pack.source === "telegram_export" ? t("stickers.fromTelegram") : pack.title;
}
