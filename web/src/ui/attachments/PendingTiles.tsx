import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
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
import { useReveal } from "./reveal";
import { SpoilerFace } from "./SpoilerFace";
import "./pending-tiles.css";

/** A picture narrower than this (width / height, 3:4) is cropped from its top in the narrowest tile; the same number as the token `layout.attachment.pending-image-min-aspect`, which sets the tile's width (a test compares them). */
export const TALL_PICTURE_ASPECT = 0.75;

interface PendingTilesProps {
  /** The unsent attachments of the draft, in order. */
  items: readonly AttachmentView[];
  /** The x on a tile. It may return a promise that says whether the tile is gone (`false`: it failed and stays), so no focus hand-over is left waiting. */
  onRemove: (attachmentId: string) => void | Promise<boolean>;
  /** A click or Enter on a picture tile: its attachment id (the window opens the viewer). Without it a picture is not a button. */
  onOpen?: (attachmentId: string) => void;
  /** "Details" (a pencil) on a tile, to the left of the x: its attachment id (the window opens the dialog). Without it there is no such button. */
  onDetails?: (attachmentId: string) => void;
}

/**
 * The tiles inside the composer's box, above the text line (slice 7, KINIG-135): a picture in a tile of one height with its own aspect ratio
 * kept, a card with the icon, the name and the size for any other file, and a round x on the top right corner of each. The tiles wrap, so
 * ten of them stay reachable at any window width without a horizontal scroll bar; the composer scrolls the area vertically past two lines.
 * Nothing animates when a tile comes or goes. A hidden photo (KINIG-136) is a cover that fills the tile until its first click; both round buttons
 * stay on top of it.
 */
