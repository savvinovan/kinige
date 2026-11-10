import { INBOX_ID, KinigeApiError } from "./KinigeApi";
import { WHITE_SPACE_CHAR, plainSnippet, rustTrim } from "./mockText";
import type { AttachmentSummary } from "./generated/AttachmentSummary";
import type { ReplyTo } from "./generated/ReplyTo";

// The rules of Rust that `MockApi` repeats. Each one is pinned against the real function by a file in `fixtures/api-contract/`, read
// by `apiContract.test.ts` here and by `crates/kinige-store/tests/api_contract/` there. Where a copy is rough on purpose, the comment says so.

export const DAY_MS = 24 * 3_600_000;

/** Sizes and limits (`fixtures/api-contract/constants.json`). */
export const LIMITS = {
  feedDefaultPage: 200,
  feedMaxPage: 500,
  agentLogPage: 30,
  recentEmoji: 24,
  trashDays: 30,
  appendWindowMs: 10 * 60 * 1000,
  chatTitleChars: 128,
  chatIconBytes: 40,
  replyExcerptChars: 120,
  onThisDayDetailedYears: 3,
  onThisDayEntriesPerYear: 2,
  onThisDayPreviewChars: 300,
  forestExcerptChars: 120,
  tagChars: 64,
  attachmentsPerEntry: 10,
  pasteMaxBytes: 64 * 1024 * 1024,
  voiceMinMs: 1000,
  voiceMaxMs: 1_805_000,
  waveformPeaks: 64,
} as const;

export const DEFAULT_ICON = "\u{1F4AC}";

/** U+FE0F, written as a code point so that it is visible in the source. */
const VARIATION_SELECTOR_16 = String.fromCodePoint(0xfe0f);
/** Control characters and the bidi overrides and isolates (U+202A..U+202E, U+2066..U+2069): Rust drops them from a file name. */
const HIDDEN_IN_NAMES = new RegExp("[\\p{Cc}\\u202a-\\u202e\\u2066-\\u2069]", "gu");

/** An entry with no attachments, as the reply quote and the day card say it (slice 7). */
export const NO_ATTACHMENTS = { photos: 0, files: 0, voices: 0, voiceMs: null, name: null } as const;

