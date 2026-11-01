import type { ChatTint, SidebarChat } from "./chats";

/** Цвета плитки с буквой. Порядок значим: цвет чата выбирается по хешу идентификатора, перестановка перекрасит чаты. */
export const TINTS: readonly ChatTint[] = ["blue", "teal", "green", "orange", "pink", "violet"];

/** Эмодзи по умолчанию (`DEFAULT_ICON` в `kinige-domain/src/chat.rs`): Rust хранит его, когда эмодзи не выбран. */
const DEFAULT_EMOJI = "💬";

/** Чат с настоящим эмодзи: пустой значок и значок по умолчанию «💬» считаются «без эмодзи» (только для показа, в данных ничего не меняется). */
export function hasEmoji(emoji: string | null | undefined): boolean {
  const value = (emoji ?? "").trim();
  return value !== "" && value !== DEFAULT_EMOJI;
}

const LETTER = /[\p{L}\p{N}]/u;
const FALLBACK_LETTER = "#";

const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/**
 * Первая буква названия для плитки: первая графема-буква или цифра в верхнем регистре. Пробелы, знаки препинания и эмодзи
 * в начале пропускаются («  «Ёлка»», «🎉 Party» дают «Ё» и «P»); графема целиком, поэтому «Е» с комбинируемым диакритическим
 * знаком не рвётся. Если букв нет вовсе, возвращается «#».
 */
export function firstLetter(title: string): string {
  for (const { segment } of segmenter.segment(title)) {
    if (!LETTER.test(segment)) continue;
    const upper = segment.toLocaleUpperCase();
    // «ß» → «SS»: плитка вмещает одну графему.
    return segmenter.segment(upper)[Symbol.iterator]().next().value?.segment ?? upper;
  }
  return FALLBACK_LETTER;
}

/**
 * Цвет плитки по идентификатору чата: 32-битный FNV-1a и остаток от деления на число цветов. Один и тот же чат всегда получает
 * один и тот же цвет; хранимый цвет чата (KINIG-59) заменит эту функцию одним значением `chat.tint` (см. `tintOf`).
 */
export function tintForId(id: string): ChatTint {
  let hash = 0x811c9dc5;
  for (let index = 0; index < id.length; index++) {
    hash ^= id.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return TINTS[hash % TINTS.length] ?? "blue";
}

/** Цвет плитки чата: заданный (`tint`) или выведенный из идентификатора. Единственное место, которое надо заменить в KINIG-59. */
export function tintOf(chat: Pick<SidebarChat, "id" | "tint">): ChatTint {
  return chat.tint ?? tintForId(chat.id);
}
