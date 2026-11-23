import { useId, useRef, useState, type UIEvent } from "react";
import { useTranslation } from "react-i18next";
import type { StickerLibrary } from "../../api/generated/StickerLibrary";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import type { StickerView } from "../../api/generated/StickerView";
import { useMediaUrl } from "../attachments/mediaUrl";
import { STICKER_COLUMNS } from "./gridNav";
import { packTitle } from "./packTitle";
import { StickerStrip, type StripTarget } from "./StickerStrip";
import { useRovingGrid } from "./useRovingGrid";
import "./stickers.css";

/** How many pixels below the top edge of the grid a section still counts as "at the top" for the strip's mark. */
const VIEWED_SLACK = 8;

interface StickerPanelProps {
  /** `null` while the library is being read; `"failed"` when it could not be. */
  library: StickerLibrary | "failed" | null;
  /** A click or Enter on a sticker: it is sent at once as its own entry. */
  onSend: (sticker: StickerView) => void;
  onAddPack: () => void;
  onRemovePack: (pack: StickerPackView) => void;
}

interface Section {
  key: string;
  title: string;
  stickers: readonly StickerView[];
}

/**
 * The Stickers tab of the emoji window (slice 5c), as in Telegram: a strip of pack icons (Recent, From Telegram, the packs, "+") above a grid of five
 * columns under pack headings, the most recently sent first in Recent. Every sticker is a button labelled "Sticker 😺"; the arrows, Home, End and Enter
 * work as in the emoji grid. There is no search and no preview. Empty: one line telling how to get stickers, and the strip with the "+" stays.
 */
export function StickerPanel({ library, onSend, onAddPack, onRemovePack }: StickerPanelProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const headingId = useId();
  const sectionRefs = useRef<(HTMLElement | null)[]>([]);
  const [viewed, setViewed] = useState(0);
  const ready = library !== null && library !== "failed" ? library : null;
  const sections: Section[] = ready
    ? [
        ...(ready.recent.length > 0 ? [{ key: "recent", title: t("stickers.recent"), stickers: ready.recent }] : []),
        ...ready.packs.map((pack) => ({ key: pack.id, title: packTitle(pack, t), stickers: pack.stickers })),
      ]
    : [];
  const grid = useRovingGrid(
    sections.map((section) => section.stickers.length),
    STICKER_COLUMNS,
  );

  const jump = (target: StripTarget) => {
    const hasRecent = (ready?.recent.length ?? 0) > 0;
    const index = target.kind === "recent" ? 0 : (hasRecent ? 1 : 0) + target.index;
    setViewed(index);
    sectionRefs.current[index]?.scrollIntoView?.({ block: "start" });
  };

  // The strip marks the pack whose section is at the top of the grid: the last section that has reached the top edge (a few pixels of slack). A last pack
  // shorter than the grid never reaches the top, so at the end of the scroll the last section is the marked one.
  const onScroll = (event: UIEvent<HTMLDivElement>) => {
    const area = event.currentTarget;
    const top = area.getBoundingClientRect().top + VIEWED_SLACK;
    let index = 0;
    sectionRefs.current.forEach((section, at) => {
      if (section && section.getBoundingClientRect().top <= top) index = at;
    });
    const scrollable = area.scrollHeight > area.clientHeight;
    if (scrollable && area.scrollTop + area.clientHeight >= area.scrollHeight - VIEWED_SLACK) index = Math.max(index, sections.length - 1);
    setViewed(index);
  };

  return (
    <div className="sticker-panel">
      <StickerStrip
        packs={ready?.packs ?? []}
        hasRecent={(ready?.recent.length ?? 0) > 0}
        viewed={sections.length > 0 ? Math.min(viewed, sections.length - 1) : null}
        onJump={jump}
        onAddPack={onAddPack}
        onRemovePack={onRemovePack}
      />
      <div className="sticker-panel__scroll" onKeyDown={grid.onKeyDown} onScroll={onScroll}>
        {library === null ? <p className="sticker-panel__note">{t("stickers.loading")}</p> : null}
        {library === "failed" ? (
          <p className="sticker-panel__note" role="alert">
            {t("stickers.loadFailed")}
          </p>
        ) : null}
        {ready && sections.length === 0 ? <p className="sticker-panel__note">{t("stickers.empty")}</p> : null}
        {sections.map((section, sectionIndex) => (
          <section
            key={section.key}
            className="sticker-panel__section"
            aria-labelledby={`${headingId}-${sectionIndex}`}
            ref={(element) => {
              sectionRefs.current[sectionIndex] = element;
            }}
          >
            <h3 className="sticker-panel__heading" id={`${headingId}-${sectionIndex}`} title={section.title}>
              {section.title}
            </h3>
            <div className="sticker-panel__grid">
              {section.stickers.map((sticker, index) => (
                <button
                  key={sticker.id}
                  type="button"
                  className="sticker-panel__cell"
                  aria-label={sticker.emoji ? t("stickers.sticker", { emoji: sticker.emoji }) : t("stickers.stickerNoEmoji")}
                  onClick={() => onSend(sticker)}
                  {...grid.cell(sectionIndex, index)}
                >
                  <img
                    className="sticker-panel__image"
                    src={mediaUrl("sticker", sticker.id)}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    draggable={false}
                  />
                </button>
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
