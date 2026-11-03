import type { TFunction } from "i18next";
import type { AttachmentSummary } from "../../api/generated/AttachmentSummary";
import type { AttachmentView } from "../../api/generated/AttachmentView";

/**
 * The summary of an entry's attachments, for places that already hold the entry (the card, search, the capture context). The same
 * counting as `kinige_domain::attachment::summary_of`: photos, everything else, and the name when there is exactly one attachment.
 */
export function summaryOf(attachments: readonly AttachmentView[]): AttachmentSummary {
  const photos = attachments.filter((a) => a.kind === "image").length;
  const files = attachments.length - photos;
  const only = attachments.length === 1 ? attachments[0] : undefined;
  return { photos, files, name: only ? only.filename : null };
}

/**
 * The line for places that have no room for attachments (reply quote, "On this day", search, capture context): "Photo",
 * "3 photos", the file name for one file, "2 files", "5 attachments" for a mix. Empty for an entry without attachments.
 */
export function summaryText({ photos, files, name }: AttachmentSummary, t: TFunction): string {
  if (photos === 0 && files === 0) return "";
  if (files === 0) return photos === 1 ? t("attachments.summary.photo") : t("attachments.summary.photos", { count: photos });
  if (photos === 0) {
    if (files === 1) return name !== null && name !== "" ? name : t("attachments.summary.file");
    return t("attachments.summary.files", { count: files });
  }
  return t("attachments.summary.mixed", { count: photos + files });
}
