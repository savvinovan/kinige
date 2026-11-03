import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { INBOX_ID, type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ChatList } from "../../api/generated/ChatList";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Settings } from "../../api/generated/Settings";
import { useLanguage } from "../../i18n/I18n";
import { AboutDialog } from "../about/AboutDialog";
import { Lightbox } from "../attachments/Lightbox";
import { CalendarDialog } from "../calendar/CalendarDialog";
import { ChatDialog } from "../chat/ChatDialog";
import { useRecordDrag } from "../drag/useRecordDrag";
import { moveTargetsFrom } from "../message/moveTargets";
import type { ChatAction } from "../sidebar/Sidebar";
import { ImportDialog } from "../import/ImportDialog";
import { OnThisDayCard } from "../onthisday/OnThisDayCard";
import { ReplyStrip } from "../reply/ReplyStrip";
import { SearchResults } from "../search/SearchResults";
import { type ChatDialogState, EMPTY_LIST, focusComposer, toSidebar } from "./mainWindowParts";
import { MainWindowView } from "./MainWindowView";
import { useAttachments } from "./useAttachments";
import { useComposerDraft } from "./useComposerDraft";
import { type FocusTarget, useFeed } from "./useFeed";
import { useImport } from "./useImport";
import { useMainSearch } from "./useMainSearch";
import { useOnThisDay } from "./useOnThisDay";
import { useOpenMessage } from "./useOpenMessage";
import { useReplyActions } from "./useReplyActions";
import { useSettingsState } from "./useSettingsState";
import { useChangeCounter, useForestPane } from "./useForestPane";
import { useTrash } from "./useTrash";

interface MainWindowProps {
  api: KinigeApi;
  /** Settings read before the first frame (the text size is applied without flicker). */
  initialSettings: Settings;
}

/**
 * Main window on the real `KinigeApi`: chats (selecting, creating, name, emoji, pinning, archive), the feed paged from Rust,
 * entry actions, the trash, a composer with a per-chat draft, settings. No optimistic display (§5): an entry appears
 * after Rust replies; on an error the text stays in the composer with the error above it.
 */
