import type { TFunction } from "i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";

/** The screen reader label of a photo: "Photo IMG_0001.jpg, 1280 by 960" (without the size if it is not known). */
export function photoLabel(attachment: AttachmentView, t: TFunction): string {
  const { filename, width, height } = attachment;
  return width !== null && height !== null && width > 0 && height > 0
    ? t("attachments.photoLabel", { name: filename, width, height })
    : t("attachments.photoLabelNoSize", { name: filename });
}
