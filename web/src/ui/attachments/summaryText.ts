import type { TFunction } from "i18next";
import type { AttachmentSummary } from "../../api/generated/AttachmentSummary";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { formatDuration } from "../../voice/format";

/**
 * The summary of an entry's attachments, for places that already hold the entry (the card, search, the capture context). The same
 * counting as `kinige_domain::attachment::summary_of`: photos, voice notes (every `audio` attachment, slice 7a), everything else, the
 * total length of the voice notes that have one, and the name when there is exactly one attachment.
 */
export function summaryOf(attachments: readonly AttachmentView[]): AttachmentSummary {
  const photos = attachments.filter((a) => a.kind === "image").length;
  const voiceNotes = attachments.filter((a) => a.kind === "audio");
  const lengths = voiceNotes.flatMap((a) => (a.durationMs === null ? [] : [a.durationMs]));
  const files = attachments.length - photos - voiceNotes.length;
  const only = attachments.length === 1 ? attachments[0] : undefined;
  return {
    photos,
    files,
    voices: voiceNotes.length,
    voiceMs: lengths.length > 0 ? lengths.reduce((sum, ms) => sum + ms, 0) : null,
    name: only ? only.filename : null,
  };
}

/**
 * The line for places that have no room for attachments (reply quote, "On this day", search, capture context): "Photo",
 * "3 photos", the file name for one file, "2 files", "Voice note · 0:40", "3 voice notes", "5 attachments" for a mix. Empty for an entry
 * without attachments.
 */
export function summaryText({ photos, files, voices, voiceMs, name }: AttachmentSummary, t: TFunction): string {
  if (photos === 0 && files === 0 && voices === 0) return "";
  if (files === 0 && voices === 0) return photos === 1 ? t("attachments.summary.photo") : t("attachments.summary.photos", { count: photos });
  if (photos === 0 && files === 0) {
    if (voices > 1) return t("attachments.summary.voices", { count: voices });
    return voiceMs === null ? t("attachments.summary.voiceNoTime") : t("attachments.summary.voice", { time: formatDuration(voiceMs) });
  }
  if (photos === 0 && voices === 0) {
    if (files === 1) return name !== null && name !== "" ? name : t("attachments.summary.file");
    return t("attachments.summary.files", { count: files });
  }
  return t("attachments.summary.mixed", { count: photos + files + voices });
}
