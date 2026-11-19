import type { TFunction } from "i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";

/**
 * The screen reader label of a revealed photo: its description if it has one, otherwise "Photo beach.jpg, 1280 by 960" (without the size if it
 * is not known). A hidden photo does not use this: it is "Hidden photo, press to show", and its description is neither read nor shown (KINIG-136).
 */
export function photoLabel(attachment: AttachmentView, t: TFunction): string {
  const { name, description, width, height } = attachment;
  if (description) return description;
  return width !== null && height !== null && width > 0 && height > 0
    ? t("attachments.photoLabel", { name, width, height })
    : t("attachments.photoLabelNoSize", { name });
}
