import { INBOX_ID } from "./KinigeApi";
import type { ChatColor } from "./generated/ChatColor";

/**
 * The automatic chat colour, as `kinige-domain/src/chat_color.rs` computes it: the emulation for `MockApi` (tests and stories). The real
 * colour always comes from Rust (`ChatSummary.color`); this copy must not be used by the UI. The order matters: reordering recolours chats.
 */
const AUTOMATIC: readonly ChatColor[] = ["blue", "teal", "green", "orange", "pink", "violet"];

/** Inbox is always grey. */
export const INBOX_COLOR: ChatColor = "grey";

/**
 * 32-bit FNV-1a over the UTF-8 bytes of the id (ADR-0001 details §C.7) modulo the six hues. `fixtures/api-contract/chat-color.json`
 * pins the pairs against Rust, ASCII and non-ASCII ids alike.
 */
export function automaticColor(id: string): ChatColor {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(id)) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return AUTOMATIC[hash % AUTOMATIC.length] ?? "blue";
}

/** The colour shown for a chat: Inbox is grey; an explicit pick wins; `null` shows the automatic colour. */
export function resolveColor(id: string, picked: ChatColor | null): ChatColor {
  if (id === INBOX_ID) return INBOX_COLOR;
  return picked ?? automaticColor(id);
}
