import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { useLanguage } from "../../i18n/I18n";
import { IconButton } from "../controls/IconButton";
import { Icon } from "../icons/Icon";
import { fileIcon, fileMetaText } from "./fileMeta";
import { FileName } from "./fileName";
import { useMediaUrl } from "./mediaUrl";
import { photoLabel } from "./photoLabel";
import { PreviewTile } from "./PreviewTile";
import "./pending-tiles.css";

interface PendingTilesProps {
  /** The unsent attachments of the draft, in order. */
  items: readonly AttachmentView[];
  /** The x on a tile. */
  onRemove: (attachmentId: string) => void;
}

/**
 * The row of tiles above the composer field (slice 7): a square thumbnail for a photo, a compact card with the icon, the name and the size for
 * any other file, and an x on each. The row wraps, so ten tiles stay reachable at any window width without a scroll bar.
 */
export function PendingTiles({ items, onRemove }: PendingTilesProps) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  return (
    <ul className="pending-tiles" aria-label={t("attachments.pendingLabel")}>
      {items.map((item) => (
        <li className="pending-tiles__item" key={item.id}>
          {item.kind === "image" ? <ImageTile item={item} onRemove={onRemove} /> : <FileTile item={item} onRemove={onRemove} />}
        </li>
      ))}
    </ul>
  );
}

type TileProps = { item: AttachmentView; onRemove: (attachmentId: string) => void };

function ImageTile({ item, onRemove }: TileProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const [failed, setFailed] = useState(false);
  const missing = item.state === "missing";
  return (
    <div className="pending-tile pending-tile--image">
      {missing || failed ? (
        <PreviewTile reason={missing ? "missing" : "noPreview"} />
      ) : (
        <img
          className="pending-tile__image"
          src={mediaUrl("thumb", item.id)}
          alt={photoLabel(item, t)}
          draggable={false}
          onError={() => setFailed(true)}
        />
      )}
      <RemoveButton item={item} onRemove={onRemove} />
    </div>
  );
}

function FileTile({ item, onRemove }: TileProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const missing = item.state === "missing";
  return (
    <div className="pending-tile pending-tile--file">
      <span className="pending-tile__icon">
        <Icon name={fileIcon(item.kind)} />
      </span>
      <span className="pending-tile__text">
        <FileName name={item.filename} className="pending-tile__name" />
        <span className="pending-tile__meta">{missing ? t("attachments.missing") : fileMetaText(item, language, t)}</span>
      </span>
      <RemoveButton item={item} onRemove={onRemove} />
    </div>
  );
}

function RemoveButton({ item, onRemove }: TileProps) {
  const { t } = useTranslation();
  return <IconButton icon="close" small className="pending-tile__remove" label={t("attachments.remove", { name: item.filename })} onClick={() => onRemove(item.id)} />;
}
