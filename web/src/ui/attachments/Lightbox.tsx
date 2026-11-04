import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Button } from "../controls/Button";
import { IconButton } from "../controls/IconButton";
import { canShowFull, useMediaUrl } from "./mediaUrl";
import { photoLabel } from "./photoLabel";
import { PreviewTile } from "./PreviewTile";
import { useDialogKeys } from "./useDialogKeys";
import { useViewerInput } from "./useViewerInput";
import { useViewerTransform } from "./useViewerTransform";
import type { Size } from "./viewerMath";
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
 * A photo over the whole window (slice 7, KINIG-87). A dialog with a scrim, a focus trap and Escape; it returns focus to what opened it.
 * The photo fits the window (contain) and is drawn from `full` when the protocol serves it (`canShowFull`), otherwise from the
 * thumbnail. Left and Right move between the entry's photos (stopping at the ends), "2 / 4" shows the place when there are several.
 * A photo that fails shows "No preview" in the same area; "Open in default app" stays available.
 * A click on the backdrop closes, a click on the photo closes it at fit size; Cmd or Ctrl + wheel, `+` `-` `0`, a double click and the toolbar
 * zoom, a drag pans, the rotate button and `R` turn it by 90 degrees (view only). The moves are in `useViewerTransform` and `useViewerInput`.
 */
export function Lightbox({ images, index, onIndexChange, onClose, onOpenExternal }: LightboxProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const root = useRef<HTMLDivElement>(null);
  const figure = useRef<HTMLElement>(null);
  // How many sources of a photo failed to load: a failed `full` falls back to the thumbnail, a failed thumbnail gives the tile.
  const [failures, setFailures] = useState<Readonly<Record<string, number>>>({});
  // The size of what was drawn, from the picture itself: a thumbnail is smaller than its photo and is not enlarged.
  const [loaded, setLoaded] = useState<Readonly<Record<string, Size>>>({});
  const last = images.length - 1;
  const current = images[Math.min(Math.max(index, 0), last)];
  const position = current ? images.indexOf(current) : 0;

  const failed = current ? (failures[current.id] ?? 0) : 0;
  const full = current ? canShowFull(current) : false;
  const variant = full && failed === 0 ? "full" : "thumb";
  const showTile = !current || current.state === "missing" || failed >= (full ? 2 : 1);
  const source = current ? `${current.id}-${variant}` : "";
  const natural = loaded[source] ?? (current && variant === "full" ? declaredSize(current) : null);

  const viewer = useViewerTransform({ resetKey: current?.id ?? "", natural, figure, enabled: !showTile });
  const input = useViewerInput(root, figure, viewer, current?.id ?? "", onClose);

  const step = (direction: -1 | 1) => {
    const target = position + direction;
    if (target >= 0 && target <= last) onIndexChange(target);
  };
  useDialogKeys(root, { onClose, onStep: step });

  if (!current) return null;
  const many = images.length > 1;
  const tip = (label: string, key: string) => t("attachments.withKey", { label, key });
  const classes = ["lightbox"];
  if (viewer.zoomed) classes.push("lightbox--zoomed");
  if (input.dragging) classes.push("lightbox--dragging");

  return (
    <div
      className={classes.join(" ")}
      role="dialog"
      aria-modal="true"
      aria-label={t("attachments.lightboxLabel")}
      tabIndex={-1}
      ref={root}
      {...input.handlers}
    >
      <div className="lightbox__scrim" aria-hidden="true" />
      <header className="lightbox__bar">
        {many ? (
          <span className="lightbox__counter" aria-live="polite" data-viewer-control>
            {t("attachments.counter", { index: position + 1, total: images.length })}
          </span>
        ) : null}
        <Button variant="secondary" className="lightbox__external" onClick={() => onOpenExternal(current.id)}>
          {t("attachments.openDefault")}
        </Button>
        {showTile ? null : (
          <div className="lightbox__tools" data-viewer-control>
            <IconButton icon="rotate" label={t("attachments.rotate")} title={tip(t("attachments.rotate"), "R")} onClick={viewer.rotate} />
            <IconButton
              icon="zoomOut"
              label={t("attachments.zoomOut")}
              title={tip(t("attachments.zoomOut"), "-")}
              className="lightbox__tool"
              aria-disabled={viewer.atMin || undefined}
              onClick={() => viewer.zoomStep(-1)}
            />
            <Button
              variant="secondary"
              className="lightbox__percent lightbox__tool"
              aria-label={t("attachments.zoomReset", { percent: viewer.percent })}
              title={tip(t("attachments.zoomResetShort"), "0")}
              aria-disabled={viewer.atMin || undefined}
              onClick={viewer.resetZoom}
            >
              {viewer.percent}%
            </Button>
            <IconButton
              icon="zoomIn"
              label={t("attachments.zoomIn")}
              title={tip(t("attachments.zoomIn"), "+")}
              className="lightbox__tool"
              aria-disabled={viewer.atMax || undefined}
              onClick={() => viewer.zoomStep(1)}
            />
          </div>
        )}
        <IconButton icon="close" label={t("attachments.close")} className="lightbox__close" onClick={onClose} />
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
        <figure className="lightbox__figure" ref={figure}>
          {showTile ? (
            <div className="lightbox__tile" data-viewer-image>
              <PreviewTile reason={current.state === "missing" ? "missing" : "noPreview"} />
            </div>
          ) : (
            <img
              key={source}
              className={viewer.animate ? "lightbox__image lightbox__image--eased" : "lightbox__image"}
              src={mediaUrl(variant, current.id)}
              alt={photoLabel(current, t)}
              style={viewer.imageStyle}
              draggable={false}
              data-viewer-image
              onLoad={(event) => {
                const { naturalWidth, naturalHeight } = event.currentTarget;
                if (naturalWidth > 0 && naturalHeight > 0) setLoaded((was) => ({ ...was, [source]: { width: naturalWidth, height: naturalHeight } }));
              }}
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

/** The pixel size the entry declares for the photo (after the EXIF orientation); `null` for a photo without one. */
function declaredSize(photo: AttachmentView): Size | null {
  return photo.width && photo.height ? { width: photo.width, height: photo.height } : null;
}
