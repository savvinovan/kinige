import type { TFunction } from "i18next";
import type { AttachmentSummary } from "../../api/generated/AttachmentSummary";

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
