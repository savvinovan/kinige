import { useEffect, useRef, useState, type MouseEvent, type Ref } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { GifBadge } from "../stickers/GifBadge";
import { gifMode } from "./gifRules";
import { observeVisibility, useReducedMotion, useWindowVisible } from "./gifPresence";
import { useMediaUrl } from "./mediaUrl";
import "./gif-cell.css";

interface GifCellProps {
  /** An uncovered GIF whose file is there (the album cell handles a cover, a missing file and a thumbnail that cannot be decoded). */
  image: AttachmentView;
  /** The screen reader label of the picture (`photoLabel`). */
  label: string;
  /** Open the viewer. */
  onOpen: () => void;
  /** The thumbnail cannot be decoded: the album cell shows its "No preview" tile. */
  onFailed: () => void;
  /** The picture's button, so the album can return focus to it. */
  ref?: Ref<HTMLButtonElement>;
}

/**
 * A GIF in its cell (slice 5c): the thumbnail, which is the first frame, until the GIF plays, then `full`. One shared `IntersectionObserver` says when at
 * least half of the cell is on screen; the GIF plays while it is, the window is visible and `gifMode` allows it. The box is the album's, fixed from the
 * record before any file loads, so the swap moves nothing and the thumbnail goes back when the cell leaves the screen (memory).
 *
 * - `autoplay` (at most 8 MiB and 2,000,000 pixels): plays on its own; a click on a playing GIF opens the viewer.
 * - `click` (larger, up to 64 MiB): the first frame with the "GIF" badge; a click plays it until it leaves the screen, a click on it then opens the viewer.
 * - `viewer` (over 64 MiB, no `full` is served): the badge; a click opens the viewer.
 * - Reduce Motion, followed live: every GIF stands still with the badge, and a click plays that one.
 */
export function GifCell({ image, label, onOpen, onFailed, ref }: GifCellProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const mode = gifMode(image);
  const reduced = useReducedMotion();
  const windowVisible = useWindowVisible();
  const area = useRef<HTMLSpanElement>(null);
  const [onScreen, setOnScreen] = useState(false);
  // "Plays on click until it leaves the screen": the click is remembered until the observer says the cell has left.
  const [clicked, setClicked] = useState(false);
  const [fullFailed, setFullFailed] = useState(false);

  useEffect(() => {
    const element = area.current;
    if (!element) return;
    return observeVisibility(element, (visible) => {
      setOnScreen(visible);
      if (!visible) setClicked(false);
    });
  }, []);

  const mayPlay = mode !== "viewer" && !fullFailed && onScreen && windowVisible;
  const playing = mayPlay && (clicked || (mode === "autoplay" && !reduced));
  // The badge marks a GIF that stands still and has to be asked to play. An autoplaying GIF that is only off screen has none: it will play on its own.
  const badge = !playing && (mode !== "autoplay" || reduced || fullFailed);
  // A click plays a GIF that is waiting for one; a playing one, one over 64 MiB, or one that autoplays and only waits for the screen opens the viewer.
  const waitsForClick = !playing && mode !== "viewer" && !fullFailed && (mode === "click" || reduced);

  const press = (event: MouseEvent) => {
    event.preventDefault();
    if (waitsForClick) setClicked(true);
    else onOpen();
  };

  const hint = waitsForClick ? t("attachments.gifPlay", { label }) : label;
  return (
    <button type="button" ref={ref} className="album__open" data-attachment-id={image.id} data-gif={playing ? "playing" : "still"} aria-label={hint} onClick={press}>
      <span className="gif-cell" ref={area}>
        <img
          className="album__image"
          src={mediaUrl(playing ? "full" : "thumb", image.id)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => (playing ? setFullFailed(true) : onFailed())}
        />
        {badge ? <GifBadge /> : null}
      </span>
    </button>
  );
}
