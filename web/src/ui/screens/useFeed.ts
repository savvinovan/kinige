import { useCallback, useEffect, useRef, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessagePage } from "../../api/generated/MessagePage";
import type { MessageView } from "../../api/generated/MessageView";
import type { FeedCursor } from "../../api/generated/FeedCursor";
import type { FeedMessage } from "../feed/buildFeed";
import type { FeedPaging, FeedUiState, ScrollRequest } from "../feed/Feed";
import { mergeLatestPage, mergeOlder, withDeleted } from "../feed/mergeFeed";
import { useFeedActions } from "./useFeedActions";

/** How many entries a feed page loads. Older pages are loaded when scrolling to the top (slice 4), without virtualization. */
export const PAGE_SIZE = 200;

/** A jump to an entry (from search): the chat, the entry and the words to highlight. A new `nonce` is a new jump. */
export interface FocusTarget {
  chatId: string;
  messageId: string;
  terms: readonly string[];
  nonce: number;
}

interface UseFeedOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash is open, there is no feed. */
  chatId: string | null;
  /** The tag filter (lowercase, without `#`). */
  tag: string | null;
  /** Chat and trash counters after an entry changes. */
  refreshChats: () => Promise<void>;
  /** An action error that has no place inside the plate (shown above the composer). */
  onError: (code: ErrorCode) => void;
  /** A click on a `#tag`. */
  onTag: (tag: string) => void;
  /** Open the chat at an entry: the page around it, then loading up and down. Used once per `nonce`. */
  focus?: FocusTarget | null;
  /** The entries of the journal changed through this feed (sent, edited, deleted, restored, moved, re-dated): the forest reloads. */
  onMutated?: () => void;
}

/**
 * The open chat's feed and the actions on its entries (slice 3): editing in the plate, deletion, flags, day, `- [ ]` items.
 * No optimistic display: an entry changes after Rust replies. A deleted entry stays in place, dimmed ("Deleted ·
 * Restore"), until the person leaves the chat (user decision 1).
 */
