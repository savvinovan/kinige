import type { ChatTint, SidebarChat } from "./chats";

/** Colors of the letter tile. The order matters: a chat's color is chosen by the hash of its id, so reordering recolors chats. */
export const TINTS: readonly ChatTint[] = ["blue", "teal", "green", "orange", "pink", "violet"];

/** The default emoji (`DEFAULT_ICON` in `kinige-domain/src/chat.rs`): Rust stores it when no emoji is chosen. */
const DEFAULT_EMOJI = "💬";

/** A chat with a real emoji: an empty icon and the default "💬" icon count as "no emoji" (display only, the data does not change). */
export function hasEmoji(emoji: string | null | undefined): boolean {
  const value = (emoji ?? "").trim();
  return value !== "" && value !== DEFAULT_EMOJI;
}

const LETTER = /[\p{L}\p{N}]/u;
const FALLBACK_LETTER = "#";

const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/**
 * The first letter of the title for the tile: the first grapheme that is a letter or digit, in upper case. Spaces,
 * punctuation and emoji at the start are skipped ("  «Ёлка»" and "🎉 Party" give "Ё" and "P"); the grapheme is taken
 * whole, so "Е" with a combining diacritical mark is not split. If there are no letters at all, "#" is returned.
 */
export function firstLetter(title: string): string {
  for (const { segment } of segmenter.segment(title)) {
    if (!LETTER.test(segment)) continue;
    const upper = segment.toLocaleUpperCase();
    // "ß" -> "SS": the tile fits one grapheme.
    return segmenter.segment(upper)[Symbol.iterator]().next().value?.segment ?? upper;
  }
  return FALLBACK_LETTER;
}

/**
 * The tile color by chat id: 32-bit FNV-1a and the remainder of division by the number of colors. The same chat always
 * gets the same color; the stored chat color (KINIG-59) will replace this function with a single `chat.tint` value (see `tintOf`).
 */
export function tintForId(id: string): ChatTint {
  let hash = 0x811c9dc5;
  for (let index = 0; index < id.length; index++) {
    hash ^= id.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return TINTS[hash % TINTS.length] ?? "blue";
}

/** The chat tile color: the given one (`tint`) or derived from the id. The only place to replace in KINIG-59. */
export function tintOf(chat: Pick<SidebarChat, "id" | "tint">): ChatTint {
  return chat.tint ?? tintForId(chat.id);
}
