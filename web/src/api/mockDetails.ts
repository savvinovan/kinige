import { KinigeApiError } from "./KinigeApi";
import type { AttachmentDetailsChanges } from "./generated/AttachmentDetailsChanges";
import type { AttachmentView } from "./generated/AttachmentView";
import { rustTrim } from "./mockText";

// The rules of attachment details that `MockApi` repeats from Rust (`kinige_domain::attachment::{normalize_name, normalize_description}`,
// slice 7b). `fixtures/api-contract/attachment-text.json` pins them against the real functions. The UI counter is advisory: these
// functions are what decides.

/** The longest description, in Unicode scalar values after normalization. */
export const DESCRIPTION_MAX_CHARS = 1024;
const DESCRIPTION_MAX_BYTES = 4096;
const NAME_MAX_BYTES = 255;

/** A control character (C0, DEL, C1), a path separator, a bidi override or isolate, or a lone surrogate: never in a name. */
const NOT_IN_A_NAME = /[\p{Cc}\p{Cs}/\\\u{202a}-\u{202e}\u{2066}-\u{2069}]/u;
/** Everything in a description except the line feed is refused if it is a control character; a lone surrogate cannot reach Rust. */
const NOT_IN_A_DESCRIPTION = /[^\P{Cc}\n]|\p{Cs}/u;

const invalid = () => new KinigeApiError("attachment_details_invalid");
const byteLength = (text: string) => new TextEncoder().encode(text).length;

/** The extension without its dot: what follows the last dot when something stands before it; empty if none. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1) : "";
}

/** The extension with its dot (`.jpg`), or empty: the fixed suffix of a display name. */
export function extSuffix(name: string): string {
  const ext = extensionOf(name);
  return ext === "" ? "" : name.slice(name.length - ext.length - 1);
}

/** ASCII lower case only, as Rust's `eq_ignore_ascii_case`: never Unicode lowercasing. */
const asciiFold = (text: string) => text.replace(/[A-Z]/g, (letter) => letter.toLowerCase());

/** The whole display name Rust makes of what was typed (the stem), or `null` for the original: empty, or equal to the original. */
export function normalizeName(stem: string, original: string): string | null {
  const trimmed = rustTrim(stem);
  if (trimmed === "") return null;
  if (NOT_IN_A_NAME.test(trimmed)) throw invalid();
  const full = `${trimmed}${extSuffix(original)}`;
  if (full === original) return null;
  if (byteLength(full) > NAME_MAX_BYTES) throw invalid();
  if (!validDisplayName(full, original)) throw invalid();
  return full;
}

/** A name that may be shown: 1 to 255 bytes, nothing to trim, no path or control character, the extension of the original. */
export function validDisplayName(name: string, original: string): boolean {
  return (
    name !== "" &&
    byteLength(name) <= NAME_MAX_BYTES &&
    !NOT_IN_A_NAME.test(name) &&
    rustTrim(name) === name &&
    name !== "." &&
    name !== ".." &&
    asciiFold(extensionOf(name)) === asciiFold(extensionOf(original))
  );
}

/**
 * What a details command does to an attachment, as Rust does it: each field that was sent is normalized, and a field that was not sent
 * stays. A spoiler belongs to an image: any value on another kind is refused, and so is `null`. Nothing is changed on an error.
 */
export function applyDetails(view: AttachmentView, changes: AttachmentDetailsChanges): AttachmentView {
  const sent = changes as Record<string, unknown>;
  if (Object.keys(sent).some((key) => key !== "name" && key !== "description" && key !== "spoiler")) throw invalid();
  const next = { ...view };
  if ("name" in sent && sent.name !== undefined) {
    if (sent.name !== null && typeof sent.name !== "string") throw invalid();
    next.name = (sent.name === null ? null : normalizeName(sent.name, view.filename)) ?? view.filename;
  }
  if ("description" in sent && sent.description !== undefined) {
    if (sent.description !== null && typeof sent.description !== "string") throw invalid();
    next.description = sent.description === null ? null : normalizeDescription(sent.description);
  }
  if ("spoiler" in sent && sent.spoiler !== undefined) {
    if (typeof sent.spoiler !== "boolean" || view.kind !== "image") throw invalid();
    next.spoiler = sent.spoiler;
  }
  return next;
}

/** A description as Rust stores it: CRLF and CR become LF, a TAB one space, the ends are trimmed; empty is `null`. */
export function normalizeDescription(raw: string): string | null {
  const text = rustTrim(raw.replaceAll("\r\n", "\n").replaceAll("\r", "\n").replaceAll("\t", " "));
  if (NOT_IN_A_DESCRIPTION.test(text)) throw invalid();
  if (text === "") return null;
  if ([...text].length > DESCRIPTION_MAX_CHARS || byteLength(text) > DESCRIPTION_MAX_BYTES) throw invalid();
  return text;
}
