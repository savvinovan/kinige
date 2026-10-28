import { useCallback, useEffect, useRef, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { SearchHit } from "../../api/generated/SearchHit";
import type { SidebarSearch } from "../sidebar/Sidebar";
import type { FocusTarget } from "./useFeed";
import { useSearch } from "./useSearch";

interface UseMainSearchOptions {
  api: KinigeApi;
  /** Открытый чат (`null` — корзина). */
  selected: string | null;
  /** Открыть чат (черновик прежнего уходит в Rust до переключения). */
  select: (chatId: string | null) => Promise<void>;
  sidebarCollapsed: boolean;
  toggleCollapsed: () => void;
  /** Лента открывает чат на записи: страница вокруг неё и подсветка найденных слов (`useFeed`). */
  onFocus: (target: FocusTarget) => void;
}

/** Мак: там поиск по ⌘F, на других системах — по Ctrl+F. */
const isMac = () => typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

/**
 * Поиск в главном окне (слайс 4): поле сайдбара, результаты на месте ленты, ⌘F, значок поиска в свёрнутом сайдбаре и выбор
 * дат фильтра. Выбор результата открывает чат на записи (`focus` для ленты) и закрывает поиск.
 */
export function useMainSearch({ api, selected, select, sidebarCollapsed, toggleCollapsed, onFocus }: UseMainSearchOptions) {
  const input = useRef<HTMLInputElement>(null);
  const wantFocus = useRef(false);
  const counter = useRef(0);
  const [datePick, setDatePick] = useState<"from" | "to" | null>(null);

  // Очистка поля нужна обработчикам, которые создаются раньше самого поиска: она берётся из ссылки на момент вызова.
  const clearSearch = useRef<() => void>(() => undefined);
  const openHit = useCallback(
    (hit: SearchHit) => {
      const chatId = hit.message.chatId;
      void select(chatId).then(() => {
        counter.current += 1;
        onFocus({ chatId, messageId: hit.message.id, terms: hit.terms, nonce: counter.current });
        clearSearch.current();
      });
    },
    [select, onFocus],
  );
  const openChat = useCallback(
    (chat: ChatSummary) => {
      void select(chat.id).then(() => clearSearch.current());
    },
    [select],
  );
  const search = useSearch({ api, chatId: selected, onOpenHit: openHit, onOpenChat: openChat });
  const { clear } = search;
  useEffect(() => {
    clearSearch.current = clear;
  }, [clear]);

  const focusSearch = useCallback(() => {
    if (sidebarCollapsed) {
      // Поля в свёрнутом сайдбаре нет: сначала он разворачивается, фокус ставит эффект ниже.
      wantFocus.current = true;
      toggleCollapsed();
      return;
    }
    input.current?.focus();
    input.current?.select();
  }, [sidebarCollapsed, toggleCollapsed]);

  useEffect(() => {
    if (sidebarCollapsed || !wantFocus.current) return;
    wantFocus.current = false;
    input.current?.focus();
  }, [sidebarCollapsed]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const primary = isMac() ? event.metaKey : event.ctrlKey;
      if (primary && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        focusSearch();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [focusSearch]);

  const sidebarSearch: SidebarSearch = {
    value: search.query,
    onChange: search.setQuery,
    onKeyDown: search.onKeyDown,
    inputRef: input,
    resultsId: search.open ? search.listId : undefined,
    activeDescendant: search.activeDescendant,
    onRailClick: focusSearch,
  };

  return { search, sidebarSearch, datePick, setDatePick };
}
