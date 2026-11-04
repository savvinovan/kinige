import type { TFunction } from "i18next";
import type { AttachmentKind } from "../../api/generated/AttachmentKind";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { IconName } from "../icons/Icon";
import { fileExtension } from "./fileName";
import { formatSize } from "./formatSize";

/** The icon of a file card by the kind of the attachment: audio and video are plain files in this slice, only the picture differs. */
export function fileIcon(kind: AttachmentKind): IconName {
  return kind === "audio" ? "audio" : kind === "video" ? "video" : "file";
}

/** "PDF · 2.4 MB" (the extension in capitals); only the size when the name has no extension. */
export function fileMetaText(attachment: AttachmentView, language: string, t: TFunction): string {
  const size = formatSize(attachment.byteSize, language);
  const extension = fileExtension(attachment.filename);
  return extension === "" ? size : t("attachments.fileMeta", { type: extension.toUpperCase(), size });
}
