import { useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { canShowFull, useMediaUrl } from "../attachments/mediaUrl";
import "./sticker-image.css";

/** The shape of a sticker's box: width over height, one when the record has no size (a picture of an unknown size gets the square). */
function shape({ width, height }: AttachmentView): CSSProperties {
  const known = width !== null && height !== null && width > 0 && height > 0;
  return { "--sticker-aspect": known ? `${width / height}` : "1" } as CSSProperties;
}

/**
 * A sticker in the feed (slice 5c), with no bubble around it. The box is fitted from the picture's own width and height to the token
 * `layout.sticker.long-side` (the long side) by CSS before the file loads, so nothing moves when it arrives. A click does nothing: there is no viewer for
 * a sticker. The picture is the entry's own copy of the file (`full` when it is served, which it is for a sticker: a small WebP or PNG), so its
 * transparency is kept. A file that is missing or cannot be decoded leaves the box with the sticker's emoji in it.
 */
export function StickerImage({ attachment }: { attachment: AttachmentView }) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const [failed, setFailed] = useState(false);
  const label = attachment.stickerEmoji ? t("stickers.sticker", { emoji: attachment.stickerEmoji }) : t("stickers.stickerNoEmoji");
  const broken = failed || attachment.state === "missing";
  return (
    <div className="sticker-image" style={shape(attachment)} data-attachment-id={attachment.id}>
      {broken ? (
        <span className="sticker-image__fallback" role="img" aria-label={label}>
          {attachment.stickerEmoji}
        </span>
      ) : (
        <img
          className="sticker-image__picture"
          src={mediaUrl(canShowFull(attachment) ? "full" : "thumb", attachment.id)}
          alt={label}
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(true)}
        />
      )}
    </div>
  );
}