export function MainWindow({ api, initialSettings }: MainWindowProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const [list, setList] = useState<ChatList>(EMPTY_LIST);
  /** The open chat; `null` means the trash. */
  const [selected, setSelected] = useState<string | null>(INBOX_ID);
  const [tag, setTag] = useState<string | null>(null);
  const [composerError, setComposerError] = useState<ErrorCode | null>(null);
  const [chatDialog, setChatDialog] = useState<ChatDialogState | null>(null);
  const [dialogError, setDialogError] = useState<ErrorCode | null>(null);
  const [dateError, setDateError] = useState<ErrorCode | null>(null);
  // "About": the build string from Rust ("Kinige 0.5.0"); `null` means the window is closed.
  const [about, setAbout] = useState<string | null>(null);
  // The number of the "to the bottom edge of the feed" request: grows after the "On this day" card is hidden.
  const [toBottom, setToBottom] = useState(0);
  const { draft, reply, tiles, addTiles, dropTile, changeReply, refreshReply, clear: clearDraft, plan: planDraft, saver: draftSaver, switchChat, type: typeDraft } =
    useComposerDraft(api, selected);
  const prefs = useSettingsState(api, initialSettings);
  const { settings } = prefs;

  const refreshChats = useCallback(
    () =>
      api
        .listChats()
        .then(setList)
        .catch(() => undefined),
    [api],
  );

  // Jump to an entry from search: the feed opens the chat at it (the page around it, found words highlighted).
  const [focus, setFocus] = useState<FocusTarget | null>(null);
  // Jumps to an entry come from search, the "Settings" window, the card and quotes: the `nonce` is shared and grows,
  // otherwise a second source with its own counter would get a number lower than one already handled and the feed would skip it.
  const focusOn = useCallback(
    (target: FocusTarget) => setFocus((previous) => ({ ...target, nonce: (previous?.nonce ?? 0) + 1 })),
    [],
  );
  // The forest reloads when the journal changes: through this feed (`onMutated`) or from outside (the capture window).
  const [mutations, bumpMutations] = useChangeCounter();
  const feed = useFeed({
    api,
    chatId: selected,
    tag,
    refreshChats,
    onError: setComposerError,
    onTag: setTag,
    focus,
    onMutated: bumpMutations,
  });
  const { reload: reloadFeed, restart: restartFeed } = feed;
  const attachments = useAttachments({ api, selected, tiles, addTiles, dropTile, messages: feed.messages, onError: setComposerError });
  const onThisDay = useOnThisDay({ api, enabled: selected === INBOX_ID && tag === null });
  // Window focus: has a new journal day begun (the "Today" and "Yesterday" labels in the feed; the card rereads itself).
  const { checkDay } = feed;
  useEffect(() => {
    const onFocus = () => void checkDay();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [checkDay]);
  const sidebarChats = useMemo(() => list.chats.map(toSidebar), [list.chats]);
  const moveTargets = useMemo(() => moveTargetsFrom(sidebarChats, selected, t), [sidebarChats, selected, t]);
  // Dragging a plate onto a chat: a drop on any chat except the one where the entry lives.
  const drag = useRecordDrag({
    canDrop: (chatId) => chatId !== selected,
    onDrop: (messageId, chatId) => feed.actions.onMoveTo(messageId, chatId),
  });
  const trash = useTrash({ api, open: selected === null, refreshChats, onRestored: bumpMutations });
  const { clearError: clearTrashError } = trash;

  /** Open a chat (or the trash on `null`). The previous chat's draft goes to Rust before switching. */
  const select = useCallback(
    async (next: string | null) => {
      await draftSaver.flush();
      switchChat(next);
      setSelected(next);
      setTag(null);
      setComposerError(null);
      clearTrashError();
    },
    [draftSaver, switchChat, clearTrashError],
  );

  const { search, sidebarSearch, datePick, setDatePick } = useMainSearch({
    api,
    selected,
    select,
    sidebarCollapsed: settings.sidebarCollapsed,
    toggleCollapsed: prefs.toggleCollapsed,
    onFocus: focusOn,
    inbox: { chat: list.chats.find((c) => c.isInbox), name: t("chat.inbox") },
  });
  // An entry from the access log (the "Settings" window) is opened in this window's feed.
  useOpenMessage({ api, select, onFocus: focusOn });
  const importer = useImport({
    api,
    // An import puts entries into the middle of the history: the open feed is reread from scratch together with the cursors.
    onChanged: () => void Promise.all([refreshChats(), restartFeed()]),
    // "Done": the Saved chat (Inbox), where the entries landed, is opened.
    onDone: () => void select(INBOX_ID),
  });

  useEffect(() => {
    void refreshChats();
  }, [refreshChats]);

  useEffect(() => {
    const offs: (() => void)[] = [];
    let closed = false;
    const keep = (off: () => void) => {
      if (closed) off();
      else offs.push(off);
    };
    // An entry came from the capture window or from the clipboard; the tray menu asks to open a chat.
    api
      .onEntriesChanged(() => {
        bumpMutations();
        void Promise.all([refreshChats(), reloadFeed()]);
      })
      .then(keep)
      .catch(() => undefined);
    api
      .onOpenChat((id) => void select(id))
      .then(keep)
      .catch(() => undefined);
    return () => {
      closed = true;
      for (const off of offs) off();
    };
  }, [api, refreshChats, reloadFeed, select, bumpMutations]);

  const { sending, submit, openEntry, startReply, hideCard } = useReplyActions({
    api,
    selected,
    select,
    focusOn,
    draft,
    tileIds: tiles.map((tile) => tile.id),
    reply,
    saver: draftSaver,
    changeReply,
    refreshReply,
    clearDraft,
    planDraft,
    appendSent: feed.appendSent,
    refreshChats,
    setComposerError,
    dismissCard: onThisDay.dismiss,
    onDismissed: () => setToBottom((count) => count + 1),
  });
  const forest = useForestPane({
    api,
    initial: { view: initialSettings.forestView, mode: initialSettings.forestMode },
    chatId: selected,
    ready: feed.loaded,
    refreshKey: `${mutations}:${feed.today}`,
    openEntry,
  });

  const closeChatDialog = () => {
    setChatDialog(null);
    setDialogError(null);
  };

  const submitChatDialog = (name: string, emoji: string | null) => {
    if (!chatDialog) return;
    setDialogError(null);
    const request =
      chatDialog.mode === "create"
        ? api.createChat(name, emoji)
        : chatDialog.mode === "rename"
          ? api.renameChat(chatDialog.chatId, name)
          : api.setChatIcon(chatDialog.chatId, emoji ?? "");
    request
      .then(async (chat) => {
        if (emoji) prefs.recordEmoji(emoji);
        closeChatDialog();
        await refreshChats();
        if (chatDialog.mode === "create") {
          await select(chat.id);
          focusComposer();
        }
      })
      .catch((reason: unknown) => setDialogError(errorCode(reason)));
  };

  const chatAction = (id: string, action: ChatAction) => {
    if (action === "rename" || action === "icon") {
      setChatDialog({ mode: action, chatId: id });
      return;
    }
    const request =
      action === "pin" || action === "unpin"
        ? api.setChatPinned(id, action === "pin")
        : api.setChatArchived(id, action === "archive");
    request
      .then(async () => {
        await refreshChats();
        // An archived chat is closed: Inbox stays open.
        if (action === "archive" && selected === id) await select(INBOX_ID);
      })
      .catch((reason: unknown) => setComposerError(errorCode(reason)));
  };

  const openLink = (url: string) => {
    api.openExternalUrl(url).catch((reason: unknown) => setComposerError(errorCode(reason)));
  };

  if (feed.loadError) return <p role="alert">{`${t("error.loadFailed")}: ${t(`error.${feed.loadError}`)}`}</p>;

  const dialogChat = chatDialog && chatDialog.mode !== "create" ? list.chats.find((c) => c.id === chatDialog.chatId) : undefined;
  const dateTarget = feed.dateTarget;

  return (
    <MainWindowView
      interactive
      showSearch={false}
      search={sidebarSearch}
      searchResults={
        search.open ? (
          <SearchResults
            listId={search.listId}
            query={search.query}
            chats={search.results.chats}
            hits={search.results.hits}
            hasMore={search.results.hasMore}
            loading={search.results.loading}
            searched={search.results.searched}
            error={search.results.error}
            today={feed.today}
            language={language}
            filters={search.filters}
            onFiltersChange={search.setFilters}
            canScopeChat={selected !== null}
            onPickDate={setDatePick}
            activeIndex={search.results.activeIndex}
            onActiveChange={search.results.setActiveIndex}
            onOpenChat={search.openChat}
            onOpenHit={search.openHit}
            onLoadMore={search.loadMore}
          />
        ) : undefined
      }
      onOpenImport={importer.start}
      // The version was not received: the window opens anyway, with a dash instead of the version (the window has no separate channel to the log).
      onOpenAbout={() => void api.appInfo().then((info) => setAbout(info.title), () => setAbout(t("about.unknown")))}
      composerButtons={{ attach: true, emoji: true }}
      emoji={{ recent: settings.recentEmoji, language, onPicked: prefs.recordEmoji }}
      chats={sidebarChats}
      moveTargets={moveTargets}
      drag={{ onDragStart: drag.begin, dropTargetId: drag.overChatId, ghost: drag.ghost }}
      selectedChatId={selected}
      onSelectChat={(id) => void select(id)}
      sidebar={{
        collapsed: settings.sidebarCollapsed,
        onToggleCollapsed: prefs.toggleCollapsed,
        onCreateChat: () => setChatDialog({ mode: "create" }),
        onChatAction: chatAction,
        trashCount: list.trashCount,
        onOpenTrash: () => void select(null),
      }}
      messages={feed.messages}
      today={feed.today}
      feedUi={feed.ui}
      feedActions={feed.actions}
      feedPaging={feed.paging}
      feedScrollTo={feed.scrollTo}
      feedToBottom={toBottom}
      feedLoading={selected === INBOX_ID && tag === null && !(feed.loaded && onThisDay.ready)}
      forest={forest.pane}
      onFeedTopDay={forest.onTopDay}
      forestMenu={{ hidden: forest.hidden, onToggle: forest.toggleHidden }}
      // The card sits at the end of the live feed, not in the middle of the history (after a jump to an entry from search).
      onThisDay={
        onThisDay.card !== null && !feed.paging.hasNewer
          ? {
              journalDate: onThisDay.card.today,
              node: (
                <OnThisDayCard
                  card={onThisDay.card}
                  onOpen={(messageId) => openEntry(INBOX_ID, messageId)}
                  onReply={startReply}
                  onDismiss={hideCard}
                />
              ),
            }
          : undefined
      }
      onOpenReply={(quote) => {
        if (quote.chatId !== null) openEntry(quote.chatId, quote.id);
      }}
      replyStrip={reply ? <ReplyStrip quote={reply} onCancel={() => changeReply(null)} /> : undefined}
      onFeedPointerDown={feed.clearMark}
      tagFilter={tag === null ? undefined : { tag, onClear: () => setTag(null) }}
      trash={{
        items: trash.items,
        today: feed.today,
        busyId: trash.busyId,
        error: trash.error ? t(`error.${trash.error}`) : undefined,
        onRestore: trash.restore,
        onPurge: trash.purge,
      }}
      draft={draft}
      onDraftChange={(value) => {
        if (sending.current) return;
        typeDraft(value);
        setComposerError(null);
      }}
      onSubmit={submit}
      composerError={composerError ? t(`error.${composerError}`) : undefined}
      attachments={attachments.controls}
      onOpenLink={openLink}
      onOpenSettings={() => void api.openSettings().catch(() => undefined)}
      overlay={
        <>
          {chatDialog ? (
            <ChatDialog
              mode={chatDialog.mode}
              initialName={dialogChat?.title}
              initialEmoji={dialogChat?.icon ?? null}
              recentEmoji={settings.recentEmoji}
              language={language}
              error={dialogError ? t(`error.${dialogError}`) : undefined}
              onSubmit={submitChatDialog}
              onClose={closeChatDialog}
            />
          ) : null}
          {importer.dialog ? <ImportDialog {...importer.dialog} /> : null}
          {attachments.lightbox ? (
            <Lightbox
              images={attachments.lightbox.images}
              index={attachments.lightbox.index}
              onIndexChange={attachments.setLightboxIndex}
              onClose={attachments.closeLightbox}
              onOpenExternal={attachments.openFile}
            />
          ) : null}
          {about !== null ? <AboutDialog title={about} onClose={() => setAbout(null)} /> : null}
          {datePick ? (
            <CalendarDialog
              value={search.filters[datePick] ?? feed.today}
              today={feed.today}
              language={language}
              title={t(datePick === "from" ? "search.fromTitle" : "search.toTitle")}
              onPick={(date) => {
                search.setFilters({ ...search.filters, [datePick]: date });
                setDatePick(null);
              }}
              onClose={() => setDatePick(null)}
            />
          ) : null}
          {dateTarget ? (
            <CalendarDialog
              value={dateTarget.journalDate}
              today={feed.today}
              language={language}
              error={dateError ? t(`error.${dateError}`) : undefined}
              onPick={(date) => {
                setDateError(null);
                void feed.pickDate(date).then(setDateError);
              }}
              onClose={() => {
                setDateError(null);
                feed.closeDate();
              }}
            />
          ) : null}
        </>
      }
    />
  );
}