const VIDEO_EXTS = ["mp4", "m4v", "mov", "webm", "mkv", "avi", "mpg", "mpeg", "3gp", "wmv", "flv"];
const AUDIO_EXTS = ["mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "aiff", "aif", "wma", "amr"];
/** Rust decides `image` by the header of the file; the mock has only the name. */
const IMAGE_EXTS = ["jpg", "jpeg", "png", "gif", "webp"];

/**
 * The entry day with the 04:00 boundary. Simplified: the mock knows only UTC, Rust uses the time zone of the entry. In UTC the two
 * agree (`journal-day-utc.json`).
 */
export function mockJournalDate(now: Date): string {
  return new Date(now.getTime() - 4 * 3_600_000).toISOString().slice(0, 10);
}

/** `YYYY-MM-DD` - an existing date with a year from 1000 (as in Rust: future dates are allowed). */
export function isRealDate(text: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const [year, month, day] = text.split("-").map(Number);
  if ((year ?? 0) < 1000) return false;
  const date = new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
  return date.getUTCFullYear() === year && date.getUTCMonth() === (month ?? 1) - 1 && date.getUTCDate() === day;
}

/** Full years between the old entry's day and the reply day; 0 if under a year or the reply is earlier. */
export function yearsLater(old: string, reply: string): number {
  if (!isRealDate(old) || !isRealDate(reply)) return 0;
  const [oy, om, od] = old.split("-").map(Number) as [number, number, number];
  const [ry, rm, rd] = reply.split("-").map(Number) as [number, number, number];
  const years = ry - oy - (rm * 32 + rd < om * 32 + od ? 1 : 0);
  return Math.max(0, years);
}

/** The entry an old entry quote is built from; `undefined` if the entry no longer exists. */
export interface QuoteTarget {
  chatId: string;
  journalDate: string;
  bodyMd: string;
  inTrash: boolean;
  attachments: AttachmentSummary;
}

/** The quote of the entry `id` for a reply written on `replyDate`: live, in the trash, or missing. */
export function replyQuote(id: string, replyDate: string, target: QuoteTarget | undefined): ReplyTo {
  if (!target) {
    return { id, chatId: null, journalDate: null, excerpt: "", state: "missing", yearsLater: 0, attachments: NO_ATTACHMENTS };
  }
  return {
    id,
    chatId: target.chatId,
    journalDate: target.journalDate,
    excerpt: plainSnippet(target.bodyMd, LIMITS.replyExcerptChars),
    state: target.inTrash ? "trash" : "live",
    yearsLater: yearsLater(target.journalDate, replyDate),
    attachments: target.attachments,
  };
}

/** Whole days left before an entry in the trash is deleted for good; a partial day counts as a day. */
export function daysLeft(deletedAt: number, now: number): number {
  return Math.max(0, Math.ceil((deletedAt + LIMITS.trashDays * DAY_MS - now) / DAY_MS));
}

/**
 * The entry that capture may append to: made by the user, not older than ten minutes, on today's day, and not read-only (an entry with
 * a group of a newer version, which Rust refuses to rewrite).
 */
export function canAppendTo(
  entry: { origin: string; createdAt: number; journalDate: string; readOnly: boolean },
  nowMs: number,
): boolean {
  return (
    !entry.readOnly &&
    entry.origin === "user" &&
    entry.journalDate === mockJournalDate(new Date(nowMs)) &&
    nowMs - entry.createdAt <= LIMITS.appendWindowMs
  );
}

/** The text after appending: a new paragraph, but an entry with no text yet gets the addition as is. */
export function joinBody(old: string, addition: string): string {
  return old === "" ? addition : `${old}\n\n${addition}`;
}

/** The size of a feed or search page: the default without a limit, never below 1 or above the maximum. */
export function pageLimit(requested: number | undefined): number {
  if (requested === undefined) return LIMITS.feedDefaultPage;
  return Math.min(LIMITS.feedMaxPage, Math.max(1, Math.trunc(requested)));
}

/** The chat name: trimmed, 1 to 128 characters, no control characters inside. */
export function normalizeTitle(raw: string): string {
  const title = rustTrim(raw);
  if (title === "" || /\p{Cc}/u.test(title)) throw new KinigeApiError("chat_title_empty");
  if ([...title].length > LIMITS.chatTitleChars) throw new KinigeApiError("chat_title_too_long");
  return title;
}

/** The chat emoji: no value or a blank one means the default; otherwise up to 40 bytes, no spaces or control characters. */
export function normalizeIcon(raw: string | null): string {
  const icon = rustTrim(raw ?? "");
  if (icon === "") return DEFAULT_ICON;
  if (new TextEncoder().encode(icon).length > LIMITS.chatIconBytes || /\p{Cc}/u.test(icon) || WHITE_SPACE_CHAR.test(icon)) {
    throw new KinigeApiError("chat_icon_invalid");
  }
  return icon;
}

/**
 * Whether a text may go to the recent emoji list. Rust accepts only a real emoji sequence (a Unicode table of pictographs); the mock
 * refuses what no emoji can be (blank, spaces, control characters) and takes the rest.
 */
export function looksLikeEmoji(text: string): boolean {
  return text !== "" && !WHITE_SPACE_CHAR.test(text) && !/\p{Cc}/u.test(text);
}

/** The emoji without the variation selector: with and without it is one emoji in the recent list. */
export function emojiIdentity(emoji: string): string {
  return emoji.replaceAll(VARIATION_SELECTOR_16, "");
}

/** The chat a click on a forest day opens; `counts` is [chat id, live entries that day]. */
export function dayChat(counts: readonly (readonly [string, number])[], current: string | null): string | null {
  const live = counts.filter(([, count]) => count > 0);
  if (current !== null && live.some(([id]) => id === current)) return current;
  const order = ([aId, a]: readonly [string, number], [bId, b]: readonly [string, number]) =>
    b - a || Number(bId === INBOX_ID) - Number(aId === INBOX_ID) || (aId < bId ? -1 : aId > bId ? 1 : 0);
  return [...live].sort(order)[0]?.[0] ?? null;
}

/**
 * The extension of the stored file: the lower-case one of the last name component if it is 1 to 10 letters or digits after a
 * non-empty stem, otherwise `bin`. Rust also cuts a name to 255 bytes first; the mock does not.
 */
export function storedExt(name: string): string {
  const last = name.split(/[\\/]/).reverse().find((part) => rustTrim(part) !== "") ?? "";
  const clean = rustTrim(last.replace(HIDDEN_IN_NAMES, ""));
  const dot = clean.lastIndexOf(".");
  const ext = clean.slice(dot + 1).replace(/[A-Z]/g, (letter) => letter.toLowerCase());
  return dot > 0 && /^[a-z0-9]{1,10}$/.test(ext) ? ext : "bin";
}

/** The kind of an attachment by its name. Rust looks at the header of an image; the mock trusts the extension. */
export function attachmentKind(name: string): "image" | "video" | "audio" | "file" {
  const ext = storedExt(name);
  if (IMAGE_EXTS.includes(ext)) return "image";
  if (VIDEO_EXTS.includes(ext)) return "video";
  if (AUDIO_EXTS.includes(ext)) return "audio";
  return "file";
}
