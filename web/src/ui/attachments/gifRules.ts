import type { AttachmentView } from "../../api/generated/AttachmentView";
import { FULL_MAX_BYTES } from "./mediaUrl";

/**
 * GIF playback in the feed and the trash (slice 5c). A GIF is an `image` attachment whose type is `image/gif`; storage, thumbnails and the
 * protocol are unchanged: the cell shows the thumbnail (the first frame) and swaps to `full` while it plays.
 *
 * Autoplay needs the file to be at most this many bytes and this many pixels. Both figures are guesses at a screen of playing GIFs that stays
 * within normal memory; slice 5c measures them (`gif-memory.txt`).
 */
export const GIF_AUTOPLAY_MAX_BYTES = 8 * 1024 * 1024;
export const GIF_AUTOPLAY_MAX_PIXELS = 2_000_000;

/**
 * How a GIF behaves in its cell:
 * - `autoplay`: it plays while at least half of the cell is on screen and the window is visible;
 * - `click`: it is too large to autoplay (or its size is not known): the first frame with the badge, and a click plays it until it leaves the screen;
 * - `viewer`: it is over 64 MiB, so ADR-0014 serves no `full` and it never plays in the app: a click opens the viewer.
 */
export type GifMode = "autoplay" | "click" | "viewer";

export function isGif(attachment: Pick<AttachmentView, "kind" | "mime">): boolean {
  return attachment.kind === "image" && attachment.mime.toLowerCase() === "image/gif";
}

/**
 * The mode of a GIF from its record. A size that is not known (no width or height) cannot be counted in pixels, so it is not autoplayed: it is the
 * safe side of the memory rule.
 */
export function gifMode({ byteSize, width, height }: Pick<AttachmentView, "byteSize" | "width" | "height">): GifMode {
  if (byteSize > FULL_MAX_BYTES) return "viewer";
  const pixels = width !== null && height !== null && width > 0 && height > 0 ? width * height : Number.POSITIVE_INFINITY;
  return byteSize <= GIF_AUTOPLAY_MAX_BYTES && pixels <= GIF_AUTOPLAY_MAX_PIXELS ? "autoplay" : "click";
}
