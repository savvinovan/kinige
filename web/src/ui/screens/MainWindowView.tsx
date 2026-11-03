import { useState, type PointerEvent, type ReactNode } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { Language } from "../../api/generated/Language";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { TrashItem } from "../../api/generated/TrashItem";
import { useLanguage } from "../../i18n/I18n";
import { DropOverlay } from "../attachments/DropOverlay";
import { PendingTiles } from "../attachments/PendingTiles";
import { Composer, type ComposerButtons } from "../composer/Composer";
import { Feed, type FeedActions, type FeedPaging, type FeedUiState, type ScrollRequest } from "../feed/Feed";
import type { FeedMessage } from "../feed/buildFeed";
import type { MoveTarget } from "../message/moveTargets";
import { type ChatAction, Sidebar, type SidebarChat, type SidebarSearch } from "../sidebar/Sidebar";
import { TagFilterBar } from "../tag/TagFilterBar";
import { TrashView } from "../trash/TrashView";
import "./main-window.css";

/** Sidebar collapsing, chat creation, the chat menu and the trash. */
export interface SidebarControls {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onCreateChat: () => void;
  onChatAction: (chatId: string, action: ChatAction) => void;
  trashCount: number;
  onOpenTrash: () => void;
}

/** The trash in place of the feed. */
export interface TrashControls {
  items: readonly TrashItem[];
  today: string;
  busyId?: string | null;
  error?: string;
  onRestore: (id: string) => void;
  onPurge: (id: string) => void;
}

interface MainWindowViewProps {
  chats: readonly SidebarChat[];
  /** The selected chat; `null` means the trash is open. */
  selectedChatId: string | null;
  messages: readonly FeedMessage[];
  /** Today's `journal_date` for the "Today" and "Yesterday" labels: Rust sends it in the app, it is fixed in stories. */
  today: string;
  onSelectChat?: (chatId: string) => void;
  onRetry?: (messageId: string) => void;
  onOpenLink?: (url: string) => void;
  /** Which buttons to show in the composer. By default all three are drawn and declared unavailable ("Coming later"), as in D1; the app shows only the working ones. */
  composerButtons?: ComposerButtons;
  /** The composer works (the app); without it, it is only drawn. */
  interactive?: boolean;
  /** The text in the composer. Without it, the view keeps the draft itself (stories). */
  draft?: string;
  onDraftChange?: (value: string) => void;
  onSubmit?: () => void;
  /** The error above the composer (already translated text). */
  composerError?: string;
  /** The emoji picker in the composer: recent ones, the language and "picked" (remember in recents). */
  emoji?: { recent: readonly string[]; language: Language; onPicked?: (emoji: string) => void };
  onOpenSettings?: () => void;
  /** The "Search" field in the sidebar, drawn without working (D1 stories). The working field is `search`. */
  showSearch?: boolean;
  /** Working search (slice 4): the sidebar field; the results are `searchResults`. */
  search?: SidebarSearch;
  /** Search results in place of the feed and the composer (the trash and search are not shown together: search wins). */
  searchResults?: ReactNode;
  /** "Import from Telegram..." in the profile menu. */
  onOpenImport?: () => void;
  /** "About" in the profile menu. */
  onOpenAbout?: () => void;
  sidebar?: SidebarControls;
  feedUi?: FeedUiState;
  feedActions?: FeedActions;
  /** Loading older and newer feed pages, scrolling to an entry and clearing the highlight of the entry opened from search (slice 4). */
  feedPaging?: FeedPaging;
  feedScrollTo?: ScrollRequest | null;
  onFeedPointerDown?: () => void;
  /** Slice 5: the "On this day" card in the feed, a click on a reply quote, the feed waiting for the first frame and the "Reply to entry" strip above the composer. */
  onThisDay?: { journalDate: string; node: ReactNode };
  onOpenReply?: (quote: ReplyTo) => void;
  /** A new number makes the feed scroll to the bottom edge (after the card is hidden). */
  feedToBottom?: number;
  feedLoading?: boolean;
  replyStrip?: ReactNode;
  /** The feed tag filter: the "#tag x" strip above the feed. */
  tagFilter?: { tag: string; onClear: () => void };
  trash?: TrashControls;
  /** Chats for "Move to chat" (the current one is marked). */
  moveTargets?: readonly MoveTarget[];
  /** Dragging an entry onto a chat (`useRecordDrag`): the plate press handler, the highlighted chat and the "ghost". */
  drag?: {
    onDragStart: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
    dropTargetId: string | null;
    ghost: ReactNode;
  };
  /** Slice 6: the forest pane above the feed (not in the trash or in search results), the feed's top day for it, and the profile menu item. */
  forest?: ReactNode;
  onFeedTopDay?: (journalDate: string | null) => void;
  forestMenu?: { hidden: boolean; onToggle: () => void };
  /** On top of the window: modal windows (settings, chat, calendar). */
  overlay?: ReactNode;
  /** Slice 7: the tiles above the composer, the paperclip, a paste, the drop overlay and opening a photo or a file from the feed. */
  attachments?: AttachmentControls;
}

