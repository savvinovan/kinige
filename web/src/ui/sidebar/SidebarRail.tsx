import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { IconButton } from "../controls/IconButton";
import { Icon } from "../icons/Icon";
import { useMenuTrigger, type MenuPosition } from "../menu/ContextMenu";
import { chatLabel } from "./ChatRow";
import { ChatTile, NavSlot } from "./ChatTile";
import { displayName, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";
import { ProfileMenu } from "./ProfileMenu";

interface Tip {
  label: string;
  /** The right edge of the button and its vertical middle, in window coordinates. */
  left: number;
  top: number;
}

interface RailButtonProps {
  label: string;
  /** The hover label: the name only, without the counter. */
  tip: string;
  selected: boolean;
  /** The chat of this button: the drop target for dragging an entry (`data-chat-drop`). */
  dropId?: string;
  /** An entry is being dragged over the button right now: highlight the target. */
  dropOver?: boolean;
  onClick?: () => void;
  onShowTip: (element: HTMLElement, label: string) => void;
  onHideTip: () => void;
  onOpenMenu?: (position: MenuPosition) => void;
  children: ReactNode;
}

/** A square rail button: the name for the screen reader is `aria-label`, for the eyes a tooltip to the right of the rail. */
function RailButton({ label, tip, selected, dropId, dropOver, onClick, onShowTip, onHideTip, onOpenMenu, children }: RailButtonProps) {
  const trigger = useMenuTrigger((position) => onOpenMenu?.(position));
  return (
    <button
      type="button"
      className={`rail-button${selected ? " rail-button--selected" : ""}${dropOver ? " rail-button--drop" : ""}`}
      data-chat-drop={dropId}
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
  /** The rail chats: without archived ones, in display order; an open archived chat goes last (otherwise the open chat has no icon). */
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
  onOpenImport?: () => void;
  onOpenAbout?: () => void;
  forestMenu?: { hidden: boolean; onToggle: () => void };
  /** The search icon: expands the sidebar and focuses the field (slice 4). */
  onSearch?: () => void;
  /** The chat an entry is dragged over (target highlight). */
  dropTargetId?: string | null;
}

/**
 * The collapsed sidebar is an "emoji rail": "expand" on top, then the chat emoji, the trash and the profile avatar at the
 * bottom. The chat name is visible on hover and focus: the tooltip is drawn over the window to the right of the rail (the
 * chat list scrolls and would clip it). There are no archived chats in the rail: the archive opens in the expanded view.
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
  onOpenImport,
  onOpenAbout,
  forestMenu,
  onSearch,
  dropTargetId = null,
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
        {onSearch ? <IconButton icon="search" label={t("sidebar.search")} onClick={onSearch} /> : null}
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
                  dropId={chat.id}
                  dropOver={chat.id === dropTargetId}
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
                  <ChatTile chat={chat} name={name} />
                </RailButton>
              </li>
            );
          })}
        </ul>
      </nav>
      <div className="sidebar__bottom">
        {onOpenTrash ? (
          <RailButton
            label={trashCount > 0 ? `${t("sidebar.trash")}, ${t("count.entries", { count: trashCount })}` : t("sidebar.trash")}
            tip={t("sidebar.trash")}
            selected={selectedId === null}
            onClick={onOpenTrash}
            onShowTip={showTip}
            onHideTip={hideTip}
          >
            <NavSlot>
              <Icon name="trash" />
            </NavSlot>
            {trashCount > 0 ? <span className="rail-button__dot" aria-hidden="true" /> : null}
          </RailButton>
        ) : null}
        <ProfileMenu compact name={profile?.name} statusDot={profile?.statusDot} onOpenSettings={onOpenSettings} onOpenImport={onOpenImport} onOpenAbout={onOpenAbout} forestMenu={forestMenu} />
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
