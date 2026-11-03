import { createContext, useContext } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";

/** `full` is served only up to this size (ADR-0014: the responder takes the whole body in memory); a larger file shows its thumbnail. */
export const FULL_MAX_BYTES = 64 * 1024 * 1024;

const FULL_MIMES: ReadonlySet<string> = new Set(["image/jpeg", "image/png", "image/gif", "image/webp"]);

export type MediaVariant = "thumb" | "full";

/** Where a picture of an attachment comes from: the real protocol in the app, synthetic images in stories and tests. */
export type MediaSource = (variant: MediaVariant, id: string) => string;

/**
 * The URL of the `kinige-media` protocol (ADR-0014): `kinige-media://localhost/<variant>/<id>`, and on Windows, where WebView2 maps a custom scheme to
 * a host name, `http://kinige-media.localhost/<variant>/<id>`.
 */
export const mediaUrl: MediaSource = (variant, id) =>
  typeof navigator !== "undefined" && navigator.userAgent.includes("Windows")
    ? `http://kinige-media.localhost/${variant}/${id}`
    : `kinige-media://localhost/${variant}/${id}`;

/** Stories, vitest and MockApi put synthetic images here; the default is the real protocol. */
export const MediaSourceContext = createContext<MediaSource>(mediaUrl);

export function useMediaUrl(): MediaSource {
  return useContext(MediaSourceContext);
}

/** The `full` variant exists for a live image of a known format up to `FULL_MAX_BYTES`; otherwise the lightbox shows the thumbnail. */
export function canShowFull(attachment: AttachmentView): boolean {
  return (
    attachment.kind === "image" &&
    attachment.state === "local" &&
    FULL_MIMES.has(attachment.mime.toLowerCase()) &&
    attachment.byteSize <= FULL_MAX_BYTES
  );
}
