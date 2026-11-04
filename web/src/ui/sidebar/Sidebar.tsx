import { useId, useMemo, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { IconButton } from "../controls/IconButton";
import { TextField } from "../controls/TextField";
import { Icon } from "../icons/Icon";
import type { MenuPosition } from "../menu/ContextMenu";
import { ChatMenu } from "./ChatMenu";
import { ChatRow } from "./ChatRow";
import { NavSlot } from "./ChatTile";
import { partitionChats, type ChatAction, type ChatTint, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";
import { ProfileMenu } from "./ProfileMenu";
import { SidebarRail } from "./SidebarRail";
import "./sidebar.css";

export { toSidebarChat } from "./chats";
export type { ChatAction, ChatTint, SidebarChat } from "./chats";

/** The working "Search" field (slice 4): the value, keys and the link to the results (combobox: `aria-activedescendant`). */
export interface SidebarSearch {
  value: string;
  onChange: (value: string) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
  inputRef?: Ref<HTMLInputElement>;
  /** The results are shown in place of the feed: the field is a combobox, `aria-controls` points to the list. */
  resultsId?: string;
  /** The id of the result row selected with the arrows. */
  activeDescendant?: string;
  /** The search icon in the collapsed sidebar: expands the sidebar and focuses the field. */
  onRailClick?: () => void;
}

interface SidebarProps {
  /** The chats in display order. Sections are derived from flags: `archived` is "Archive", otherwise `pinned` is "Pinned", otherwise "Chats". */
  chats: readonly SidebarChat[];
  /** The selected chat; `null` means the trash is selected. */
  selectedId: string | null;
  /** Without a handler the rows only show the selection (D1 stories). */
  onSelect?: (chatId: string) => void;
  /** Collapsed into the "emoji rail". */
  collapsed?: boolean;
  /** The "collapse" / "expand" button; without a handler there is no button. */
  onToggleCollapsed?: () => void;
  /** The "+" button in the "Chats" section header; without a handler there is no button. */
  onCreateChat?: () => void;
  /** A chat context menu item. Without a handler the rows have no menu. */
  onChatAction?: (chatId: string, action: ChatAction) => void;
  /** "Colour" in the chat menu (KINIG-59): the chosen tile colour. Without a handler the item is not shown. */
  onChatColor?: (chatId: string, color: ChatTint) => void;
  /** How many entries are in the trash (the quiet counter in the "Trash" row). */
  trashCount?: number;
  /** The "Trash" row above the profile row; without a handler there is no row. */
  onOpenTrash?: () => void;
  /** The profile row at the bottom: the name (default "Я") and the status dot by the avatar (slice 11; not shown in D1). */
  profile?: { name?: string; statusDot?: boolean };
  /** "Settings" in the profile menu (slice 1). */
  onOpenSettings?: () => void;
  /** The "Search" field. In D1 stories it is only drawn; the working field is `search`. */
  showSearch?: boolean;
  /** Working search (slice 4): the field is controlled from outside. Without it the field is only drawn (if `showSearch`). */
  search?: SidebarSearch;
  /** "Import from Telegram..." in the profile menu (slice 4). */
  onOpenImport?: () => void;
  /** "About" in the profile menu (KINIG-54). */
  onOpenAbout?: () => void;
  /** "Show forest" / "Hide forest" in the profile menu (slice 6): the only way back after the forest is hidden. */
  forestMenu?: { hidden: boolean; onToggle: () => void };
  /** The chat an entry is dragged over right now (acceptance decision): target highlight; the rows themselves are always targets (`data-chat-drop`). */
  dropTargetId?: string | null;
}

interface OpenMenu {
  chat: SidebarChat;
  position: MenuPosition;
}

/**
 * The sidebar: the "Search" field, chats in the "Pinned" / "Chats" / "Archive" sections (rows "emoji + name + entry
 * counter"), the "Trash" and the profile at the bottom. The selected chat is highlighted. The counter on the right is
 * quiet gray (not an accent): how many entries have accumulated, not "what is unread". A section header is visible only
 * if the section has chats; the "Chats" header with the "+" button is always visible while `onCreateChat` exists
 * (otherwise, with a single pinned chat, there would be no way to create a new one). "Archive" is collapsed and
 * expands by itself if a chat from the archive is selected.
 *
 * Right click, Shift+F10 and the menu key open the chat menu (`ContextMenu`). In the collapsed view the sidebar is an
 * "emoji rail" (`SidebarRail`); the view change animates only the width (duration from a token, instant with Reduce Motion).
 */
export function Sidebar({
  chats,
  selectedId,
  onSelect,
  collapsed = false,
  onToggleCollapsed,
  onCreateChat,
  onChatAction,
  onChatColor,
  trashCount = 0,
  onOpenTrash,
  profile,
  onOpenSettings,
  showSearch = true,
  search,
  onOpenImport,
  onOpenAbout,
  forestMenu,
  dropTargetId = null,
}: SidebarProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const archiveListId = useId();
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const sections = useMemo(() => partitionChats(chats), [chats]);
  const selectedInArchive = sections.archived.some((chat) => chat.id === selectedId);
  // "Archive" expands by itself when the person opens an archived chat; after that the button works as a plain toggle:
  // a collapsed section hides all its chats, including the open one (the open chat stays in the feed).
  const [archiveOpen, setArchiveOpen] = useState(selectedInArchive);
  const [seenSelected, setSeenSelected] = useState(selectedId);
  if (seenSelected !== selectedId) {
    setSeenSelected(selectedId);
    if (selectedInArchive) setArchiveOpen(true);
  }
  const archiveShown = archiveOpen;
  const openMenu = onChatAction ? (chat: SidebarChat, position: MenuPosition) => setMenu({ chat, position }) : undefined;

  const rows = (list: readonly SidebarChat[]) =>
    list.map((chat) => (
      <ChatRow
        key={chat.id}
        chat={chat}
        selected={chat.id === selectedId}
        dropOver={chat.id === dropTargetId}
        language={language}
        onSelect={onSelect}
        onOpenMenu={openMenu}
      />
    ));

  const trashCounter = formatCount(trashCount, language);

  return (
    <aside className={collapsed ? "sidebar sidebar--rail" : "sidebar"}>
      {collapsed ? (
        <SidebarRail
          chats={[...sections.pinned, ...sections.chats, ...sections.archived.filter((chat) => chat.id === selectedId)]}
          selectedId={selectedId}
          language={language}
          onSelect={onSelect}
          onOpenMenu={openMenu}
          onToggleCollapsed={onToggleCollapsed}
          trashCount={trashCount}
          onOpenTrash={onOpenTrash}
          profile={profile}
          onOpenSettings={onOpenSettings}
          onOpenImport={onOpenImport}
          onOpenAbout={onOpenAbout}
          forestMenu={forestMenu}
          onSearch={search?.onRailClick}
          dropTargetId={dropTargetId}
        />
      ) : (
        <>
          {showSearch || search || onToggleCollapsed ? (
            <div className="sidebar__top">
              {search ? (
                <TextField
                  className="sidebar__search"
                  label={t("sidebar.search")}
                  icon="search"
                  type="search"
                  placeholder={t("sidebar.search")}
                  value={search.value}
                  onChange={(event) => search.onChange(event.target.value)}
                  onKeyDown={search.onKeyDown}
                  inputRef={search.inputRef}
                  role="combobox"
                  aria-expanded={search.resultsId !== undefined}
                  aria-controls={search.resultsId}
                  aria-activedescendant={search.activeDescendant}
                  aria-autocomplete="list"
                  autoComplete="off"
                />
              ) : showSearch ? (
                <TextField className="sidebar__search" label={t("sidebar.search")} icon="search" type="search" placeholder={t("sidebar.search")} />
              ) : null}
              {onToggleCollapsed ? <IconButton icon="collapse" label={t("sidebar.collapse")} onClick={onToggleCollapsed} /> : null}
            </div>
          ) : null}
          <nav aria-label={t("sidebar.chats")} className="sidebar__nav">
            {sections.pinned.length > 0 ? <Section title={t("sidebar.pinned")}>{rows(sections.pinned)}</Section> : null}
            {sections.chats.length > 0 || onCreateChat ? (
              <Section
                title={t("sidebar.chats")}
                action={onCreateChat ? <IconButton small icon="plus" label={t("sidebar.newChat")} onClick={onCreateChat} /> : undefined}
              >
                {rows(sections.chats)}
              </Section>
            ) : null}
            {sections.archived.length > 0 ? (
              <div className="sidebar__section">
                <button
                  type="button"
                  className="sidebar__heading sidebar__heading--toggle"
                  aria-expanded={archiveShown}
                  aria-controls={archiveListId}
                  onClick={() => setArchiveOpen((was) => !was)}
                >
                  <span className="sidebar__heading-text">{t("sidebar.archive")}</span>
                  <Icon name={archiveShown ? "chevron" : "chevronRight"} />
                </button>
                {archiveShown ? (
                  <ul className="sidebar__list" id={archiveListId}>
                    {rows(sections.archived)}
                  </ul>
                ) : null}
              </div>
            ) : null}
          </nav>
          <div className="sidebar__bottom">
            {onOpenTrash ? (
              <button
                type="button"
                className={`chat-row${selectedId === null ? " chat-row--selected" : ""}`}
                aria-current={selectedId === null ? "true" : undefined}
                onClick={onOpenTrash}
              >
                <NavSlot>
                  <Icon name="trash" />
                </NavSlot>
                <span className="chat-row__name">{t("sidebar.trash")}</span>
                {trashCounter ? (
                  <span className="chat-row__count">
                    <span className="visually-hidden">{t("sidebar.entriesLabel")}</span> {trashCounter}
                  </span>
                ) : null}
              </button>
            ) : null}
            <ProfileMenu name={profile?.name} statusDot={profile?.statusDot} onOpenSettings={onOpenSettings} onOpenImport={onOpenImport} onOpenAbout={onOpenAbout} forestMenu={forestMenu} />
          </div>
        </>
      )}
      {menu && onChatAction ? (
        <ChatMenu
          chat={menu.chat}
          position={menu.position}
          onAction={(action) => onChatAction(menu.chat.id, action)}
          onColor={onChatColor ? (color) => onChatColor(menu.chat.id, color) : undefined}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </aside>
  );
}

interface SectionProps {
  title: string;
  /** The button to the right of the header ("+" by "Chats"). */
  action?: ReactNode;
  children: ReactNode;
}

/** A list section: a quiet gray header and a list of rows. */
function Section({ title, action, children }: SectionProps) {
  const headingId = useId();
  return (
    <div className="sidebar__section" role="group" aria-labelledby={headingId}>
      <div className="sidebar__heading">
        <span className="sidebar__heading-text" id={headingId}>
          {title}
        </span>
        {action}
      </div>
      <ul className="sidebar__list">{children}</ul>
    </div>
  );
}
