import { useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import { useMediaUrl } from "../attachments/mediaUrl";
import { Icon } from "../icons/Icon";
import { ContextMenu, type MenuPosition, useMenuTrigger } from "../menu/ContextMenu";
import { packTitle } from "./packTitle";

/** One button of the strip: `recent`, a pack (by its index in the packs), or the "+" at the end. */
export type StripTarget = { kind: "recent" } | { kind: "pack"; index: number };

interface StickerStripProps {
  packs: readonly StickerPackView[];
  /** Whether "Recent" is in the strip (it is when something was sent). */
  hasRecent: boolean;
  /** A click on an icon: the grid scrolls to that section. */
  onJump: (target: StripTarget) => void;
  onAddPack: () => void;
  onRemovePack: (pack: StickerPackView) => void;
}

/**
 * The pack strip above the sticker grid, as in Telegram: Recent first, then "From Telegram" and the packs in the order added, and a "+" at the end.
 * A toolbar with one Tab stop (roving tabindex); left, right, Home and End move along it. A pack icon is its first sticker. A right click on a pack
 * icon (or the menu key, Shift+F10) opens "Remove pack"; Recent has no menu. 32 px high, from the token `layout.sticker.strip-icon`.
 */
export function StickerStrip({ packs, hasRecent, onJump, onAddPack, onRemovePack }: StickerStripProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const strip = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(0);
  const [menu, setMenu] = useState<{ pack: StickerPackView; position: MenuPosition } | null>(null);
  const count = (hasRecent ? 1 : 0) + packs.length + 1;
  const current = Math.min(active, count - 1);

  // The buttons in the order they are drawn: the places the arrows count.
  const buttons = () => [...(strip.current?.querySelectorAll<HTMLButtonElement>(":scope > button") ?? [])];

  const focusAt = (index: number) => {
    const next = Math.max(0, Math.min(count - 1, index));
    setActive(next);
    buttons()[next]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const from = buttons().indexOf(event.target as HTMLButtonElement);
    const to = { ArrowRight: from + 1, ArrowLeft: from - 1, Home: 0, End: count - 1 }[event.key];
    if (to === undefined || from < 0) return;
    event.preventDefault();
    focusAt(to);
  };

  const offset = hasRecent ? 1 : 0;
  const bind = (index: number) => ({
    tabIndex: index === current ? 0 : -1,
    onFocus: () => setActive(index),
  });

  return (
    <div className="sticker-strip" ref={strip} role="toolbar" aria-label={t("stickers.packs")} aria-orientation="horizontal" onKeyDown={onKeyDown}>
      {hasRecent ? (
        <button type="button" className="sticker-strip__item" aria-label={t("stickers.recent")} title={t("stickers.recent")} {...bind(0)} onClick={() => onJump({ kind: "recent" })}>
          <Icon name="recent" />
        </button>
      ) : null}
      {packs.map((pack, index) => {
        const title = packTitle(pack, t);
        const first = pack.stickers[0];
        return (
          <PackButton
            key={pack.id}
            title={title}
            src={first ? mediaUrl("sticker", first.id) : null}
            {...bind(offset + index)}
            onClick={() => onJump({ kind: "pack", index })}
            onMenu={(position) => setMenu({ pack, position })}
          />
        );
      })}
      <button type="button" className="sticker-strip__item" aria-label={t("stickers.addPack")} title={t("stickers.addPack")} {...bind(count - 1)} onClick={onAddPack}>
        <Icon name="plus" />
      </button>
      {menu ? (
        <ContextMenu
          items={[{ id: "remove", label: t("stickers.removePack"), icon: "trash", danger: true }]}
          position={menu.position}
          label={packTitle(menu.pack, t)}
          onClose={() => setMenu(null)}
          onSelect={() => {
            const { pack } = menu;
            setMenu(null);
            onRemovePack(pack);
          }}
        />
      ) : null}
    </div>
  );
}

interface PackButtonProps {
  title: string;
  src: string | null;
  tabIndex: number;
  onFocus: () => void;
  onClick: () => void;
  onMenu: (position: MenuPosition) => void;
}

function PackButton({ title, src, tabIndex, onFocus, onClick, onMenu }: PackButtonProps) {
  const trigger = useMenuTrigger(onMenu);
  return (
    <button type="button" className="sticker-strip__item" aria-label={title} title={title} tabIndex={tabIndex} onFocus={onFocus} onClick={onClick} {...trigger}>
      {src ? <img className="sticker-strip__image" src={src} alt="" draggable={false} /> : null}
    </button>
  );
}
