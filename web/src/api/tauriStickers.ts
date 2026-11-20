import { listen } from "@tauri-apps/api/event";
import type { JournalGif } from "./generated/JournalGif";
import type { MessageView } from "./generated/MessageView";
import type { StickerLibrary } from "./generated/StickerLibrary";
import type { StickerPackAdded } from "./generated/StickerPackAdded";
import type { StickerPackProgress } from "./generated/StickerPackProgress";
import type { TelegramTokenState } from "./generated/TelegramTokenState";
import { STICKER_PACK_PROGRESS, call } from "./tauriCall";

// The sticker, GIF and bot token commands (slice 5c), apart from `TauriApi` to keep that file short. Argument names are camelCase.

export function getStickerLibrary(): Promise<StickerLibrary> {
  return call("get_sticker_library");
}

export function addStickerPack(link: string): Promise<StickerPackAdded> {
  return call("add_sticker_pack", { link });
}

export async function cancelStickerPack(): Promise<void> {
  await call("cancel_sticker_pack");
}

export async function removeStickerPack(packId: string): Promise<void> {
  await call("remove_sticker_pack", { packId });
}

export function sendSticker(chatId: string, stickerId: string, replyToId?: string | null): Promise<MessageView> {
  return call("send_sticker", { chatId, stickerId, replyToId: replyToId ?? null });
}

export function listJournalGifs(): Promise<JournalGif[]> {
  return call("list_journal_gifs");
}

export function sendGifAgain(chatId: string, attachmentId: string, replyToId?: string | null): Promise<MessageView> {
  return call("send_gif_again", { chatId, attachmentId, replyToId: replyToId ?? null });
}

/** The progress event (`stickers::STICKER_PACK_PROGRESS`, the main window only). A payload that is not two sane counts is ignored. */
export function onStickerPackProgress(handler: (progress: StickerPackProgress) => void): Promise<() => void> {
  return listen<unknown>(STICKER_PACK_PROGRESS, (event) => {
    const progress = parseProgress(event.payload);
    if (progress) handler(progress);
  });
}

/** Two whole, non-negative counts with `done <= total`; anything else (a string, a negative, NaN, a missing field) is `null`. */
export function parseProgress(payload: unknown): StickerPackProgress | null {
  if (typeof payload !== "object" || payload === null) return null;
  const { done, total } = payload as Record<string, unknown>;
  if (!isCount(done) || !isCount(total) || done > total) return null;
  return { done, total };
}

function isCount(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

export function getTelegramTokenState(): Promise<TelegramTokenState> {
  return call("get_telegram_token_state");
}

export function setTelegramBotToken(token: string): Promise<TelegramTokenState> {
  return call("set_telegram_bot_token", { token });
}

export function clearTelegramBotToken(): Promise<TelegramTokenState> {
  return call("clear_telegram_bot_token");
}
