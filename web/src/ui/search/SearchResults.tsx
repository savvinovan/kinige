import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Language } from "../../api/generated/Language";
import type { SearchHit } from "../../api/generated/SearchHit";
import { summaryOf, summaryText } from "../attachments/summaryText";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { IconButton } from "../controls/IconButton";
import { formatDayLabel } from "../feed/dates";
import { SegmentedChoice } from "../settings/SegmentedChoice";
import { StatusNote } from "../settings/StatusNote";
import { ChatTile } from "../sidebar/ChatTile";
import { HighlightedText } from "./HighlightedText";
import "./search.css";

/** Filters as the interface sees them. "This chat" becomes `chatId` in the request. */
export interface SearchUiFilters {
  scope: "all" | "chat";
  favorite: boolean;
  tasks: boolean;
  from: string | null;
  to: string | null;
}

export const NO_FILTERS: SearchUiFilters = { scope: "all", favorite: false, tasks: false, from: null, to: null };

/** The id of a result row for the search field's `aria-activedescendant`. */
export const searchRowId = (listId: string, index: number) => `${listId}-row-${index}`;

interface SearchResultsProps {
  /** The list id: the search field's `aria-controls` points at it. */
  listId: string;
  /** The query the results were obtained for (the "type a word" hint is shown when it is empty). */
  query: string;
  chats: readonly ChatSummary[];
  hits: readonly SearchHit[];
  hasMore: boolean;
  loading: boolean;
  /** The search for the query has already happened (otherwise "Nothing found" would flash before the answer). */
  searched: boolean;
  error?: ErrorCode | null;
  today: string;
  language: Language;
  filters: SearchUiFilters;
  onFiltersChange: (filters: SearchUiFilters) => void;
  /** "This chat" is available only when some chat is open (not the trash). */
  canScopeChat: boolean;
  /** Pick the filter date in the calendar. */
  onPickDate: (which: "from" | "to") => void;
  /** The row selected with the arrows (an index into the list "chats, then entries"). */
  activeIndex: number;
  onActiveChange: (index: number) => void;
  onOpenChat: (chat: ChatSummary) => void;
  onOpenHit: (hit: SearchHit) => void;
  onLoadMore: () => void;
}

/**
 * Search results in place of the feed (like the trash): filters on top, a "Chats" row with the matching names, below it entries:
 * the emoji and chat name, the day, up to three lines of a snippet with highlighted words. Newest entries on top. The list is a `listbox`,
 * the row selected with the arrows is marked `aria-selected`; focus stays in the search field. An empty result is a single quiet line,
 * an error is an error line. The next page loads when the end of the list appears in the scroll area; "Show
 * more" does the same from the keyboard.
 */
