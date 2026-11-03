import { useTranslation } from "react-i18next";

/**
 * The stand-in for a photo that cannot be drawn: "No preview" (the file is there, but it cannot be decoded) or "File is missing".
 * It fills the box of the photo, which is already reserved by the parent, so the feed does not move.
 */
export function PreviewTile({ reason }: { reason: "noPreview" | "missing" }) {
  const { t } = useTranslation();
  return (
    <span className="preview-tile">
      <span className="preview-tile__text">{t(reason === "missing" ? "attachments.missing" : "attachments.noPreview")}</span>
    </span>
  );
}
