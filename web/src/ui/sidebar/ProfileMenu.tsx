import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import "../controls/controls.css";
import { Icon } from "../icons/Icon";
import { initials } from "./initials";
import "./profile.css";

const ITEMS = ["settings", "import", "export", "about"] as const;
type Item = (typeof ITEMS)[number];

interface ProfileMenuProps {
  /** Имя в строке профиля; по умолчанию «Я» / «Me» из переводов. Из него берутся инициалы аватара. */
  name?: string;
  /** Точка статуса у аватара (синхронизация, слайс 11). В D1 не показывается; значение появится вместе со слайсом. */
  statusDot?: boolean;
  /** Меню открыто с самого начала (для историй и снимков). */
  defaultOpen?: boolean;
  /** «Настройки» (слайс 1). Без обработчика пункт только нарисован и объявлен недоступным, как «Экспорт» и «О программе». */
  onOpenSettings?: () => void;
  /** «Импорт из Telegram…» (слайс 4). Без обработчика пункт нарисован и объявлен недоступным. */
  onOpenImport?: () => void;
  /** Свёрнутый сайдбар: только аватар, меню открывается вправо от него, а не вверх на всю ширину. */
  compact?: boolean;
}

/**
 * Строка профиля внизу сайдбара, как в Notion: аватар с инициалами, имя, шеврон и меню «Настройки», «Импорт из
 * Telegram…», «Экспорт», «О программе». Работают «Настройки» и «Импорт» (если переданы обработчики); остальные пункты объявлены недоступными
 * (`aria-disabled`, «Появится позже»), но меню настоящее: открывается щелчком, Enter, пробелом или стрелкой; стрелки,
 * Home и End ходят по пунктам, Esc закрывает и возвращает фокус на кнопку, щелчок вне меню и Tab закрывают.
 * В свёрнутом сайдбаре (`compact`) остаётся аватар; имя читается по подписи кнопки.
 */
export function ProfileMenu({
  name,
  statusDot = false,
  defaultOpen = false,
  onOpenSettings,
  onOpenImport,
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

  const handlers: Partial<Record<Item, () => void>> = { settings: onOpenSettings, import: onOpenImport };
  const itemAvailable = (item: Item) => handlers[item] !== undefined;

  return (
    <div className={compact ? "profile profile--compact" : "profile"} ref={root}>
      {open ? (
        <div className="profile__menu" role="menu" id={menuId} aria-label={t("profile.menu")} ref={menu} onKeyDown={onMenuKeyDown}>
          {ITEMS.map((item) =>
            itemAvailable(item) ? (
              <button
                key={item}
                type="button"
                role="menuitem"
                className="profile__item"
                tabIndex={-1}
                onClick={() => {
                  // Фокус сначала возвращается на кнопку профиля (пункт меню исчезнет): окно настроек запомнит её
                  // и вернёт фокус на неё при закрытии.
                  close(true);
                  handlers[item]?.();
                }}
              >
                {t(`profile.${item}`)}
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
                title={`${t(`profile.${item}`)}. ${t("common.comingLater")}`}
                onClick={(event) => event.preventDefault()}
              >
                {t(`profile.${item}`)}
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
