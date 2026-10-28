import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Language } from "../../api/generated/Language";
import type { SearchHit } from "../../api/generated/SearchHit";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { IconButton } from "../controls/IconButton";
import { formatDayLabel } from "../feed/dates";
import { SegmentedChoice } from "../settings/SegmentedChoice";
import { StatusNote } from "../settings/StatusNote";
import { HighlightedText } from "./HighlightedText";
import "./search.css";

/** Фильтры, как их видит интерфейс. «Этот чат» превращается в `chatId` при запросе. */
export interface SearchUiFilters {
  scope: "all" | "chat";
  favorite: boolean;
  tasks: boolean;
  from: string | null;
  to: string | null;
}

export const NO_FILTERS: SearchUiFilters = { scope: "all", favorite: false, tasks: false, from: null, to: null };

/** Id строки результата для `aria-activedescendant` поля поиска. */
export const searchRowId = (listId: string, index: number) => `${listId}-row-${index}`;

interface SearchResultsProps {
  /** Id списка: на него указывает `aria-controls` поля поиска. */
  listId: string;
  /** Запрос, по которому получены результаты (подсказка «введите слово» показывается при пустом). */
  query: string;
  chats: readonly ChatSummary[];
  hits: readonly SearchHit[];
  hasMore: boolean;
  loading: boolean;
  /** Поиск по запросу уже состоялся (иначе «Ничего не найдено» мигало бы до ответа). */
  searched: boolean;
  error?: ErrorCode | null;
  today: string;
  language: Language;
  filters: SearchUiFilters;
  onFiltersChange: (filters: SearchUiFilters) => void;
  /** «Этот чат» доступен, только когда открыт какой-нибудь чат (не корзина). */
  canScopeChat: boolean;
  /** Выбрать дату фильтра в календаре. */
  onPickDate: (which: "from" | "to") => void;
  /** Строка, выбранная стрелками (индекс по списку «чаты, затем записи»). */
  activeIndex: number;
  onActiveChange: (index: number) => void;
  onOpenChat: (chat: ChatSummary) => void;
  onOpenHit: (hit: SearchHit) => void;
  onLoadMore: () => void;
}

/**
 * Результаты поиска на месте ленты (как корзина): фильтры сверху, строка «Чаты» с совпавшими названиями, ниже записи —
 * эмодзи и имя чата, день, до трёх строк фрагмента с подсвеченными словами. Новые записи сверху. Список — `listbox`,
 * выбранная стрелками строка отмечена `aria-selected`; фокус остаётся в поле поиска. Пустой результат — одна тихая строка,
 * ошибка — строка ошибки. Следующая страница подгружается, когда конец списка показался в области прокрутки; «Показать
 * ещё» — то же с клавиатуры.
 */
export function SearchResults(props: SearchResultsProps) {
  const { t } = useTranslation();
  const { listId, chats, hits, hasMore, loading, searched, error, today, language, filters, activeIndex } = props;
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const end = useRef<HTMLLIElement>(null);
  const { onLoadMore } = props;

  // Конец списка показался в области прокрутки: грузим следующую страницу (там, где есть IntersectionObserver).
  useEffect(() => {
    const target = end.current;
    if (!target || !hasMore || loading || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) onLoadMore();
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasMore, loading, onLoadMore, hits.length]);

  // Выбранная стрелками строка остаётся на виду (в jsdom прокрутки нет).
  useEffect(() => {
    document.getElementById(searchRowId(listId, activeIndex))?.scrollIntoView?.({ block: "nearest" });
  }, [listId, activeIndex]);

  // На кнопке фильтра дата всегда полная («21 октября 2026»), а не «Сегодня»: «с Сегодня» не прочитать.
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
              <span className="search-row__emoji" aria-hidden="true">
                {chat.icon ?? "\u{1F4AC}"}
              </span>
              <span className="search-row__name">{chat.title}</span>
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
                    <span className="search-row__emoji" aria-hidden="true">
                      {hit.chatIcon ?? "\u{1F4AC}"}
                    </span>
                    <span className="search-row__name">{chat}</span>
                  </span>
                  <span className="search-hit__when">{formatDayLabel(hit.message.journalDate, today, language, labels)}</span>
                </div>
                <p className="search-hit__snippet">
                  <HighlightedText text={hit.snippet} terms={hit.terms} />
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
