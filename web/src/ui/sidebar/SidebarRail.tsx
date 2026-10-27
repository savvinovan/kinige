import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { IconButton } from "../controls/IconButton";
import { Icon } from "../icons/Icon";
import { useMenuTrigger, type MenuPosition } from "../menu/ContextMenu";
import { chatLabel } from "./ChatRow";
import { displayName, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";
import { ProfileMenu } from "./ProfileMenu";

interface Tip {
  label: string;
  /** Правый край кнопки и её середина по вертикали в координатах окна. */
  left: number;
  top: number;
}

interface RailButtonProps {
  label: string;
  /** Подпись при наведении: только имя, без счётчика. */
  tip: string;
  selected: boolean;
  onClick?: () => void;
  onShowTip: (element: HTMLElement, label: string) => void;
  onHideTip: () => void;
  onOpenMenu?: (position: MenuPosition) => void;
  children: ReactNode;
}

/** Квадратная кнопка полосы: имя для диктора — `aria-label`, для глаз — подсказка справа от полосы. */
function RailButton({ label, tip, selected, onClick, onShowTip, onHideTip, onOpenMenu, children }: RailButtonProps) {
  const trigger = useMenuTrigger((position) => onOpenMenu?.(position));
  return (
    <button
      type="button"
      className={`rail-button${selected ? " rail-button--selected" : ""}`}
      aria-label={label}
      aria-current={selected ? "true" : undefined}
      aria-haspopup={onOpenMenu ? "menu" : undefined}
      onClick={onClick}
      onPointerEnter={(event) => onShowTip(event.currentTarget, tip)}
      onPointerLeave={onHideTip}
      onFocus={(event) => onShowTip(event.currentTarget, tip)}
      onBlur={onHideTip}
      {...(onOpenMenu ? trigger : {})}
    >
      {children}
    </button>
  );
}

interface SidebarRailProps {
  /** Чаты полосы: без архивных, в порядке показа. */
  chats: readonly SidebarChat[];
  selectedId: string | null;
  language: Language;
  onSelect?: (chatId: string) => void;
  onOpenMenu?: (chat: SidebarChat, position: MenuPosition) => void;
  onToggleCollapsed?: () => void;
  trashCount?: number;
  onOpenTrash?: () => void;
  profile?: { name?: string; statusDot?: boolean };
  onOpenSettings?: () => void;
}

/**
 * Свёрнутый сайдбар — «полоса эмодзи»: сверху «развернуть», дальше эмодзи чатов, внизу корзина и аватар профиля. Имя чата
 * видно при наведении и фокусе: подсказка рисуется поверх окна справа от полосы (список чатов прокручивается и обрезал
 * бы её). Архивных чатов в полосе нет — архив открывается в развёрнутом виде.
 */
export function SidebarRail({
  chats,
  selectedId,
  language,
  onSelect,
  onOpenMenu,
  onToggleCollapsed,
  trashCount = 0,
  onOpenTrash,
  profile,
  onOpenSettings,
}: SidebarRailProps) {
  const { t } = useTranslation();
  const [tip, setTip] = useState<Tip | null>(null);
  const showTip = (element: HTMLElement, label: string) => {
    const rect = element.getBoundingClientRect();
    setTip({ label, left: rect.right, top: rect.top + rect.height / 2 });
  };
  const hideTip = () => setTip(null);

  return (
    <>
      <div className="sidebar__rail-top">
        {onToggleCollapsed ? <IconButton icon="expand" label={t("sidebar.expand")} onClick={onToggleCollapsed} /> : null}
      </div>
      <nav aria-label={t("sidebar.chats")} className="sidebar__nav" onScroll={hideTip}>
        <ul className="sidebar__list">
          {chats.map((chat) => {
            const name = displayName(chat, t);
            const counter = formatCount(chat.count ?? 0, language);
            return (
              <li key={chat.id}>
                <RailButton
                  label={chatLabel(chat, name, counter, t)}
                  tip={name}
                  selected={chat.id === selectedId}
                  onClick={onSelect ? () => onSelect(chat.id) : undefined}
                  onShowTip={showTip}
                  onHideTip={hideTip}
                  onOpenMenu={
                    onOpenMenu
                      ? (position) => {
                          hideTip();
                          onOpenMenu(chat, position);
                        }
                      : undefined
                  }
                >
                  <span className="chat-row__emoji" aria-hidden="true">
                    {chat.emoji}
                  </span>
                </RailButton>
              </li>
            );
          })}
        </ul>
      </nav>
      <div className="sidebar__rail-bottom">
        {onOpenTrash ? (
          <RailButton
            label={trashCount > 0 ? `${t("sidebar.trash")}, ${t("count.entries", { count: trashCount })}` : t("sidebar.trash")}
            tip={t("sidebar.trash")}
            selected={selectedId === null}
            onClick={onOpenTrash}
            onShowTip={showTip}
            onHideTip={hideTip}
          >
            <Icon name="trash" />
            {trashCount > 0 ? <span className="rail-button__dot" aria-hidden="true" /> : null}
          </RailButton>
        ) : null}
        <ProfileMenu compact name={profile?.name} statusDot={profile?.statusDot} onOpenSettings={onOpenSettings} />
      </div>
      {tip
        ? createPortal(
            <div className="rail-tip" aria-hidden="true" style={{ left: tip.left, top: tip.top }}>
              {tip.label}
            </div>,
            document.body,
          )
        : null}
    </>
  );
}
