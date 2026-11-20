import type { AttachmentView } from "../../api/generated/AttachmentView";

/**
 * The display rule of slice 5c, the same as `kinige_domain::attachment::sticker_of`: an entry is drawn as a sticker (no bubble, a pill for
 * the time) when its body is empty, it has exactly one attachment, that attachment reads as a sticker, and it is not covered (slice 7b).
 * Anything else is an ordinary picture: an entry that was edited after the import has a body, and a covered sticker is drawn as the cover
 * (which can still be uncovered). Returns that attachment, or `null`.
 *
 * The body is compared as it is, not trimmed: Rust's rule is `body_md.is_empty()`.
 */
export function stickerOf(bodyMd: string, attachments: readonly AttachmentView[] | undefined): AttachmentView | null {
  if (attachments?.length !== 1) return null;
  const [only] = attachments;
  return only !== undefined && bodyMd === "" && only.sticker && !only.spoiler ? only : null;
}
