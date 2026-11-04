import { useEffect, useLayoutEffect, useMemo, useRef, type PointerEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { MoveTarget } from "../message/moveTargets";
import { type FeedMessage, buildFeed } from "./buildFeed";
import { formatDayLabel } from "./dates";
import { FeedRow } from "./FeedRow";
import { topDayOf } from "./topDay";
import "./feed.css";

/** Actions on feed entries; each receives the entry id. Without them bubbles behave as in D1 (buttons are drawn and disabled). */
export interface FeedActions {
  onEdit(id: string): void;
  onSaveEdit(id: string, text: string): void;
  onCancelEdit(id: string): void;
  onToggleTask(id: string, on: boolean): void;
  onToggleDone(id: string, done: boolean): void;
  onToggleFavorite(id: string, on: boolean): void;
  onChangeDate(id: string): void;
  onDelete(id: string): void;
  onRestore(id: string): void;
  onToggleTaskItem(id: string, index: number, checked: boolean): void;
  onTagClick(tag: string): void;
  /** "Move to chat" (the entry menu and dragging onto a chat). */
  onMoveTo(id: string, chatId: string): void;
}

/** Entry UI state: which one is being edited, which are deleted (they stay muted in place until the person leaves the chat). */
export interface FeedUiState {
  editingId?: string | null;
  deletedIds?: ReadonlySet<string>;
  /** Error saving an edit (already translated). */
  editError?: string;
  /** The entry opened from search: highlighted with a fill until the next click, the found words are marked (slice 4). */
  markedId?: string | null;
  markTerms?: readonly string[];
}

/** Loading feed pages (slice 4): without virtualization, on the signal "the list edge appeared in the scroll area". */
export interface FeedPaging {
  hasOlder: boolean;
  hasNewer: boolean;
  onLoadOlder: () => void;
  onLoadNewer: () => void;
}

/** A request to scroll the feed to an entry (a jump from search): a new `nonce` is a new request, even for the same entry. */
export interface ScrollRequest {
  id: string;
  nonce: number;
}

interface FeedProps {
  messages: readonly FeedMessage[];
  /** Today's `journal_date` (`YYYY-MM-DD`) for the "Today" and "Yesterday" labels: Rust sends it. In stories it is pinned so that snapshots are stable. */
  today: string;
  onRetry?: (messageId: string) => void;
  /** A click on a link in an entry -> the system browser. */
  onOpenLink?: (url: string) => void;
  ui?: FeedUiState;
  actions?: FeedActions;
  /** Empty-state text; by default the empty-chat hint. */
  emptyText?: string;
  /** Chats for the "Move to chat" item; the current one is marked (`useRecordDrag` and `moveTargetsFrom`). */
  moveTargets?: readonly MoveTarget[];
  /** Dragging a bubble onto a chat: `begin(id)` is the bubble's press handler. Without it bubbles cannot be dragged. */
  onDragStart?: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
  /** Loading older and newer pages. Without it the feed shows exactly what it received. */
  paging?: FeedPaging;
  /** Scroll to an entry (it ends up in the center of the area). */
  scrollTo?: ScrollRequest | null;
  /** A press in the feed (any): removes the highlight of the entry opened from search. */
  onPointerDownFeed?: () => void;
  /** The "On this day" card (slice 5): stands under the day label `journalDate` before its entries; with no entries that day, in its place by date. */
  card?: { journalDate: string; node: ReactNode };
  /** A click on a reply quote: open the old entry in context. Without a handler the quote is not clickable. */
  onOpenReply?: (quote: ReplyTo) => void;
  /** A new number means scroll the feed to the bottom edge (after the card is hidden the reader stays at the newest entries). */
  toBottom?: number;
  /** The feed has not yet received everything needed for the first frame: it draws nothing, not even the empty-chat hint (nothing jumps). */
  loading?: boolean;
  /**
   * Slice 6: the journal day the feed shows, reported when the list changes and while the person scrolls: the day at the top,
   * `null` for a list with no entries. A scroll the feed makes itself to an entry reports that entry's day once and then nothing
   * until the person scrolls again (the entry is centred, so the day at the top would be an earlier one); moving to the bottom
   * and opening another list (`listKey`) are reported, because the day at the top is then not what the board still shows.
   */
  onTopDay?: (journalDate: string | null) => void;
  /** Slice 6: what the list belongs to (the open chat). A new value starts listening to the person's position again. */
  listKey?: string;
  /**
   * Slice 7: opening an entry's photo (the lightbox, given the entry's attachments so that its other photos are known) and a file
   * card (the system app). Without it the photos and cards are drawn but do nothing.
   */
  onOpenImage?: (entryAttachments: readonly AttachmentView[], attachmentId: string) => void;
  onOpenFile?: (attachmentId: string) => void;
}

/**
 * The feed: day labels ("pills" in the center) and message bubbles pressed to the left. It opens scrolled to the bottom
 * (new at the bottom): the `column-reverse` scroll area keeps the bottom edge in place even when row heights
 * change after fonts load; a short feed lies at the bottom edge, above the composer. There is neither a title nor a chat name above the feed (§6).
 */
export function Feed({
  messages,
  today,
  onRetry,
  onOpenLink,
  ui,
  actions,
  emptyText,
  moveTargets,
  onDragStart,
  paging,
  scrollTo,
  onPointerDownFeed,
  card,
  onOpenReply,
  toBottom = 0,
  loading = false,
  onTopDay,
  listKey,
  onOpenImage,
  onOpenFile,
}: FeedProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const cardDate = card?.journalDate;
  const items = useMemo(() => buildFeed(messages, cardDate), [messages, cardDate]);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const scroller = useRef<HTMLDivElement>(null);
  const olderEdge = useRef<HTMLDivElement>(null);
  const newerEdge = useRef<HTMLDivElement>(null);
  // Height and scroll position before loading newer entries at the bottom: afterwards the scroll returns to the same entries.
  const beforeNewer = useRef<{ height: number; top: number } | null>(null);
  const paged = useRef(paging);
  useEffect(() => {
    paged.current = paging;
  });
  const hasOlder = paging?.hasOlder === true;
  const hasNewer = paging?.hasNewer === true;

  // The day at the top (slice 6). Reported while the person is in charge of the scroll position (`listening`); the feed's own
  // jumps to an entry switch it off until the next wheel, key, touch or press on the scroller (a scroll to the bottom keeps it on).
  const listening = useRef(true);
  const topDayHandler = useRef(onTopDay);
  const reportFrame = useRef(0);
  const loadingNow = useRef(loading);
  // A layout effect, declared before the ones that report, so that they see the current handler and loading state.
  useLayoutEffect(() => {
    topDayHandler.current = onTopDay;
    loadingNow.current = loading;
  });
  const reportTop = () => {
    const root = scroller.current;
    if (!root || !topDayHandler.current) return;
    const date = topDayOf(root);
    if (date) topDayHandler.current(date);
    // A list with no entries has no day: the board must not keep the one of the list before (a list still loading has none yet).
    else if (!loadingNow.current && !root.querySelector("[data-journal-date]")) topDayHandler.current(null);
  };
  const reportSoon = () => {
    if (reportFrame.current !== 0) return;
    reportFrame.current = requestAnimationFrame(() => {
      reportFrame.current = 0;
      reportTop();
    });
  };
  const listen = () => {
    listening.current = true;
  };
  useEffect(() => () => cancelAnimationFrame(reportFrame.current), []);

  // The list edge appeared in the scroll area: load the next page. The `column-reverse` area keeps the bottom edge in
  // place, so entries added at the TOP shift nothing; those added at the bottom do shift things, and the effect below fixes that.
  useEffect(() => {
    const root = scroller.current;
    if (!root || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          if (entry.target === olderEdge.current) paged.current?.onLoadOlder();
          else if (entry.target === newerEdge.current) {
            beforeNewer.current = { height: root.scrollHeight, top: root.scrollTop };
            paged.current?.onLoadNewer();
          }
        }
      },
      { root },
    );
    if (olderEdge.current) observer.observe(olderEdge.current);
    if (newerEdge.current) observer.observe(newerEdge.current);
    return () => observer.disconnect();
  }, [hasOlder, hasNewer, items.length]);

  useLayoutEffect(() => {
    const root = scroller.current;
    const saved = beforeNewer.current;
    if (!root || !saved) return;
    const added = root.scrollHeight - saved.height;
    if (added > 0) root.scrollTop = saved.top - added;
    beforeNewer.current = null;
  }, [messages]);

  // Another list (the person opened another chat): after a jump the feed stayed deaf until the next scroll, and the board kept
  // the day of the old chat. Declared before the effects that jump, so a jump that comes with the new list still wins.
  const listSeen = useRef(listKey);
  useLayoutEffect(() => {
    if (listSeen.current === listKey) return;
    listSeen.current = listKey;
    listening.current = true;
  }, [listKey]);

  // A jump from search: the entry in the center of the area. Instant, without smooth scrolling (quiet UI, Reduce Motion).
  // The `column-reverse` area: the bottom edge is `scrollTop = 0`. Scrolling happens on the parent's request, not on first show.
  const bottomSeen = useRef(toBottom);
  useLayoutEffect(() => {
    if (bottomSeen.current === toBottom) return;
    bottomSeen.current = toBottom;
    if (scroller.current) scroller.current.scrollTop = 0;
    // The bottom is no jump to a day: the day at the top is what the board has to follow.
    listening.current = true;
    reportTop();
  }, [toBottom]);

  // Once per request (`nonce`): the entry may not have reached the feed yet, in which case scrolling waits for it to appear, but once it finds the entry
  // it does not come back. Otherwise sending a reply, loading pages and the card appearing would each time pull the feed back.
  const scrollKey = scrollTo ? `${scrollTo.id}:${scrollTo.nonce}` : "";
  const scrolledKey = useRef("");
  useLayoutEffect(() => {
    if (!scrollTo || scrolledKey.current === scrollKey) return;
    // The feed is about to move itself: its top day is not the person's choice until they scroll again.
    listening.current = false;
    const plate = [...(scroller.current?.querySelectorAll<HTMLElement>("[data-message-id]") ?? [])].find(
      (element) => element.dataset.messageId === scrollTo.id,
    );
    if (!plate) return;
    scrolledKey.current = scrollKey;
    plate.scrollIntoView?.({ block: "center" });
    // The caption follows the feed after its own jump (a click on a day, a sent note, a restored entry): the day of the entry
    // it landed on, not the one at the top, which is an earlier day while the entry is centred.
    const landed = plate.dataset.journalDate;
    if (landed) topDayHandler.current?.(landed);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- on a new request or when the entry appears in the feed
  }, [scrollKey, items.length]);

  // Declared after the effects that move the feed itself, so a list that arrives with a jump is not reported at its old position.
  useLayoutEffect(() => {
    if (listening.current) reportTop();
  }, [items, loading]);

  return (
    // The scrollable area takes keyboard focus, otherwise the feed cannot be scrolled without a mouse.
    <div
      className="feed-scroll"
      tabIndex={0}
      role="region"
      aria-label={t("feed.label")}
      ref={scroller}
      onPointerDown={() => {
        listen();
        onPointerDownFeed?.();
      }}
      onWheel={listen}
      onTouchStart={listen}
      onKeyDown={listen}
      onScroll={() => {
        if (listening.current) reportSoon();
      }}
    >
      {/* In `column-reverse` the first one in the markup stands at the bottom: the bottom edge (new) goes before the list, the top one (old) after it. */}
      {hasNewer ? <div className="feed__edge" ref={newerEdge} aria-hidden="true" /> : null}
      {loading ? null : items.length === 0 ? (
        <p className="feed__empty">{emptyText ?? t("feed.empty")}</p>
      ) : (
        <ol className="feed">
          {items.map((item) =>
            item.kind === "day" ? (
              <li className="feed__item feed__item--day" key={item.key} data-journal-date={item.journalDate}>
                <h2 className="day-label">{formatDayLabel(item.journalDate, today, language, labels)}</h2>
              </li>
            ) : item.kind === "card" ? (
              <li className="feed__item feed__item--card" key={item.key}>
                {card?.node}
              </li>
            ) : (
              <FeedRow
                key={item.key}
                message={item.message}
                inSeries={item.inSeries}
                marked={ui?.markedId === item.message.id}
                markTerms={ui?.markedId === item.message.id ? ui?.markTerms : undefined}
                mode={ui?.editingId === item.message.id ? "editing" : ui?.deletedIds?.has(item.message.id) ? "deleted" : "view"}
                editError={ui?.editingId === item.message.id ? ui?.editError : undefined}
                onRetry={onRetry}
                onOpenLink={onOpenLink}
                onOpenReply={onOpenReply}
                onOpenImage={onOpenImage}
                onOpenFile={onOpenFile}
                actions={actions}
                moveTargets={moveTargets}
                onDragStart={onDragStart}
              />
            ),
          )}
        </ol>
      )}
      {hasOlder ? <div className="feed__edge" ref={olderEdge} aria-hidden="true" /> : null}
    </div>
  );
}
