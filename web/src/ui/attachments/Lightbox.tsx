import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Button } from "../controls/Button";
import { IconButton } from "../controls/IconButton";
import { canShowFull, useMediaUrl } from "./mediaUrl";
import { photoLabel } from "./photoLabel";
import { PreviewTile } from "./PreviewTile";
import { useDialogKeys } from "./useDialogKeys";
import "./lightbox.css";

interface LightboxProps {
  /** The photos of the entry being viewed, in order. */
  images: readonly AttachmentView[];
  /** The photo on screen, an index in `images`. */
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
  /** "Open in default app": Rust opens the file in the system viewer (Preview). */
  onOpenExternal: (attachmentId: string) => void;
}

/**
 * A photo over the whole window (slice 7). A dialog with a scrim, a focus trap and Escape; it returns focus to what opened it.
 * The photo fits the window (contain) and is drawn from `full` when the protocol serves it (`canShowFull`), otherwise from the
 * thumbnail. Left and Right move between the entry's photos (stopping at the ends), "2 / 4" shows the place when there are several.
 * A photo that fails shows "No preview" in the same area; "Open in default app" stays available. No zoom animation: nothing moves.
 */
export function Lightbox({ images, index, onIndexChange, onClose, onOpenExternal }: LightboxProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const root = useRef<HTMLDivElement>(null);
  // How many sources of a photo failed to load: a failed `full` falls back to the thumbnail, a failed thumbnail gives the tile.
  const [failures, setFailures] = useState<Readonly<Record<string, number>>>({});
  const last = images.length - 1;
  const current = images[Math.min(Math.max(index, 0), last)];
  const position = current ? images.indexOf(current) : 0;

  const step = (direction: -1 | 1) => {
    const target = position + direction;
    if (target >= 0 && target <= last) onIndexChange(target);
  };
  useDialogKeys(root, { onClose, onStep: step });

  if (!current) return null;
  const failed = failures[current.id] ?? 0;
  const full = canShowFull(current);
  const variant = full && failed === 0 ? "full" : "thumb";
  const many = images.length > 1;
  const showTile = current.state === "missing" || failed >= (full ? 2 : 1);

  return (
    <div className="lightbox" role="dialog" aria-modal="true" aria-label={t("attachments.lightboxLabel")} tabIndex={-1} ref={root}>
      <div className="lightbox__scrim" aria-hidden="true" onClick={onClose} />
      <header className="lightbox__bar">
        {many ? (
          <span className="lightbox__counter" aria-live="polite">
            {t("attachments.counter", { index: position + 1, total: images.length })}
          </span>
        ) : null}
        <Button variant="secondary" className="lightbox__external" onClick={() => onOpenExternal(current.id)}>
          {t("attachments.openDefault")}
        </Button>
        <IconButton icon="close" label={t("attachments.close")} onClick={onClose} />
      </header>
      <div className="lightbox__stage">
        {many ? (
          <IconButton
            icon="chevronLeft"
            label={t("attachments.previous")}
            className="lightbox__step"
            aria-disabled={position === 0 || undefined}
            onClick={() => step(-1)}
          />
        ) : null}
        <figure className="lightbox__figure">
          {showTile ? (
            <div className="lightbox__tile">
              <PreviewTile reason={current.state === "missing" ? "missing" : "noPreview"} />
            </div>
          ) : (
            <img
              key={`${current.id}-${variant}`}
              className="lightbox__image"
              src={mediaUrl(variant, current.id)}
              alt={photoLabel(current, t)}
              draggable={false}
              onError={() => setFailures((was) => ({ ...was, [current.id]: (was[current.id] ?? 0) + 1 }))}
            />
          )}
        </figure>
        {many ? (
          <IconButton
            icon="chevronRight"
            label={t("attachments.next")}
            className="lightbox__step"
            aria-disabled={position === last || undefined}
            onClick={() => step(1)}
          />
        ) : null}
      </div>
    </div>
  );
}
