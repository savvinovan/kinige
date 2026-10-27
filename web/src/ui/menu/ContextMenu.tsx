import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { Icon, type IconName } from "../icons/Icon";
import "./menu.css";

export interface MenuItem {
  id: string;
  label: string;
  icon?: IconName;
  /** Эмодзи вместо значка (данные пользователя: эмодзи чата в подменю «Переместить в чат»). */
  emoji?: string;
  /** Опасное действие («Удалить»): акцентируется цветом ошибки. */
  danger?: boolean;
  /** Пункт виден, но недоступен (`aria-disabled`); стрелки его пропускают. */
  disabled?: boolean;
  /** Тонкая черта над пунктом: группы действий. */
  separatorBefore?: boolean;
}

export interface MenuPosition {
  x: number;
  y: number;
}

interface ContextMenuProps {
  items: readonly MenuItem[];
  /** Угол меню в координатах окна (щелчок мыши или нижний левый угол кнопки). Меню не выходит за края окна. */
  position: MenuPosition;
  /** Подпись меню для диктора. */
  label: string;
  onSelect: (id: string) => void;
  onClose: () => void;
  /**
   * Подменю (два уровня в одной панели): Esc и стрелка влево возвращают на уровень выше, а не закрывают меню. Без него Esc
   * закрывает.
   */
  onBack?: () => void;
  /** Меняется, когда пункты заменены другим уровнем: фокус снова уходит на первый доступный пункт. */
  levelKey?: string;
}

const isEnabled = (element: HTMLElement) => element.getAttribute("aria-disabled") !== "true";

/**
 * Контекстное меню дизайн-системы (не системное): накладывается поверх окна и ничего не сдвигает. Открывается правым
 * щелчком, кнопкой «ещё» и с клавиатуры (`useMenuTrigger`). Стрелки, Home и End ходят по пунктам, Enter и пробел выбирают,
 * Esc закрывает и возвращает фокус туда, где он был; щелчок вне меню, прокрутка, смена размера окна и Tab закрывают.
 */
export function ContextMenu({ items, position, label, onSelect, onClose, onBack, levelKey }: ContextMenuProps) {
  const panel = useRef<HTMLDivElement>(null);
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const [placed, setPlaced] = useState<MenuPosition>(position);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // Меню прижимается к краям окна: если справа или снизу не хватает места, оно открывается влево или вверх от точки.
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

  const enabledItems = () => [...(panel.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])].filter(isEnabled);

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
            role="menuitem"
            className={`context-menu__item${item.danger ? " context-menu__item--danger" : ""}`}
            tabIndex={-1}
            aria-disabled={item.disabled ? true : undefined}
            onClick={() => {
              if (!item.disabled) onSelect(item.id);
            }}
          >
            <span className={`context-menu__icon${item.emoji ? " context-menu__icon--emoji" : ""}`} aria-hidden="true">
              {item.emoji ?? (item.icon ? <Icon name={item.icon} /> : null)}
            </span>
            <span className="context-menu__label">{item.label}</span>
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}

/** Позиция меню по прямоугольнику элемента (кнопка «ещё», клавиатура): нижний левый угол. */
export function positionBelow(element: Element): MenuPosition {
  const rect = element.getBoundingClientRect();
  return { x: rect.left, y: rect.bottom };
}

/**
 * Обработчики для элемента, у которого есть контекстное меню: правый щелчок и клавиша меню / Shift+F10. Меню по клавиатуре
 * открывается под самим элементом.
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
