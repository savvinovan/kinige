import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { FeedCursor } from "../../api/generated/FeedCursor";
import type { SearchHit } from "../../api/generated/SearchHit";
import { inboxMatches } from "../search/inboxMatch";
import { NO_FILTERS, type SearchUiFilters, searchRowId } from "../search/SearchResults";

/** Пауза после последней клавиши до запроса (спецификация слайса 4, задача 4.13). */
export const SEARCH_DEBOUNCE_MS = 150;
/** Записей на странице результатов. */
const PAGE = 50;

interface UseSearchOptions {
  api: KinigeApi;
  /** Открытый чат: «Этот чат» ищет в нём; `null` — открыта корзина, такого фильтра нет. */
  chatId: string | null;
  /** Результат выбран (щелчок или Enter): открыть чат на записи. */
  onOpenHit: (hit: SearchHit, query: string) => void;
  onOpenChat: (chat: ChatSummary) => void;
  /** Inbox и его переведённое имя: Rust Inbox среди «Чатов» не отдаёт, строку добавляет интерфейс (задача 4.9). */
  inbox?: { chat: ChatSummary | undefined; name: string };
}

/**
 * Поиск: запрос из поля сайдбара с задержкой 150 мс, фильтры, страницы по курсору, выбор стрелками. Ответы, устаревшие к
 * моменту прихода (запрос или фильтры успели измениться), отбрасываются. Результаты показываются, пока в поле есть текст;
 * Esc очищает поле и возвращает ленту. Фокус остаётся в поле: выбранная строка передаётся диктору через
 * `aria-activedescendant`.
 */
export function useSearch({ api, chatId, onOpenHit, onOpenChat, inbox }: UseSearchOptions) {
  const listId = useId();
  // Inbox читается в момент прихода ответа: смена счётчика чата не должна перезапускать поиск.
  const inboxRef = useRef(inbox);
  useEffect(() => {
    inboxRef.current = inbox;
  });
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<SearchUiFilters>(NO_FILTERS);
  const [chats, setChats] = useState<ChatSummary[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [older, setOlder] = useState<FeedCursor | null>(null);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<ErrorCode | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const generation = useRef(0);
  const trimmed = query.trim();
  const open = trimmed !== "";
  const scopedChat = filters.scope === "chat" && chatId !== null ? chatId : null;
  const dateFrom = filters.from;
  const dateTo = filters.to;
  const { favorite, tasks } = filters;

  // Новый запрос или другие фильтры: состояние подстраивается прямо при отрисовке (так React советует «сбрасывать» состояние);
  // пустой запрос сбрасывает всё, иначе прежние результаты остаются на виду, пока не придёт ответ на новый запрос.
  const requestKey = JSON.stringify([trimmed, scopedChat, favorite, tasks, dateFrom, dateTo]);
  const [shownKey, setShownKey] = useState(requestKey);
  if (shownKey !== requestKey) {
    setShownKey(requestKey);
    setSearched(false);
    setLoading(trimmed !== "");
    setError(null);
    if (trimmed === "") {
      setChats([]);
      setHits([]);
      setOlder(null);
      setActiveIndex(0);
    }
  }

  // Первая страница с задержкой; ответы, устаревшие к приходу, отбрасываются (`generation`).
  useEffect(() => {
    generation.current += 1;
    const mine = generation.current;
    if (trimmed === "") return;
    const timer = setTimeout(() => {
      api
        .searchMessages(trimmed, { filters: { chatId: scopedChat, favorite, tasks, from: dateFrom, to: dateTo }, limit: PAGE })
        .then((page) => {
          if (generation.current !== mine) return;
          const home = inboxRef.current;
          const withInbox =
            home?.chat && !page.chats.some((c) => c.isInbox) && inboxMatches(home.name, trimmed) ? [home.chat, ...page.chats] : page.chats;
          setChats(withInbox);
          setHits(page.hits);
          setOlder(page.older);
          setError(null);
          setSearched(true);
          setActiveIndex(0);
        })
        .catch((reason: unknown) => {
          if (generation.current !== mine) return;
          setChats([]);
          setHits([]);
          setOlder(null);
          setError(errorCode(reason));
          setSearched(true);
        })
        .finally(() => {
          if (generation.current === mine) setLoading(false);
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [api, trimmed, scopedChat, favorite, tasks, dateFrom, dateTo]);

  const loadMore = useCallback(() => {
    if (older === null || loading || trimmed === "") return;
    const mine = generation.current;
    setLoading(true);
    api
      .searchMessages(trimmed, {
        filters: { chatId: scopedChat, favorite, tasks, from: dateFrom, to: dateTo },
        before: older,
        limit: PAGE,
      })
      .then((page) => {
        if (generation.current !== mine) return;
        setHits((shown) => [...shown, ...page.hits.filter((hit) => !shown.some((s) => s.message.id === hit.message.id))]);
        setOlder(page.older);
      })
      .catch((reason: unknown) => {
        if (generation.current === mine) setError(errorCode(reason));
      })
      .finally(() => {
        if (generation.current === mine) setLoading(false);
      });
  }, [api, older, loading, trimmed, scopedChat, favorite, tasks, dateFrom, dateTo]);

  const rows = chats.length + hits.length;
  const clear = useCallback(() => setQuery(""), []);

  const openActive = useCallback(
    (index: number) => {
      const chat = chats[index];
      if (chat) onOpenChat(chat);
      else {
        const hit = hits[index - chats.length];
        if (hit) onOpenHit(hit, trimmed);
      }
    },
    [chats, hits, onOpenChat, onOpenHit, trimmed],
  );

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape" && (open || query !== "")) {
      event.preventDefault();
      // Esc — человек сам уходит из поля: фокус отдаётся полю заметки, которое вернётся вместе с лентой.
      event.currentTarget.blur();
      clear();
      return;
    }
    if (!open) return;
    if (event.key === "ArrowDown" && rows > 0) {
      event.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, rows - 1));
    } else if (event.key === "ArrowUp" && rows > 0) {
      event.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (event.key === "Enter" && rows > 0) {
      event.preventDefault();
      openActive(Math.min(activeIndex, rows - 1));
    }
  };

  return {
    open,
    query,
    setQuery,
    clear,
    filters,
    setFilters,
    listId,
    results: { chats, hits, hasMore: older !== null, loading, searched, error, activeIndex, setActiveIndex },
    loadMore,
    onKeyDown,
    activeDescendant: open && rows > 0 ? searchRowId(listId, Math.min(activeIndex, rows - 1)) : undefined,
    openChat: onOpenChat,
    openHit: (hit: SearchHit) => onOpenHit(hit, trimmed),
  };
}