export function useFeed({ api, chatId, tag, refreshChats, onError, onTag, focus = null, onMutated }: UseFeedOptions) {
  const mutatedRef = useRef(onMutated);
  useEffect(() => {
    mutatedRef.current = onMutated;
  });
  const mutated = useCallback(() => mutatedRef.current?.(), []);
  const [messages, setMessages] = useState<FeedMessage[]>([]);
  const [today, setToday] = useState("");
  // The first page of the open chat is received: until then the feed may draw nothing (the first frame with the "On this day" card).
  const [loaded, setLoaded] = useState(false);
  // The edges of the loaded feed: the cursor of the next older page and of the next newer one; `newer === null` means the feed reached the end.
  const [older, setOlder] = useState<FeedCursor | null>(null);
  const [newer, setNewer] = useState<FeedCursor | null>(null);
  const [mark, setMark] = useState<{ id: string; terms: readonly string[] } | null>(null);
  const [scrollTo, setScrollTo] = useState<ScrollRequest | null>(null);
  // The jump is already done: the same `nonce` a second time (returning to the chat) does not take the feed to the entry.
  const usedFocus = useRef(0);
  // Cursors for the edge handlers and the Rust event. A new cursor lands here at the same moment as in the state (`setCursor`):
  // React cleans up effects with a delay, so an edge that showed up right after a page render would load by the old cursor,
  // and the "entries changed" event would take the middle of the history for the end. On a chat switch the effect below resets the cursors.
  const paging = useRef<{ older: FeedCursor | null; newer: FeedCursor | null }>({ older: null, newer: null });
  const setCursor = useCallback((edge: "older" | "newer", cursor: FeedCursor | null) => {
    paging.current = { ...paging.current, [edge]: cursor };
    (edge === "older" ? setOlder : setNewer)(cursor);
  }, []);
  const loadingEdge = useRef(false);
  const [loadError, setLoadError] = useState<ErrorCode | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editError, setEditError] = useState<string | undefined>(undefined);
  const [deletedIds, setDeletedIds] = useState<ReadonlySet<string>>(new Set());
  const [dateFor, setDateFor] = useState<string | null>(null);
  const deleted = useRef(deletedIds);
  // The latest values for handlers that live longer than one render (Rust replies, events, sending an entry).
  const current = useRef({ chatId, tag, loaded, messages, onError });
  const shownToday = useRef(today);
  useEffect(() => {
    deleted.current = deletedIds;
    current.current = { chatId, tag, loaded, messages, onError };
    shownToday.current = today;
  });

  // A chat or filter switch: the plate state is reset right during rendering (this is how React advises to "adjust state").
  // Deleted entries ("Deleted · Restore") live until the person leaves the chat; changing the tag filter does not change the chat.
  const chatKey = chatId ?? "";
  const feedKey = `${chatKey}\u0000${tag ?? ""}`;
  const [shownChat, setShownChat] = useState(chatKey);
  const [shownKey, setShownKey] = useState(feedKey);
  if (shownKey !== feedKey) {
    setShownKey(feedKey);
    setEditingId(null);
    setEditError(undefined);
    setDateFor(null);
    if (shownChat !== chatKey) {
      setShownChat(chatKey);
      setLoaded(false);
      setDeletedIds(new Set());
      setMessages([]);
      setOlder(null);
      setNewer(null);
      setMark(null);
      // The request to scroll to an entry belonged to the previous chat: on return the feed opens at the bottom, not at the old entry.
      setScrollTo(null);
    } else {
      // The same chat, another filter: only the deleted entries that match the new filter stay in place.
      setMessages((shown) => withDeleted(shown, [], deletedIds, tag));
    }
  }

  // Another chat: the previous chat's cursors are no longer valid (the state is reset above). Declared before the feed
  // loading so as not to wipe the cursors it brings.
  useEffect(() => {
    paging.current = { older: null, newer: null };
  }, [chatKey]);

  /**
   * The jump to an entry failed (it is not in place: in the trash, deleted): the loaded copies of entries are stale, first of
   * all the reply quotes (they still have `state: "live"`). Fresh copies take the place of the loaded ones; nothing is added or removed.
   * The window: the last page, and in the middle of the history the page around the middle of what is loaded (scrolling does not jump).
   */
  const refreshLoaded = useCallback(async () => {
    const { chatId: id, tag: filter, messages: shown } = current.current;
    if (id === null) return;
    const atEdge = paging.current.newer === null;
    const middle = shown.at(Math.floor(shown.length / 2));
    if (!atEdge && !middle) return;
    try {
      const page = await api.listMessages(id, {
        limit: PAGE_SIZE,
        tag: filter ?? undefined,
        around: atEdge ? undefined : middle?.id,
      });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => mergeOlder(loaded, page.messages));
    } catch {
      // The quotes will refresh on the next feed read.
    }
  }, [api]);

  // A chat switch, a filter change or a jump to an entry: the feed is reread. A jump is the page around the entry; the
  // found words are highlighted in it, the entry stays highlighted until the next click (`clearMark`). A failed jump is
  // not a load error: the feed stays, the error goes to `onError`; in a chat where nothing is loaded yet, its last page is shown.
  const focusNonce = focus?.nonce ?? 0;
  useEffect(() => {
    if (chatId === null) return;
    let live = true;
    const target = focus && focus.chatId === chatId && focus.nonce > usedFocus.current ? focus : null;
    if (target) usedFocus.current = target.nonce;
    const show = (page: MessagePage, to: FocusTarget | null) => {
      if (!live) return;
      setMessages((shown) => withDeleted(shown, page.messages, deleted.current, tag));
      setCursor("older", page.older);
      setCursor("newer", page.newer);
      setToday(page.today);
      setLoaded(true);
      setLoadError(null);
      if (to) {
        setMark({ id: to.messageId, terms: to.terms });
        setScrollTo({ id: to.messageId, nonce: to.nonce });
      }
    };
    const failed = (reason: unknown) => live && setLoadError(errorCode(reason));
    const options = { limit: PAGE_SIZE, tag: tag ?? undefined };
    api
      .listMessages(chatId, { ...options, around: target?.messageId })
      .then((page) => show(page, target))
      .catch((reason: unknown) => {
        if (!live) return;
        if (!target) return failed(reason);
        current.current.onError(errorCode(reason));
        if (current.current.loaded) {
          void refreshLoaded();
          return;
        }
        api.listMessages(chatId, options).then((page) => show(page, null)).catch(failed);
      });
    return () => {
      live = false;
    };
    // `focus` is read once per `nonce`: the object itself is not needed as a dependency; `setCursor` and `refreshLoaded` are stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, chatId, tag, focusNonce]);

  /** An older page at the top: the `older` cursor, the entries go before the loaded ones. Scrolling does not jump. */
  const loadOlder = useCallback(async () => {
    const { older: cursor } = paging.current;
    const { chatId: id, tag: filter } = current.current;
    if (cursor === null || id === null || loadingEdge.current) return;
    loadingEdge.current = true;
    try {
      const page = await api.listMessages(id, { before: cursor, limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => mergeOlder(loaded, page.messages));
      setCursor("older", page.older);
    } catch (reason) {
      onError(errorCode(reason));
    } finally {
      loadingEdge.current = false;
    }
  }, [api, onError, setCursor]);

  /** A newer page at the bottom (only after a jump to an entry from the middle of the history); on reaching the end, the feed is "live" again. */
  const loadNewer = useCallback(async () => {
    const { newer: cursor } = paging.current;
    const { chatId: id, tag: filter } = current.current;
    if (cursor === null || id === null || loadingEdge.current) return;
    loadingEdge.current = true;
    try {
      const page = await api.listMessages(id, { after: cursor, limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => mergeOlder(loaded, page.messages));
      setCursor("newer", page.newer);
    } catch (reason) {
      onError(errorCode(reason));
    } finally {
      loadingEdge.current = false;
    }
  }, [api, onError, setCursor]);

  // A fresh last page: an entry came from the capture window or from the clipboard (the Rust "entries changed" event). The
  // loaded history is not reset; deleted entries that still stand in place do not vanish with the page.
  const reload = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    // The feed is open in the middle of the history (a jump from search): new entries at the bottom will appear when loading reaches them.
    if (paging.current.newer !== null) return;
    try {
      const page = await api.listMessages(id, { limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => {
        const merged = mergeLatestPage(loaded, page.messages, page.older !== null);
        return withDeleted(loaded, merged, deleted.current, filter);
      });
      // The loaded history stays; if there is nothing older than the page, there is no cursor any more.
      if (page.older === null) setCursor("older", null);
      setToday(page.today);
    } catch {
      // The feed stays as it was: the next event will reread it.
    }
  }, [api, setCursor]);

  /**
   * A new journal day began (after 04:00) while the window was open: the feed is reread, the "Today" and "Yesterday" labels
   * come out right. Called on window focus; the day is asked from Rust with the cheapest request (one entry), regardless of
   * whether there is an "On this day" card.
   */
  const checkDay = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    try {
      const page = await api.listMessages(id, { limit: 1, tag: filter ?? undefined });
      if (shownToday.current !== "" && page.today !== shownToday.current) await reload();
    } catch {
      // The next focus will check again.
    }
  }, [api, reload]);

  /**
   * The feed from scratch: the last page replaces everything loaded, both cursors are recomputed. Needed when entries
   * landed in the middle of the already loaded history (import): the "older" cursor of the previous feed would point past
   * the new entries, and the pages between them would stay unloaded. Deleted plates that have not left the chat stay in place.
   */
  const restart = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    try {
      const page = await api.listMessages(id, { limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((shown) => withDeleted(shown, page.messages, deleted.current, filter));
      setCursor("older", page.older);
      setCursor("newer", page.newer);
      setToday(page.today);
      setMark(null);
    } catch {
      // The feed stays as it was: the next event or "Done" will reread it.
    }
  }, [api, setCursor]);

  const replace = useCallback(
    (view: MessageView) => {
      setMessages((list) => list.map((m) => (m.id === view.id ? view : m)));
      mutated();
    },
    [mutated],
  );

  /**
   * A sent entry goes to the end of the feed. Called from the reply to a send, which could arrive after a chat or filter
   * switch, so the chat and tag are read from `current`, not from the closure. An entry of another chat does not get into
   * the feed; under a tag filter it is visible only if it has that tag; a feed in the middle of the history (after a jump
   * to an entry) is reopened at the live edge, otherwise the entry would land right after the old ones.
   */
  const appendSent = useCallback(
    (sent: MessageView) => {
      mutated();
      const { chatId: id, tag: filter } = current.current;
      if (sent.chatId !== id) return;
      if (filter !== null && !sent.tags.includes(filter)) return;
      if (paging.current.newer !== null) {
        // The feed goes to the live edge, the sent entry to the center of the frame, as after a jump to an entry.
        void restart().then(() => setScrollTo({ id: sent.id, nonce: 0 }));
        return;
      }
      setMessages((all) => [...all, sent]);
    },
    [restart, mutated],
  );

  const fail = useCallback((reason: unknown) => onError(errorCode(reason)), [onError]);

  const actions = useFeedActions({
    api,
    refreshChats,
    replace,
    fail,
    reload,
    onTag,
    mutated,
    setMessages,
    setEditingId,
    setEditError,
    setDeletedIds,
    setDateFor,
  });

  /** The calendar picked a day for the entry `dateFor`; the reply is an error code (the dialog shows it), `null` means done. */
  const pickDate = useCallback(
    async (journalDate: string): Promise<ErrorCode | null> => {
      if (dateFor === null) return null;
      try {
        replace(await api.setMessageDate(dateFor, journalDate));
        setDateFor(null);
        return null;
      } catch (reason) {
        return errorCode(reason);
      }
    },
    [api, dateFor, replace],
  );

  const ui: FeedUiState = { editingId, deletedIds, editError, markedId: mark?.id ?? null, markTerms: mark?.terms };
  const feedPaging: FeedPaging = { hasOlder: older !== null, hasNewer: newer !== null, onLoadOlder: loadOlder, onLoadNewer: loadNewer };
  const dateTarget = dateFor === null ? undefined : messages.find((m) => m.id === dateFor);
  return {
    messages,
    today,
    loaded,
    loadError,
    ui,
    actions,
    paging: feedPaging,
    scrollTo,
    clearMark: useCallback(() => setMark(null), []),
    reload,
    checkDay,
    restart,
    appendSent,
    dateTarget,
    closeDate: () => setDateFor(null),
    pickDate,
  };
}
