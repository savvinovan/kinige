import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { INBOX_ID, type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ChatList } from "../../api/generated/ChatList";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Settings } from "../../api/generated/Settings";
import { useDraftSaver } from "../../draft/useDraftSaver";
import { useLanguage } from "../../i18n/I18n";
import { CalendarDialog } from "../calendar/CalendarDialog";
import { ChatDialog } from "../chat/ChatDialog";
import { useRecordDrag } from "../drag/useRecordDrag";
import { moveTargetsFrom } from "../message/moveTargets";
import { SettingsDialog } from "../settings/SettingsDialog";
import type { ChatAction, SidebarChat } from "../sidebar/Sidebar";
import { ImportDialog } from "../import/ImportDialog";
import { SearchResults } from "../search/SearchResults";
import { MainWindowView } from "./MainWindowView";
import { type FocusTarget, useFeed } from "./useFeed";
import { useImport } from "./useImport";
import { useMainSearch } from "./useMainSearch";
import { useSettingsState } from "./useSettingsState";
import { useTrash } from "./useTrash";

interface MainWindowProps {
  api: KinigeApi;
  /** Настройки, прочитанные до первого кадра (размер текста применяется без мигания). */
  initialSettings: Settings;
}

type ChatDialogState = { mode: "create" } | { mode: "rename" | "icon"; chatId: string };

const EMPTY_LIST: ChatList = { chats: [], trashCount: 0 };

/** Чат из ответа Rust → строка сайдбара. Чат без эмодзи получает нейтральный значок. */
function toSidebar(chat: ChatList["chats"][number]): SidebarChat {
  return {
    id: chat.id,
    name: chat.title,
    isInbox: chat.isInbox,
    emoji: chat.icon ?? "💬",
    count: chat.count,
    pinned: chat.pinnedAt !== null && chat.archivedAt === null,
    archived: chat.archivedAt !== null,
  };
}

/**
 * Главное окно на настоящем `KinigeApi`: чаты (выбор, создание, имя, эмодзи, закрепление, архив), лента страницей из Rust,
 * действия над записями, корзина, поле ввода с черновиком у каждого чата, настройки. Без оптимистичного показа (§5): запись
 * появляется после ответа Rust; при ошибке текст остаётся в поле, над ним — ошибка.
 */
