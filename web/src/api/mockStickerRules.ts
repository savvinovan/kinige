// The sticker rules of Rust that `MockApi` repeats (`kinige_domain::stickers`, slice 5c): the pack link, the bot token and the ids of the library.
// They are ours, not Telegram's (its own formats are not verified).

const PACK_NAME = /^[A-Za-z0-9_]{1,64}$/;
const TOKEN = /^[0-9]{1,20}:[A-Za-z0-9_-]{1,100}$/;
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

/** The recent list holds at most this many stickers (`app_setting.recent_stickers`). */
export const MAX_RECENT_STICKERS = 20;

/** The pack name of `https://t.me/addstickers/<name>`, `t.me/addstickers/<name>` or a bare name; anything else is `null`. */
export function packName(input: string): string | null {
  const text = input.trim();
  const name = ["https://t.me/addstickers/", "t.me/addstickers/"].find((prefix) => text.startsWith(prefix));
  const bare = name === undefined ? text : text.slice(name.length);
  return PACK_NAME.test(bare) ? bare : null;
}

/** `^[0-9]{1,20}:[A-Za-z0-9_-]{1,100}$`. */
export function isBotToken(token: string): boolean {
  return TOKEN.test(token);
}

/** A library id is a version 4 UUID; anything else is not a sticker. */
export function isLibraryId(id: string): boolean {
  return UUID_V4.test(id);
}