/** What the window gives the feed and the composer for attachments (slice 7). */
export interface AttachmentControls {
  tiles: readonly AttachmentView[];
  onAttach: () => void;
  onRemoveTile: (attachmentId: string) => void;
  onPasteFiles: (files: readonly File[]) => void;
  /** Files are dragged over the window: "Drop to attach" over the feed. */
  dragOver: boolean;
  onOpenImage: (messageId: string, attachmentId: string) => void;
  onOpenFile: (attachmentId: string) => void;
}

/**
 * Main window: sidebar + feed (or trash) + composer. There is no chat-title row above the feed: the selected chat is
 * highlighted in the sidebar (§6). View only: data comes from outside (in Storybook directly, in the app via `MainWindow`).
 */
export function MainWindowView({
  chats,
  selectedChatId,
  messages,
  today,
  onSelectChat,
  onRetry,
  onOpenLink,
  composerButtons,
  interactive = false,
  draft,
  onDraftChange,
  onSubmit,
  composerError,
  emoji,
  onOpenSettings,
  showSearch = true,
  search,
  searchResults,
  onOpenImport,
  onOpenAbout,
  sidebar,
  feedUi,
  feedActions,
  feedPaging,
  feedScrollTo,
  onFeedPointerDown,
  onThisDay,
  onOpenReply,
  feedToBottom,
  feedLoading,
  replyStrip,
  tagFilter,
  trash,
  moveTargets,
  drag,
  forest,
  onFeedTopDay,
  forestMenu,
  overlay,
  attachments,
}: MainWindowViewProps) {
  const [ownDraft, setOwnDraft] = useState("");
  const language = useLanguage();
  return (
    <div className="main-window">
      <Sidebar
        chats={chats}
        selectedId={selectedChatId}
        onSelect={onSelectChat}
        onOpenSettings={onOpenSettings}
        onOpenImport={onOpenImport}
        onOpenAbout={onOpenAbout}
        forestMenu={forestMenu}
        showSearch={showSearch}
        search={search}
        collapsed={sidebar?.collapsed}
        onToggleCollapsed={sidebar?.onToggleCollapsed}
        onCreateChat={sidebar?.onCreateChat}
        onChatAction={sidebar?.onChatAction}
        trashCount={sidebar?.trashCount}
        onOpenTrash={sidebar?.onOpenTrash}
        dropTargetId={drag?.dropTargetId}
      />
      <main className="main-window__chat">
        {searchResults ? (
          searchResults
        ) : selectedChatId === null && trash ? (
          <TrashView
            items={trash.items}
            language={language}
            today={trash.today}
            busyId={trash.busyId}
            error={trash.error}
            onRestore={trash.onRestore}
            onPurge={trash.onPurge}
            onOpenAttachment={attachments?.onOpenFile}
          />
        ) : (
          <>
            {forest}
            {tagFilter ? <TagFilterBar tag={tagFilter.tag} onClear={tagFilter.onClear} /> : null}
            <Feed
              messages={messages}
              today={today}
              onRetry={onRetry}
              onOpenLink={onOpenLink}
              ui={feedUi}
              actions={feedActions}
              moveTargets={moveTargets}
              onDragStart={drag?.onDragStart}
              paging={feedPaging}
              scrollTo={feedScrollTo}
              onPointerDownFeed={onFeedPointerDown}
              card={onThisDay}
              onOpenReply={onOpenReply}
              toBottom={feedToBottom}
              loading={feedLoading}
              onTopDay={onFeedTopDay}
              listKey={selectedChatId ?? "trash"}
              onOpenImage={attachments?.onOpenImage}
              onOpenFile={attachments?.onOpenFile}
            />
            {attachments ? <DropOverlay visible={attachments.dragOver} /> : null}
            <Composer
              key={selectedChatId ?? "trash"}
              value={draft ?? ownDraft}
              onChange={onDraftChange ?? setOwnDraft}
              onSubmit={onSubmit}
              buttons={composerButtons}
              viewOnly={!interactive}
              error={composerError}
              autoFocus={interactive}
              emoji={emoji}
              replyStrip={replyStrip}
              tiles={attachments ? <PendingTiles items={attachments.tiles} onRemove={attachments.onRemoveTile} /> : undefined}
              hasTiles={(attachments?.tiles.length ?? 0) > 0}
              onAttach={attachments?.onAttach}
              onPasteFiles={attachments?.onPasteFiles}
            />
          </>
        )}
      </main>
      {drag?.ghost}
      {overlay}
    </div>
  );
}
