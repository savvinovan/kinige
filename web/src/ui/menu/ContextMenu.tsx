import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon, type IconName } from "../icons/Icon";
import "./menu.css";

export interface MenuItem {
  id: string;
  label: string;
  icon?: IconName;
  /**
   * A chat tile instead of an icon (`ChatTile`: the chat emoji or the coloured letter; the "Move to chat" and "Colour" submenus).
   * It has the size of a sidebar tile, so the row text is shifted to the right of the usual icon column by the difference.
   */
  tile?: ReactNode;
  /**
   * A choice among alternatives (the "Colour" submenu): the item becomes `menuitemradio`, the current one is `aria-checked` and
   * has a check mark at the end. `undefined` is an ordinary item.
   */
  checked?: boolean;
  /** The item opens a second level in the same panel: a chevron at the end (the caller switches the items on select). */
  submenu?: boolean;
  /** A dangerous action ("Delete"): emphasized with the error color. */
  danger?: boolean;
  /** The item is visible but unavailable (`aria-disabled`); the arrows skip it. */
  disabled?: boolean;
  /** A thin line above the item: groups of actions. */
  separatorBefore?: boolean;
}

export interface MenuPosition {
  x: number;
  y: number;
}

interface ContextMenuProps {
  items: readonly MenuItem[];
  /** The menu corner in window coordinates (a mouse click or the bottom left corner of a button). The menu does not go beyond the window edges. */
  position: MenuPosition;
  /** The menu label for the screen reader. */
  label: string;
  onSelect: (id: string) => void;
  onClose: () => void;
  /**
   * A submenu (two levels in one panel): Esc and the left arrow go back up a level instead of closing the menu. Without it Esc
   * closes.
   */
  onBack?: () => void;
  /** Changes when the items are replaced by another level: focus goes to the first available item again. */
  levelKey?: string;
}

const isEnabled = (element: HTMLElement) => element.getAttribute("aria-disabled") !== "true";

/**
 * The design system's context menu (not the system one): overlays the window and shifts nothing. It opens with a right
 * click, the "more" button and from the keyboard (`useMenuTrigger`). Arrows, Home and End move between items, Enter and space select,
 * Esc closes and returns focus to where it was; a click outside the menu, scrolling, a window resize and Tab close it.
 */
export function ContextMenu({ items, position, label, onSelect, onClose, onBack, levelKey }: ContextMenuProps) {
  const panel = useRef<HTMLDivElement>(null);
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const [placed, setPlaced] = useState<MenuPosition>(position);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // The menu is kept within the window edges: if there is not enough room on the right or at the bottom, it opens to the left or upward from the point.
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    const fitsRight = position.x + width <= window.innerWidth;
    const fitsBelow = position.y + height <= window.innerHeight;
    setPlaced({
      x: Math.max(0, fitsRight ? position.x : position.x - width),
      y: Math.max(0, fitsBelow ? position.y : position.y - height),
    });
  }, [position.x, position.y, items.length]);

  const enabledItems = () =>
    [...(panel.current?.querySelectorAll<HTMLElement>('[role="menuitem"], [role="menuitemradio"]') ?? [])].filter(isEnabled);

  useEffect(() => {
    enabledItems()[0]?.focus();
  }, [levelKey]);

  useEffect(() => {
    const close = () => onCloseRef.current();
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !panel.current?.contains(event.target)) close();
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    document.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
      document.removeEventListener("scroll", close, true);
      if (opener?.isConnected) opener.focus();
    };
  }, [opener]);

  const move = (to: number) => {
    const list = enabledItems();
    if (list.length === 0) return;
    list[(to + list.length) % list.length]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = enabledItems();
    const current = list.indexOf(document.activeElement as HTMLElement);
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        move(current + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        move(current <= 0 ? -1 : current - 1);
        break;
      case "Home":
        event.preventDefault();
        move(0);
        break;
      case "End":
        event.preventDefault();
        move(-1);
        break;
      case "ArrowLeft":
        if (onBack) {
          event.preventDefault();
          onBack();
        }
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        if (onBack) onBack();
        else onClose();
        break;
      case "Tab":
        event.preventDefault();
        onClose();
        break;
    }
  };

  return createPortal(
    <div
      className="context-menu"
      role="menu"
      aria-label={label}
      ref={panel}
      style={{ left: placed.x, top: placed.y }}
      onKeyDown={onKeyDown}
      onContextMenu={(event) => event.preventDefault()}
    >
      {items.map((item) => (
        <div key={item.id} role="none" className={item.separatorBefore ? "context-menu__group" : undefined}>
          <button
            type="button"
            role={item.checked === undefined ? "menuitem" : "menuitemradio"}
            className={`context-menu__item${item.danger ? " context-menu__item--danger" : ""}`}
            tabIndex={-1}
            aria-disabled={item.disabled ? true : undefined}
            aria-checked={item.checked}
            aria-haspopup={item.submenu ? "menu" : undefined}
            onClick={() => {
              if (!item.disabled) onSelect(item.id);
            }}
          >
            {item.tile ? (
              <span className="context-menu__tile" aria-hidden="true">
                {item.tile}
              </span>
            ) : (
              <span className="context-menu__icon" aria-hidden="true">
                {item.icon ? <Icon name={item.icon} /> : null}
              </span>
            )}
            <span className="context-menu__label">{item.label}</span>
            {item.checked ? (
              <span className="context-menu__check" aria-hidden="true">
                <Icon name="done" />
              </span>
            ) : null}
            {item.submenu ? (
              <span className="context-menu__check" aria-hidden="true">
                <Icon name="chevronRight" />
              </span>
            ) : null}
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}

/** The menu position from an element's rectangle (the "more" button, keyboard): the bottom left corner. */
export function positionBelow(element: Element): MenuPosition {
  const rect = element.getBoundingClientRect();
  return { x: rect.left, y: rect.bottom };
}

/**
 * Handlers for an element that has a context menu: a right click and the menu key / Shift+F10. A menu opened by keyboard
 * opens right under the element itself.
 */
export function useMenuTrigger(open: (position: MenuPosition) => void) {
  return {
    onContextMenu: (event: MouseEvent<HTMLElement>) => {
      event.preventDefault();
      open({ x: event.clientX, y: event.clientY });
    },
    onKeyDown: (event: KeyboardEvent<HTMLElement>) => {
      if (event.target !== event.currentTarget) return;
      if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault();
        open(positionBelow(event.currentTarget));
      }
    },
  };
}
