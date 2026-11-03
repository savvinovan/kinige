import { useCallback, useEffect, useRef, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { SearchHit } from "../../api/generated/SearchHit";
import type { SidebarSearch } from "../sidebar/Sidebar";
import type { FocusTarget } from "./useFeed";
import { useSearch } from "./useSearch";

interface UseMainSearchOptions {
  api: KinigeApi;
  /** The open chat (`null` means the trash). */
  selected: string | null;
  /** Open a chat (the previous chat's draft goes to Rust before switching). */
  select: (chatId: string | null) => Promise<void>;
  sidebarCollapsed: boolean;
  toggleCollapsed: () => void;
  /** The feed opens the chat at an entry: the page around it and highlighting of the found words (`useFeed`). */
  onFocus: (target: FocusTarget) => void;
  /** Inbox and its translated name for the "Chats" row. */
  inbox: { chat: ChatSummary | undefined; name: string };
}

/** Mac: search is on Cmd+F there, on other systems on Ctrl+F. */
const isMac = () => typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

/**
 * Search in the main window (slice 4): the sidebar field, results in place of the feed, Cmd+F, the search icon in the
 * collapsed sidebar and the filter date picking. Picking a result opens the chat at the entry (`focus` for the feed) and closes search.
 */
export function useMainSearch({ api, selected, select, sidebarCollapsed, toggleCollapsed, onFocus, inbox }: UseMainSearchOptions) {
  const input = useRef<HTMLInputElement>(null);
  const wantFocus = useRef(false);
  const counter = useRef(0);
  const [datePick, setDatePick] = useState<"from" | "to" | null>(null);

  // Clearing the field is needed by handlers created before the search itself: it is taken from the ref at call time.
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
  const search = useSearch({ api, chatId: selected, onOpenHit: openHit, onOpenChat: openChat, inbox });
  const { clear } = search;
  useEffect(() => {
    clearSearch.current = clear;
  }, [clear]);

  const focusSearch = useCallback(() => {
    if (sidebarCollapsed) {
      // There is no field in the collapsed sidebar: it expands first, the effect below sets focus.
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
