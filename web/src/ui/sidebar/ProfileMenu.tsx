import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import "../controls/controls.css";
import { Icon, type IconName } from "../icons/Icon";
import { NavSlot } from "./ChatTile";
import { initials } from "./initials";
import "./profile.css";

const ITEMS = ["settings", "import", "export", "about"] as const;
/** With the forest (slice 6) a "Show forest" / "Hide forest" item follows "Settings". */
const ITEMS_WITH_FOREST = ["settings", "forest", "import", "export", "about"] as const;
type Item = (typeof ITEMS_WITH_FOREST)[number];

const ITEM_ICON: Record<Item, IconName> = { settings: "settings", forest: "forest", import: "download", export: "upload", about: "info" };

interface ProfileMenuProps {
  /** Name in the profile row; defaults to "Я" / "Me" from the translations. The avatar initials come from it. */
  name?: string;
  /** Status dot by the avatar (sync, slice 11). Not shown in D1; the value arrives with the slice. */
  statusDot?: boolean;
  /** The menu is open from the start (for stories and screenshots). */
  defaultOpen?: boolean;
  /** "Settings" (slice 1). Without a handler the item is only drawn and announced as unavailable, like "Export" and "About". */
  onOpenSettings?: () => void;
  /** "Import from Telegram..." (slice 4). Without a handler the item is drawn and announced as unavailable. */
  onOpenImport?: () => void;
  /** "About" (KINIG-54): the build version. Without a handler the item is drawn and announced as unavailable. */
  onOpenAbout?: () => void;
  /** The forest above the feed (slice 6): without it the menu has no forest item; `hidden` decides "Show" or "Hide". */
  forestMenu?: { hidden: boolean; onToggle: () => void };
  /** Collapsed sidebar: only the avatar; the menu opens to the right of it, not upward at full width. */
  compact?: boolean;
}

/**
 * The profile row at the bottom of the sidebar, as in Notion: avatar with initials, name, chevron and a menu with
 * "Settings", "Import from Telegram...", "Export", "About". "Settings", "Import" and "About" work (if handlers are
 * passed); "Export" is announced as unavailable (`aria-disabled`, "Coming later"), but the menu is real: it opens with
 * a click, Enter, Space or an arrow; arrows, Home and End move between items, Esc closes it and returns focus to the
 * button, a click outside the menu and Tab close it.
 * In the collapsed sidebar (`compact`) only the avatar stays; the name is read from the button label.
 */
export function ProfileMenu({
  name,
  statusDot = false,
  defaultOpen = false,
  onOpenSettings,
  onOpenImport,
  onOpenAbout,
  forestMenu,
  compact = false,
}: ProfileMenuProps) {
  const { t } = useTranslation();
  const displayName = name ?? t("profile.defaultName");
  const [open, setOpen] = useState(defaultOpen);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const noteId = useId();

  const items = () => [...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
  const focusItem = (index: number) => {
    const list = items();
    list[(index + list.length) % list.length]?.focus();
  };
  const close = (returnFocus: boolean) => {
    setOpen(false);
    if (returnFocus) button.current?.focus();
  };

  // On open, focus goes to the first item (or to the last one if the menu was opened with the up arrow).
  const opener = useRef<"first" | "last">("first");
  useEffect(() => {
    if (!open) return;
    const list = [...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    (opener.current === "last" ? list.at(-1) : list[0])?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const onButtonKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    opener.current = event.key === "ArrowUp" ? "last" : "first";
    setOpen(true);
  };

  const onMenuKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = items();
    const current = list.indexOf(document.activeElement as HTMLElement);
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusItem(current + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusItem(current - 1);
        break;
      case "Home":
        event.preventDefault();
        focusItem(0);
        break;
      case "End":
        event.preventDefault();
        focusItem(-1);
        break;
      case "Escape":
        event.preventDefault();
        close(true);
        break;
      case "Tab":
        // The menu closes; focus moves on through the page as usual.
        setOpen(false);
        break;
    }
  };

  const handlers: Partial<Record<Item, () => void>> = { settings: onOpenSettings, forest: forestMenu?.onToggle, import: onOpenImport, about: onOpenAbout };
  const itemAvailable = (item: Item) => handlers[item] !== undefined;
  const itemLabel = (item: Item) => (item === "forest" ? t(forestMenu?.hidden ? "profile.forestShow" : "profile.forestHide") : t(`profile.${item}`));

  return (
    <div className={compact ? "profile profile--compact" : "profile"} ref={root}>
      {open ? (
        <div className="profile__menu" role="menu" id={menuId} aria-label={t("profile.menu")} ref={menu} onKeyDown={onMenuKeyDown}>
          {(forestMenu ? ITEMS_WITH_FOREST : ITEMS).map((item) =>
            itemAvailable(item) ? (
              <button
                key={item}
                type="button"
                role="menuitem"
                className="profile__item"
                tabIndex={-1}
                onClick={() => {
                  // Focus first returns to the profile button (the menu item will disappear): the settings window
                  // remembers it and returns focus to it on close.
                  close(true);
                  handlers[item]?.();
                }}
              >
                <NavSlot>
                  <Icon name={ITEM_ICON[item]} />
                </NavSlot>
                <span className="profile__item-label">{itemLabel(item)}</span>
              </button>
            ) : (
              <button
                key={item}
                type="button"
                role="menuitem"
                className="profile__item"
                tabIndex={-1}
                aria-disabled="true"
                aria-describedby={noteId}
                title={`${itemLabel(item)}. ${t("common.comingLater")}`}
                onClick={(event) => event.preventDefault()}
              >
                <NavSlot>
                  <Icon name={ITEM_ICON[item]} />
                </NavSlot>
                <span className="profile__item-label">{itemLabel(item)}</span>
              </button>
            ),
          )}
          <span className="visually-hidden" id={noteId}>
            {t("common.comingLater")}
          </span>
        </div>
      ) : null}
      <button
        type="button"
        className="profile__button"
        ref={button}
        aria-label={t("profile.button", { name: displayName })}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => {
          opener.current = "first";
          setOpen((value) => !value);
        }}
        onKeyDown={onButtonKeyDown}
      >
        <span className="profile__avatar" aria-hidden="true">
          {initials(displayName, t("profile.defaultName"))}
          {statusDot ? <span className="profile__dot" /> : null}
        </span>
        {compact ? null : (
          <>
            <span className="profile__name">{displayName}</span>
            <span className="profile__chevron" aria-hidden="true">
              <Icon name="chevron" />
            </span>
          </>
        )}
      </button>
    </div>
  );
}
