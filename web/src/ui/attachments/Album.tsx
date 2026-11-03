import { useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { albumRows } from "./albumRows";
import { useMediaUrl } from "./mediaUrl";
import { photoLabel } from "./photoLabel";
import { PreviewTile } from "./PreviewTile";
import "./album.css";

interface AlbumProps {
  /** The photos of one entry, in order: 1 to 10, all of kind `image`. */
  images: readonly AttachmentView[];
  /** A click or Enter on a photo: its index in `images`. */
  onOpen: (index: number) => void;
}

/**
 * The photos of an entry in the bubble (slice 7). One photo goes up to the bubble width and 400 px high, is never cropped and never
 * stretched beyond its own size; its box is sized from `width` and `height` before the file loads (CSS aspect-ratio), so the thumbnail
 * arriving does not move the feed. Two to ten photos form a grid of rows (`albumRows`): every cell is a fixed-ratio box from the
 * tokens and the photo fills it with a crop. A photo that cannot be decoded (`onError`) or whose file is missing keeps its box and
 * shows a text tile. No zoom and no fade: nothing moves.
 */
export function Album({ images, onOpen }: AlbumProps) {
  const [first] = images;
  if (first === undefined) return null;
  if (images.length === 1) {
    return (
      <div className="album album--single" style={singleBox(first)}>
        <AlbumCell image={first} index={0} onOpen={onOpen} />
      </div>
    );
  }
  const rows = albumRows(images.length);
  const starts = rows.map((_, row) => rows.slice(0, row).reduce((sum, count) => sum + count, 0));
  return (
    <div className="album album--grid">
      {rows.map((count, row) => {
        const start = starts[row] ?? 0;
        return (
          <div className={`album__row album__row--${Math.min(count, 3)}`} key={start}>
            {images.slice(start, start + count).map((image, offset) => (
              <AlbumCell key={image.id} image={image} index={start + offset} onOpen={onOpen} />
            ))}
          </div>
        );
      })}
    </div>
  );
}

function AlbumCell({ image, index, onOpen }: { image: AttachmentView; index: number; onOpen: (index: number) => void }) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const [failed, setFailed] = useState(false);
  const label = photoLabel(image, t);

  if (image.state === "missing") {
    return (
      <div className="album__cell" role="img" aria-label={`${label}. ${t("attachments.missing")}`}>
        <PreviewTile reason="missing" />
      </div>
    );
  }
  return (
    <button type="button" className="album__cell album__cell--button" aria-label={label} onClick={() => onOpen(index)}>
      {failed ? (
        <PreviewTile reason="noPreview" />
      ) : (
        <img
          className="album__image"
          src={mediaUrl("thumb", image.id)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(true)}
        />
      )}
    </button>
  );
}

/** The box of a single photo: the shape and the natural size are CSS variables read in album.css; a photo of unknown size gets the default shape. */
function singleBox({ width, height }: AttachmentView): CSSProperties {
  if (width === null || height === null || width <= 0 || height <= 0) return {};
  return {
    "--album-aspect": `${width / height}`,
    "--album-natural": `calc(${width} * var(--layout-attachment-pixel))`,
  } as CSSProperties;
}
