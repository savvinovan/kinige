import { useRef, useState, type CSSProperties, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { albumRows } from "./albumRows";
import { useMediaUrl } from "./mediaUrl";
import { photoLabel } from "./photoLabel";
import { PreviewTile } from "./PreviewTile";
import { useReveal } from "./reveal";
import { HideButton, SpoilerFace } from "./SpoilerFace";
import "./album.css";

interface AlbumProps {
  /** The photos of one entry, in order: 1 to 10, all of kind `image`. */
  images: readonly AttachmentView[];
  /** A click or Enter on a photo: its index in `images`. */
  onOpen: (index: number) => void;
  /** A right click on a photo (the attachment menu): its index in `images` and the pointer's place. */
  onContextMenu?: (index: number, event: MouseEvent<HTMLElement>) => void;
}

/**
 * The photos of an entry in the bubble (slice 7). One photo goes up to the bubble width and 400 px high, is never cropped and never
 * stretched beyond its own size; its box is sized from `width` and `height` before the file loads (CSS aspect-ratio), so the thumbnail
 * arriving does not move the feed. Two to ten photos form a grid of rows (`albumRows`): every cell is a fixed-ratio box from the
 * tokens and the photo fills it with a crop. A photo that cannot be decoded (`onError`) or whose file is missing keeps its box and
 * shows a text tile. No zoom and no fade: nothing moves.
 *
 * A hidden photo (KINIG-136, `spoiler`) is a cover in the same box until its click reveals it: no `img` and no request before that. In a row of
 * three the cover shows the eye only.
 */
export function Album({ images, onOpen, onContextMenu }: AlbumProps) {
  const [first] = images;
  if (first === undefined) return null;
  if (images.length === 1) {
    return (
      <div className="album album--single" style={singleBox(first)}>
        <AlbumCell image={first} index={0} onOpen={onOpen} onContextMenu={onContextMenu} />
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
              <AlbumCell key={image.id} image={image} index={start + offset} onOpen={onOpen} onContextMenu={onContextMenu} compact={count >= 3} />
            ))}
          </div>
        );
      })}
    </div>
  );
}

interface CellProps {
  image: AttachmentView;
  index: number;
  onOpen: (index: number) => void;
  onContextMenu?: (index: number, event: MouseEvent<HTMLElement>) => void;
  /** A cell of a row of three: the cover shows the eye without the word. */
  compact?: boolean;
}

function AlbumCell({ image, index, onOpen, onContextMenu, compact = false }: CellProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const { isRevealed, reveal, hide } = useReveal();
  const [failed, setFailed] = useState(false);
  const open = useRef<HTMLButtonElement>(null);
  const covered = image.spoiler && !isRevealed(image.id);
  const missing = image.state === "missing";
  const label = photoLabel(image, t);
  const menu = onContextMenu ? (event: MouseEvent<HTMLElement>) => onContextMenu(index, event) : undefined;

  // A missing file that is not hidden is only a tile. A hidden one keeps its button, so the cover can be pressed and the focus stays put when
  // it turns into the tile (a cover is never "File is missing" with a name).
  if (missing && !image.spoiler) {
    return (
      <div className="album__cell" role="img" aria-label={`${label}. ${t("attachments.missing")}`} onContextMenu={menu}>
        <PreviewTile reason="missing" />
      </div>
    );
  }
  const press = () => {
    if (covered) reveal(image.id);
    else if (!missing) onOpen(index);
  };
  // The focus goes to the picture's button before the Hide button disappears with the cover coming back.
  const cover = () => {
    open.current?.focus();
    hide(image.id);
  };
  return (
    <div className={image.spoiler && !covered ? "album__cell has-hide" : "album__cell"} onContextMenu={menu}>
      <button
        type="button"
        ref={open}
        className="album__open"
        data-attachment-id={image.id}
        aria-label={covered ? t("attachments.hiddenPhoto") : missing ? `${label}. ${t("attachments.missing")}` : label}
        aria-disabled={(!covered && missing) || undefined}
        onClick={press}
      >
        {covered ? (
          <SpoilerFace compact={compact} />
        ) : missing ? (
          <PreviewTile reason="missing" />
        ) : failed ? (
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
      {image.spoiler && !covered ? <HideButton onHide={cover} /> : null}
    </div>
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