export function MainWindow({ api, initialSettings }: MainWindowProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const [list, setList] = useState<ChatList>(EMPTY_LIST);
  /** Открытый чат; `null` — корзина. */
  const [selected, setSelected] = useState<string | null>(INBOX_ID);
  const [tag, setTag] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [composerError, setComposerError] = useState<ErrorCode | null>(null);
  const [chatDialog, setChatDialog] = useState<ChatDialogState | null>(null);
  const [dialogError, setDialogError] = useState<ErrorCode | null>(null);
  const [dateError, setDateError] = useState<ErrorCode | null>(null);
  const sending = useRef(false);
  // Пока человек не печатал, черновик из Rust подставляется в пустое поле; набранное до ответа не затирается.
  const typed = useRef(false);
  const chatId = selected ?? INBOX_ID;

  const saveDraft = useCallback((body: string) => api.saveDraft(chatId, body), [api, chatId]);
  const draftSaver = useDraftSaver(saveDraft);
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

  // Переход к записи из поиска: лента открывает чат на ней (страница вокруг, найденные слова выделены).
  const [focus, setFocus] = useState<FocusTarget | null>(null);
  const feed = useFeed({
    api,
    chatId: selected,
    tag,
    refreshChats,
    onError: setComposerError,
    onTag: setTag,
    focus,
  });
  const { reload: reloadFeed } = feed;
  const sidebarChats = useMemo(() => list.chats.map(toSidebar), [list.chats]);
  const moveTargets = useMemo(() => moveTargetsFrom(sidebarChats, selected, t), [sidebarChats, selected, t]);
  // Перетаскивание плашки на чат: бросок на любой чат, кроме того, где запись лежит.
  const drag = useRecordDrag({
    canDrop: (chatId) => chatId !== selected,
    onDrop: (messageId, chatId) => feed.actions.onMoveTo(messageId, chatId),
  });
  const trash = useTrash({ api, open: selected === null, refreshChats });
  const { clearError: clearTrashError } = trash;

  /** Открыть чат (или корзину при `null`). Черновик прежнего чата уходит в Rust до переключения. */
  const select = useCallback(
    async (next: string | null) => {
      await draftSaver.flush();
      // Поле нового чата пустое, пока не подгрузится его черновик (эффект ниже).
      typed.current = false;
      setDraft("");
      setSelected(next);
      setTag(null);
      setComposerError(null);
      clearTrashError();
    },
    [draftSaver, clearTrashError],
  );

  const { search, sidebarSearch, datePick, setDatePick } = useMainSearch({
    api,
    selected,
    select,
    sidebarCollapsed: settings.sidebarCollapsed,
    toggleCollapsed: prefs.toggleCollapsed,
    onFocus: setFocus,
  });
  const importer = useImport({
    api,
    onChanged: () => void Promise.all([refreshChats(), reloadFeed()]),
    // «Готово»: открывается «Избранное» (Inbox), куда легли записи.
    onDone: () => void select(INBOX_ID),
  });

  useEffect(() => {
    void refreshChats();
  }, [refreshChats]);

  // Черновик открытого чата: пока человек не печатал, подставляется в пустое поле.
  useEffect(() => {
    if (selected === null) return;
    let live = true;
    api
      .getDraft(selected)
      .then((saved) => {
        if (live && !typed.current && saved !== "") setDraft(saved);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [api, selected]);

  useEffect(() => {
    const offs: (() => void)[] = [];
    let closed = false;
    const keep = (off: () => void) => {
      if (closed) off();
      else offs.push(off);
    };
    // Запись пришла из окна захвата или из буфера; меню значка просит открыть чат.
    api
      .onEntriesChanged(() => void Promise.all([refreshChats(), reloadFeed()]))
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
  }, [api, refreshChats, reloadFeed, select]);

  // Окно скрывается или теряет фокус: черновик уходит в Rust сразу, не дожидаясь паузы в наборе (§5.1).
  const { flush } = draftSaver;
  useEffect(() => {
    const onHide = () => {
      if (document.visibilityState === "hidden") void flush();
    };
    const onBlur = () => void flush();
    document.addEventListener("visibilitychange", onHide);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("visibilitychange", onHide);
      window.removeEventListener("blur", onBlur);
    };
  }, [flush]);

  const submit = () => {
    if (sending.current || draft.trim() === "" || selected === null) return;
    sending.current = true;
    // Пока идёт отправка, ввод не принимается (см. `onDraftChange`), а запланированное сохранение черновика отменено, уже
    // отправленное — дожидаемся: Rust очищает черновик чата в той же транзакции, что и отправку, а запоздавшая запись
    // вернула бы отправленный текст.
    draftSaver.cancel();
    const text = draft;
    draftSaver
      .settle()
      .then(() => api.sendMessage(selected, text))
      .then((sent) => {
        draftSaver.cancel();
        feed.appendSent(sent);
        setDraft("");
        setComposerError(null);
        void refreshChats();
      })
      // Успех: запись просто появляется в ленте, без тоста и подтверждений (как в Telegram). Ошибка: текст остаётся в поле,
      // запись не показана (без оптимистичного показа); черновик возвращается в очередь сохранения.
      .catch((reason: unknown) => {
        setComposerError(errorCode(reason));
        draftSaver.schedule(text);
      })
      .finally(() => {
        sending.current = false;
      });
  };

  const focusComposer = () => {
    // Диалог возвращает фокус тому, кто его открыл, уже после закрытия: поле ввода берёт его следом.
    setTimeout(() => document.querySelector<HTMLTextAreaElement>("textarea.composer__input")?.focus(), 0);
  };

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
        // Архивный чат закрывается: открытым остаётся Inbox.
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
      composerButtons={{ emoji: true }}
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
        typed.current = true;
        setDraft(value);
        setComposerError(null);
        draftSaver.schedule(value);
      }}
      onSubmit={submit}
      composerError={composerError ? t(`error.${composerError}`) : undefined}
      onOpenLink={openLink}
      onOpenSettings={prefs.openDialog}
      overlay={
        <>
          {prefs.open ? (
            <SettingsDialog
              settings={settings}
              shortcuts={settings.shortcuts}
              onShortcutChange={prefs.changeShortcut}
              onShortcutReset={prefs.resetShortcut}
              onShortcutRecording={(active) => void api.shortcutRecording(active).catch(() => undefined)}
              shortcutErrors={prefs.shortcutErrors}
              choosing={prefs.choosing}
              actionError={prefs.pickError ? t(`error.${prefs.pickError}`) : undefined}
              onChooseFolder={prefs.chooseFolder}
              onTextSizeChange={prefs.changeTextSize}
              onClose={prefs.closeDialog}
            />
          ) : null}
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
          {datePick ? (
            <CalendarDialog
              value={search.filters[datePick] ?? feed.today}
              today={feed.today}
              language={language}
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