export function SearchResults(props: SearchResultsProps) {
  const { t } = useTranslation();
  const { listId, chats, hits, hasMore, loading, searched, error, today, language, filters, activeIndex } = props;
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const end = useRef<HTMLLIElement>(null);
  const { onLoadMore } = props;

  // The end of the list appeared in the scroll area: load the next page (where IntersectionObserver exists).
  useEffect(() => {
    const target = end.current;
    if (!target || !hasMore || loading || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) onLoadMore();
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasMore, loading, onLoadMore, hits.length]);

  // The row selected with the arrows stays in view (jsdom has no scrolling).
  useEffect(() => {
    document.getElementById(searchRowId(listId, activeIndex))?.scrollIntoView?.({ block: "nearest" });
  }, [listId, activeIndex]);

  // On the filter button the date is always full ("21 октября 2026"), not "Today": "from Today" would not read.
  const longDate = (value: string) => {
    const [year, month, day] = value.split("-").map(Number);
    return new Intl.DateTimeFormat(language, { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" }).format(
      new Date(Date.UTC(year ?? 1970, (month ?? 1) - 1, day ?? 1)),
    );
  };
  const dateLabel = (kind: "from" | "to", value: string | null) =>
    value === null ? t(`search.${kind}`) : t(kind === "from" ? "search.fromValue" : "search.toValue", { date: longDate(value) });

  const empty = searched && !loading && !error && chats.length === 0 && hits.length === 0;
  return (
    <section className="search" aria-label={t("search.label")}>
      <div className="search__column">
        <div className="search__filters" role="group" aria-label={t("search.filters")}>
          <SegmentedChoice
            label={t("search.filters")}
            value={filters.scope === "chat" && props.canScopeChat ? "chat" : "all"}
            options={[
              { value: "all", label: t("search.scopeAll") },
              ...(props.canScopeChat ? [{ value: "chat" as const, label: t("search.scopeChat") }] : []),
            ]}
            onChange={(scope) => props.onFiltersChange({ ...filters, scope })}
          />
          <Checkbox checked={filters.favorite} onChange={(e) => props.onFiltersChange({ ...filters, favorite: e.target.checked })}>
            {t("search.favorite")}
          </Checkbox>
          <Checkbox checked={filters.tasks} onChange={(e) => props.onFiltersChange({ ...filters, tasks: e.target.checked })}>
            {t("search.tasks")}
          </Checkbox>
          {(["from", "to"] as const).map((kind) => (
            <span className="search__date" key={kind}>
              <Button variant="secondary" title={t(kind === "from" ? "search.fromTitle" : "search.toTitle")} onClick={() => props.onPickDate(kind)}>
                {dateLabel(kind, filters[kind])}
              </Button>
              {filters[kind] !== null ? (
                <IconButton
                  small
                  icon="close"
                  label={t(kind === "from" ? "search.clearFrom" : "search.clearTo")}
                  onClick={() => props.onFiltersChange({ ...filters, [kind]: null })}
                />
              ) : null}
            </span>
          ))}
        </div>
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {t(`error.${error}`)}
          </StatusNote>
        ) : null}
        {props.query.trim() === "" ? <p className="search__empty">{t("search.hint")}</p> : null}
        {empty ? <p className="search__empty">{t("search.empty")}</p> : null}
        <ul className="search__list" role="listbox" id={listId} aria-label={t("search.label")}>
          {chats.length > 0 ? (
            <li role="presentation" className="search__heading">
              {t("search.chats")}
            </li>
          ) : null}
          {chats.map((chat, index) => (
            <li
              key={chat.id}
              role="option"
              id={searchRowId(listId, index)}
              aria-selected={activeIndex === index}
              className={`search-chat${activeIndex === index ? " search-row--active" : ""}`}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => props.onActiveChange(index)}
              onClick={() => props.onOpenChat(chat)}
            >
              <ChatTile chat={{ emoji: chat.icon ?? "", tint: chat.color }} name={chat.isInbox ? t("chat.inbox") : chat.title} />
              <span className="search-row__name">{chat.isInbox ? t("chat.inbox") : chat.title}</span>
              <span className="search-row__count">{t("count.entries", { count: chat.count })}</span>
            </li>
          ))}
          {hits.map((hit, position) => {
            const index = chats.length + position;
            const chat = hit.chatIsInbox && hit.chatTitle === "" ? t("chat.inbox") : hit.chatTitle;
            return (
              <li
                key={hit.message.id}
                role="option"
                id={searchRowId(listId, index)}
                aria-selected={activeIndex === index}
                className={`search-hit${activeIndex === index ? " search-row--active" : ""}`}
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => props.onActiveChange(index)}
                onClick={() => props.onOpenHit(hit)}
              >
                <div className="search-hit__head">
                  <span className="search-row__chat" title={chat}>
                    <ChatTile chat={{ emoji: hit.chatIcon ?? "", tint: hit.chatColor }} name={chat} />
                    <span className="search-row__name">{chat}</span>
                  </span>
                  <span className="search-hit__when">{formatDayLabel(hit.message.journalDate, today, language, labels)}</span>
                </div>
                <p className="search-hit__snippet">
                  {/* An entry with no text is described by what it holds, and the file name can be the found word (slice 7). */}
                  <HighlightedText
                    text={hit.snippet !== "" ? hit.snippet : (summaryText(summaryOf(hit.message.attachments), t) || hit.snippet)}
                    terms={hit.terms}
                  />
                </p>
              </li>
            );
          })}
          <li role="presentation" className="search__end" ref={end} />
        </ul>
        {loading ? <p className="search__empty">{t("search.loading")}</p> : null}
        {hasMore && !loading ? (
          <div className="search__more">
            <Button variant="secondary" onClick={onLoadMore}>
              {t("search.more")}
            </Button>
          </div>
        ) : null}
      </div>
    </section>
  );
}
