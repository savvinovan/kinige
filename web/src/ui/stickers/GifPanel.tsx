import { useTranslation } from "react-i18next";
import type { JournalGif } from "../../api/generated/JournalGif";
import { useMediaUrl } from "../attachments/mediaUrl";
import { Button } from "../controls/Button";
import { GIF_COLUMNS } from "./gridNav";
import { GifBadge } from "./GifBadge";
import { useRovingGrid } from "./useRovingGrid";
import "./stickers.css";

interface GifPanelProps {
  /** `null` while the list is being read; `"failed"` when it could not be. */
  gifs: readonly JournalGif[] | "failed" | null;
  /** A click or Enter on a GIF: a copy of it is sent as a new entry. */
  onSend: (gif: JournalGif) => void;
  /** "Choose a GIF file…": the same flow as the paperclip. */
  onChoose: () => void;
}

/**
 * The GIFs tab of the emoji window (slice 5c): the GIFs already in the journal as still frames in three columns, newest first, each with the "GIF"
 * badge (the first frame is the thumbnail; nothing plays here), and "Choose a GIF file…" on top. There is no catalogue to search. A click sends that
 * GIF again as a new entry. Arrows, Home, End and Enter work as in the sticker grid.
 */
export function GifPanel({ gifs, onSend, onChoose }: GifPanelProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const loaded = gifs !== null && gifs !== "failed";
  const list: readonly JournalGif[] = loaded ? gifs : [];
  const grid = useRovingGrid([list.length], GIF_COLUMNS);

  return (
    <div className="gif-panel">
      <Button variant="secondary" className="gif-panel__choose" onClick={onChoose}>
        {t("stickers.gifs.choose")}
      </Button>
      <div className="gif-panel__scroll" onKeyDown={grid.onKeyDown}>
        {gifs === null ? <p className="sticker-panel__note">{t("stickers.gifs.loading")}</p> : null}
        {gifs === "failed" ? (
          <p className="sticker-panel__note" role="alert">
            {t("stickers.gifs.loadFailed")}
          </p>
        ) : null}
        {loaded && list.length === 0 ? <p className="sticker-panel__note">{t("stickers.gifs.empty")}</p> : null}
        {list.length > 0 ? (
          <div className="gif-panel__grid" role="group" aria-label={t("stickers.gifs.label")}>
            {list.map((gif, index) => (
              <button
                key={gif.attachmentId}
                type="button"
                className="gif-panel__cell"
                aria-label={t("stickers.gifs.item", { width: gif.width, height: gif.height })}
                onClick={() => onSend(gif)}
                {...grid.cell(0, index)}
              >
                <img className="gif-panel__image" src={mediaUrl("thumb", gif.attachmentId)} alt="" loading="lazy" decoding="async" draggable={false} />
                <GifBadge />
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
