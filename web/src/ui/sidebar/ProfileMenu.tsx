import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import "../controls/controls.css";
import { Icon } from "../icons/Icon";
import { DEFAULT_PROFILE_NAME, initials } from "./initials";
import "./profile.css";

const ITEMS = ["Настройки", "Экспорт", "О программе"] as const;

interface ProfileMenuProps {
  /** Имя в строке профиля; по умолчанию «Я». Из него берутся инициалы аватара. */
  name?: string;
  /** Точка статуса у аватара (синхронизация, слайс 11). В D1 не показывается; значение появится вместе со слайсом. */
  statusDot?: boolean;
  /** Меню открыто с самого начала (для историй и снимков). */
  defaultOpen?: boolean;
}

/**
 * Строка профиля внизу сайдбара, как в Notion: аватар с инициалами, имя, шеврон и меню «Настройки», «Экспорт»,
 * «О программе». В D1 только вид: пункты объявлены недоступными (`aria-disabled`, «Появится позже»), но меню настоящее:
 * открывается щелчком, Enter, пробелом или стрелкой; стрелки, Home и End ходят по пунктам, Esc закрывает и возвращает
 * фокус на кнопку, щелчок вне меню и Tab закрывают.
 */
export function ProfileMenu({ name = DEFAULT_PROFILE_NAME, statusDot = false, defaultOpen = false }: ProfileMenuProps) {
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

  // При открытии фокус уходит на первый пункт (или на последний, если меню открыли стрелкой вверх).
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
        // Меню закрывается, фокус идёт дальше по странице как обычно.
        setOpen(false);
        break;
    }
  };

  return (
    <div className="profile" ref={root}>
      {open ? (
        <div className="profile__menu" role="menu" id={menuId} aria-label="Профиль" ref={menu} onKeyDown={onMenuKeyDown}>
          {ITEMS.map((item) => (
            <button
              key={item}
              type="button"
              role="menuitem"
              className="profile__item"
              tabIndex={-1}
              aria-disabled="true"
              aria-describedby={noteId}
              title={`${item}. Появится позже`}
              onClick={(event) => event.preventDefault()}
            >
              {item}
            </button>
          ))}
          <span className="visually-hidden" id={noteId}>
            Появится позже
          </span>
        </div>
      ) : null}
      <button
        type="button"
        className="profile__button"
        ref={button}
        aria-label={`Профиль: ${name}`}
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
          {initials(name)}
          {statusDot ? <span className="profile__dot" /> : null}
        </span>
        <span className="profile__name">{name}</span>
        <span className="profile__chevron" aria-hidden="true">
          <Icon name="chevron" />
        </span>
      </button>
    </div>
  );
}
