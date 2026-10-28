import { useId, useMemo, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { IconButton } from "../controls/IconButton";
import { TextField } from "../controls/TextField";
import { Icon } from "../icons/Icon";
import { ContextMenu, type MenuPosition } from "../menu/ContextMenu";
import { ChatRow } from "./ChatRow";
import { chatMenuItems, displayName, partitionChats, type ChatAction, type SidebarChat } from "./chats";
import { formatCount } from "./formatCount";
import { ProfileMenu } from "./ProfileMenu";
import { SidebarRail } from "./SidebarRail";
import "./sidebar.css";

export { toSidebarChat } from "./chats";
export type { ChatAction, SidebarChat } from "./chats";

/** Рабочее поле «Поиск» (слайс 4): значение, клавиши и связь с результатами (combobox: `aria-activedescendant`). */
export interface SidebarSearch {
  value: string;
  onChange: (value: string) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
  inputRef?: Ref<HTMLInputElement>;
  /** Результаты показаны на месте ленты: поле — combobox, `aria-controls` указывает на список. */
  resultsId?: string;
  /** Id выбранной стрелками строки результатов. */
  activeDescendant?: string;
  /** Значок поиска в свёрнутом сайдбаре: разворачивает сайдбар и ставит фокус в поле. */
  onRailClick?: () => void;
}

interface SidebarProps {
  /** Чаты в порядке показа. Разделы выводятся по флагам: `archived` — «Архив», иначе `pinned` — «Закреплённые», иначе «Чаты». */
  chats: readonly SidebarChat[];
  /** Выбранный чат; `null` — выбрана корзина. */
  selectedId: string | null;
  /** Без обработчика строки только показывают выбор (истории D1). */
  onSelect?: (chatId: string) => void;
  /** Свёрнут в «полосу эмодзи». */
  collapsed?: boolean;
  /** Кнопка «свернуть» / «развернуть»; без обработчика кнопки нет. */
  onToggleCollapsed?: () => void;
  /** Кнопка «+» в заголовке раздела «Чаты»; без обработчика кнопки нет. */
  onCreateChat?: () => void;
  /** Пункт контекстного меню чата. Без обработчика меню у строк нет. */
  onChatAction?: (chatId: string, action: ChatAction) => void;
  /** Сколько записей в корзине (тихий счётчик в строке «Корзина»). */
  trashCount?: number;
  /** Строка «Корзина» над строкой профиля; без обработчика строки нет. */
  onOpenTrash?: () => void;
  /** Строка профиля внизу: имя (по умолчанию «Я») и точка статуса у аватара (слайс 11; в D1 не показывается). */
  profile?: { name?: string; statusDot?: boolean };
  /** «Настройки» в меню профиля (слайс 1). */
  onOpenSettings?: () => void;
  /** Поле «Поиск». В историях D1 оно только нарисовано; рабочее поле — `search`. */
  showSearch?: boolean;
  /** Рабочий поиск (слайс 4): поле управляется снаружи. Без него поле только нарисовано (если `showSearch`). */
  search?: SidebarSearch;
  /** «Импорт из Telegram…» в меню профиля (слайс 4). */
  onOpenImport?: () => void;
  /** Чат, над которым сейчас тащат запись (решение приёмки): подсветка цели; сами строки всегда цели (`data-chat-drop`). */
  dropTargetId?: string | null;
}

interface OpenMenu {
  chat: SidebarChat;
  position: MenuPosition;
}

/**
 * Сайдбар: поле «Поиск», чаты в разделах «Закреплённые» / «Чаты» / «Архив» (строки «эмодзи + имя + счётчик записей»),
 * «Корзина» и профиль внизу. Выбранный чат подсвечен. Счётчик справа — тихий серый (не акцент): сколько записей накоплено,
 * а не «что не прочитано». Заголовок раздела виден, только если в разделе есть чаты; заголовок «Чаты» с кнопкой «+» виден
 * всегда, пока есть `onCreateChat` (иначе при закреплённом единственном чате создать новый было бы нечем). «Архив» свёрнут и
 * раскрывается сам, если выбран чат из архива.
 *
 * Правый щелчок, Shift+F10 и клавиша меню открывают меню чата (`ContextMenu`). В свёрнутом виде сайдбар — «полоса эмодзи»
 * (`SidebarRail`); смена вида анимирует только ширину (длительность из токена, при Reduce Motion — сразу).
 */
export function Sidebar({
  chats,
  selectedId,
  onSelect,
  collapsed = false,
  onToggleCollapsed,
  onCreateChat,
  onChatAction,
  trashCount = 0,
  onOpenTrash,
  profile,
  onOpenSettings,
  showSearch = true,
  search,
  onOpenImport,
  dropTargetId = null,
}: SidebarProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const archiveListId = useId();
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const sections = useMemo(() => partitionChats(chats), [chats]);
  const selectedInArchive = sections.archived.some((chat) => chat.id === selectedId);
  // «Архив» раскрывается сам, когда человек открывает архивный чат; дальше кнопка работает как обычный переключатель:
  // свёрнутый раздел прячет все свои чаты, в том числе открытый (открытый чат остаётся в ленте).
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
          {onOpenTrash ? (
            <button
              type="button"
              className={`chat-row${selectedId === null ? " chat-row--selected" : ""}`}
              aria-current={selectedId === null ? "true" : undefined}
              onClick={onOpenTrash}
            >
              <span className="chat-row__emoji" aria-hidden="true">
                <Icon name="trash" />
              </span>
              <span className="chat-row__name">{t("sidebar.trash")}</span>
              {trashCounter ? (
                <span className="chat-row__count">
                  <span className="visually-hidden">{t("sidebar.entriesLabel")}</span> {trashCounter}
                </span>
              ) : null}
            </button>
          ) : null}
          <ProfileMenu name={profile?.name} statusDot={profile?.statusDot} onOpenSettings={onOpenSettings} onOpenImport={onOpenImport} />
        </>
      )}
      {menu && onChatAction ? (
        <ContextMenu
          items={chatMenuItems(menu.chat, t)}
          position={menu.position}
          label={t("chatMenu.label", { name: displayName(menu.chat, t) })}
          onSelect={(id) => {
            const { chat } = menu;
            setMenu(null);
            onChatAction(chat.id, id as ChatAction);
          }}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </aside>
  );
}

interface SectionProps {
  title: string;
  /** Кнопка справа от заголовка («+» у «Чатов»). */
  action?: ReactNode;
  children: ReactNode;
}

/** Раздел списка: тихий серый заголовок и список строк. */
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