export function PendingTiles({ items, onRemove, onOpen, onDetails }: PendingTilesProps) {
  const { t } = useTranslation();
  const list = useRef<HTMLUListElement>(null);
  const { arm, disarm } = useFocusHandOver(items, list);
  if (items.length === 0) return null;
  const remove = (attachmentId: string) => {
    arm(attachmentId);
    // A removal that failed leaves the tile and no hand-over waiting for it.
    void Promise.resolve(onRemove(attachmentId)).then((gone) => {
      if (gone === false) disarm(attachmentId);
    });
  };
  return (
    <ul className="pending-tiles" aria-label={t("attachments.pendingLabel")} ref={list}>
      {items.map((item) => (
        <li className="pending-tiles__item" key={item.id}>
          {item.kind === "image" ? (
            <ImageTile item={item} onRemove={remove} onOpen={onOpen} onDetails={onDetails} />
          ) : (
            <FileTile item={item} onRemove={remove} onDetails={onDetails} />
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * Keyboard focus after a tile's x (PRD §12.2): the removed button disappears, and the focus must not drop to the page. It goes to the x of
 * the next tile, else of the previous one. When the last tile goes, the list goes too and the composer gives the focus to the text field.
 * `arm(id)` is called right before the removal starts (the list changes later: the removal is asked of Rust first); `disarm(id)` when it failed.
 * The focus moves only if it is on the page (`<body>`) when the tile goes, as the removed button had it: if the person has focused something
 * else meanwhile (a slow removal), it stays there.
 */
function useFocusHandOver(items: readonly AttachmentView[], list: { current: HTMLUListElement | null }) {
  const waiting = useRef<{ removed: string; next: string | undefined } | null>(null);
  useLayoutEffect(() => {
    const pending = waiting.current;
    if (!pending || items.some((item) => item.id === pending.removed)) return;
    waiting.current = null;
    const active = document.activeElement;
    if (pending.next === undefined || (active !== null && active !== document.body && active !== document.documentElement)) return;
    const buttons = list.current?.querySelectorAll<HTMLElement>("[data-remove-for]") ?? [];
    for (const button of buttons) if (button.dataset.removeFor === pending.next) button.focus();
  }, [items, list]);
  const arm = (removed: string) => {
    const at = items.findIndex((item) => item.id === removed);
    // Only a press of the x that has the focus hands it over: after a mouse click the focus is where the person left it.
    const focused = document.activeElement;
    const own = list.current?.contains(focused) === true && focused instanceof HTMLElement && focused.dataset.removeFor === removed;
    waiting.current = own ? { removed, next: (items[at + 1] ?? items[at - 1])?.id } : null;
  };
  const disarm = (removed: string) => {
    if (waiting.current?.removed === removed) waiting.current = null;
  };
  return { arm, disarm };
}

type TileProps = { item: AttachmentView; onRemove: (attachmentId: string) => void; onDetails?: (attachmentId: string) => void };

function ImageTile({ item, onRemove, onOpen, onDetails }: TileProps & { onOpen?: (attachmentId: string) => void }) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const { isRevealed, reveal } = useReveal();
  const [failed, setFailed] = useState(false);
  const missing = item.state === "missing";
  const covered = item.spoiler && !isRevealed(item.id);
  const label = covered ? t("attachments.hiddenPhoto") : photoLabel(item, t);
  const openable = onOpen !== undefined && !missing;
  // A hidden photo is a button from its first frame to its last, so the focus stays on it when the cover goes (and the first click is a reveal).
  const asButton = openable || item.spoiler;
  let picture: ReactNode;
  if (covered) picture = <SpoilerFace />;
  else if (missing || failed) picture = <PreviewTile reason={missing ? "missing" : "noPreview"} />;
  else {
    picture = (
      <img
        className="pending-tile__image"
        src={mediaUrl("thumb", item.id)}
        alt={asButton ? "" : label}
        draggable={false}
        onError={() => setFailed(true)}
      />
    );
  }
  const press = () => {
    if (covered) reveal(item.id);
    else if (openable) onOpen(item.id);
  };
  return (
    <div className={isTall(item) ? "pending-tile pending-tile--image pending-tile--tall" : "pending-tile pending-tile--image"} style={imageBox(item)}>
      {asButton ? (
        <button
          type="button"
          className="pending-tile__open"
          aria-label={covered || !missing ? label : `${label}. ${t("attachments.missing")}`}
          aria-disabled={(!covered && !openable) || undefined}
          onClick={press}
        >
          {picture}
        </button>
      ) : (
        <Surface label={missing ? `${label}. ${t("attachments.missing")}` : undefined}>{picture}</Surface>
      )}
      <DetailsButton item={item} covered={covered} onDetails={onDetails} />
      <RemoveButton item={item} onRemove={onRemove} covered={covered} />
    </div>
  );
}

/** "Details" (a pencil): the same round button as the x, to its left. It is not there when the window gives no handler. */
function DetailsButton({ item, covered, onDetails }: { item: AttachmentView; covered: boolean; onDetails?: (attachmentId: string) => void }) {
  const { t } = useTranslation();
  if (!onDetails) return null;
  // A hidden photo's name is not read out either until it is shown.
  const label = covered ? t("attachments.detailsOfHidden") : t("attachments.detailsOf", { name: item.name });
  return <IconButton icon="edit" className="pending-tile__details" label={label} data-details-for={item.id} onClick={() => onDetails(item.id)} />;
}

/** The picture area of a tile that is not a button: a missing picture is announced as one image with its state. */
function Surface({ label, children }: { label?: string; children: ReactNode }) {
  return label === undefined ? (
    <div className="pending-tile__open">{children}</div>
  ) : (
    <div className="pending-tile__open" role="img" aria-label={label}>
      {children}
    </div>
  );
}

function FileTile({ item, onRemove, onDetails }: TileProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const missing = item.state === "missing";
  return (
    <div className="pending-tile pending-tile--file">
      <span className="pending-tile__icon">
        <Icon name={fileIcon(item.kind)} />
      </span>
      <span className="pending-tile__text">
        <FileName name={item.name} className="pending-tile__name" />
        <span className="pending-tile__meta">{missing ? t("attachments.missing") : fileMetaText(item, language, t)}</span>
      </span>
      <DetailsButton item={item} covered={false} onDetails={onDetails} />
      <RemoveButton item={item} onRemove={onRemove} />
    </div>
  );
}

function RemoveButton({ item, onRemove, covered = false }: TileProps & { covered?: boolean }) {
  const { t } = useTranslation();
  return (
    <IconButton
      icon="close"
      className="pending-tile__remove"
      // The name of a covered photo is not read out through this button either (as in the cover's own label and in "Details").
      label={covered ? t("attachments.removeHidden") : t("attachments.remove", { name: item.name })}
      data-remove-for={item.id}
      onClick={() => onRemove(item.id)}
    />
  );
}

/** A picture taller than 3:4: its tile has the narrowest width and shows the top of the picture. */
function isTall({ width, height }: AttachmentView): boolean {
  return width !== null && height !== null && width > 0 && height > 0 && width / height < TALL_PICTURE_ASPECT;
}

/** The shape of a picture's tile from its pixel size, so the thumbnail arriving does not change the tile; a picture of unknown size gets the default shape. */
function imageBox({ width, height }: AttachmentView): CSSProperties {
  if (width === null || height === null || width <= 0 || height <= 0) return {};
  return { "--pending-aspect": `${width / height}` } as CSSProperties;
}
