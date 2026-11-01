import { useEffect, useLayoutEffect, useMemo, useRef, type PointerEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { Message } from "../message/Message";
import type { MoveTarget } from "../message/moveTargets";
import { ReplyQuote } from "../reply/ReplyQuote";
import { type FeedMessage, buildFeed } from "./buildFeed";
import { formatDayLabel } from "./dates";
import "./feed.css";

/** Действия над записями ленты; каждое получает id записи. Без них плашки ведут себя как в D1 (кнопки нарисованы и недоступны). */
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
  /** «Переместить в чат» (меню записи и перетаскивание на чат). */
  onMoveTo(id: string, chatId: string): void;
}

/** Состояние интерфейса записей: какая правится, какие удалены (стоят приглушёнными на месте до ухода из чата). */
export interface FeedUiState {
  editingId?: string | null;
  deletedIds?: ReadonlySet<string>;
  /** Ошибка сохранения правки (уже переведённая). */
  editError?: string;
  /** Запись, открытая из поиска: подсвечена заливкой до следующего щелчка, найденные слова выделены (слайс 4). */
  markedId?: string | null;
  markTerms?: readonly string[];
}

/** Подгрузка страниц ленты (слайс 4): без виртуализации, по сигналу «край списка показался в области прокрутки». */
export interface FeedPaging {
  hasOlder: boolean;
  hasNewer: boolean;
  onLoadOlder: () => void;
  onLoadNewer: () => void;
}

/** Просьба прокрутить ленту к записи (переход из поиска): новый `nonce` — новая просьба, даже к той же записи. */
export interface ScrollRequest {
  id: string;
  nonce: number;
}

interface FeedProps {
  messages: readonly FeedMessage[];
  /** Сегодняшний `journal_date` (`YYYY-MM-DD`) для меток «Сегодня» и «Вчера»: его присылает Rust. В историях фиксируется, чтобы снимки были стабильны. */
  today: string;
  onRetry?: (messageId: string) => void;
  /** Щелчок по ссылке в записи → системный браузер. */
  onOpenLink?: (url: string) => void;
  ui?: FeedUiState;
  actions?: FeedActions;
  /** Текст пустого состояния; по умолчанию — подсказка пустого чата. */
  emptyText?: string;
  /** Чаты для пункта «Переместить в чат»; текущий помечен (`useRecordDrag` и `moveTargetsFrom`). */
  moveTargets?: readonly MoveTarget[];
  /** Перетаскивание плашки на чат: `begin(id)` — обработчик нажатия плашки. Без него плашки не тащатся. */
  onDragStart?: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
  /** Подгрузка старых и новых страниц. Без неё лента показывает ровно то, что получила. */
  paging?: FeedPaging;
  /** Прокрутить к записи (она по центру области). */
  scrollTo?: ScrollRequest | null;
  /** Нажатие в ленте (любое): снимает подсветку записи, открытой из поиска. */
  onPointerDownFeed?: () => void;
  /** Карточка «В этот день» (слайс 5): стоит под меткой дня `journalDate` перед его записями; нет записей этого дня — на своём месте по дате. */
  card?: { journalDate: string; node: ReactNode };
  /** Щелчок по цитате ответа: открыть старую запись в контексте. Без обработчика цитата не нажимается. */
  onOpenReply?: (quote: ReplyTo) => void;
  /** Новый номер — прокрутить ленту к нижнему краю (после скрытия карточки читатель остаётся у самых новых записей). */
  toBottom?: number;
  /** Лента ещё не получила всё нужное для первого кадра: не рисует ничего, даже подсказку пустого чата (ничего не прыгает). */
  loading?: boolean;
}

/**
 * Лента: метки дней («таблетки» по центру) и плашки сообщений, прижатые влево. Открывается прокрученной к низу
 * (новое внизу): область прокрутки `column-reverse` держит нижний край на месте, даже когда после загрузки шрифтов
 * меняется высота строк; короткая лента лежит у нижнего края, над composer. Над лентой нет ни заголовка, ни названия чата (§6).
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
}: FeedProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const cardDate = card?.journalDate;
  const items = useMemo(() => buildFeed(messages, cardDate), [messages, cardDate]);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };
  const scroller = useRef<HTMLDivElement>(null);
  const olderEdge = useRef<HTMLDivElement>(null);
  const newerEdge = useRef<HTMLDivElement>(null);
  // Высота и положение прокрутки перед подгрузкой новых записей внизу: после неё прокрутка возвращается к тем же записям.
  const beforeNewer = useRef<{ height: number; top: number } | null>(null);
  const paged = useRef(paging);
  useEffect(() => {
    paged.current = paging;
  });
  const hasOlder = paging?.hasOlder === true;
  const hasNewer = paging?.hasNewer === true;

  // Край списка показался в области прокрутки: грузим следующую страницу. Область `column-reverse` держит нижний край на
  // месте, поэтому записи, добавленные СВЕРХУ, ничего не сдвигают; добавленные снизу сдвигают — это исправляет эффект ниже.
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

  // Переход из поиска: запись по центру области. Мгновенно, без плавной прокрутки (тихий интерфейс, Reduce Motion).
  // Область `column-reverse`: нижний край — `scrollTop = 0`. Прокрутка по просьбе родителя, не при первом показе.
  const bottomSeen = useRef(toBottom);
  useLayoutEffect(() => {
    if (bottomSeen.current === toBottom) return;
    bottomSeen.current = toBottom;
    if (scroller.current) scroller.current.scrollTop = 0;
  }, [toBottom]);

  // Один раз на просьбу (`nonce`): запись могла ещё не попасть в ленту, тогда прокрутка ждёт её появления, но, найдя запись,
  // больше не возвращается. Иначе отправка ответа, подгрузка страниц и появление карточки каждый раз уводили бы ленту назад.
  const scrollKey = scrollTo ? `${scrollTo.id}:${scrollTo.nonce}` : "";
  const scrolledKey = useRef("");
  useLayoutEffect(() => {
    if (!scrollTo || scrolledKey.current === scrollKey) return;
    const plate = [...(scroller.current?.querySelectorAll<HTMLElement>("[data-message-id]") ?? [])].find(
      (element) => element.dataset.messageId === scrollTo.id,
    );
    if (!plate) return;
    scrolledKey.current = scrollKey;
    plate.scrollIntoView?.({ block: "center" });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- по новой просьбе или появлению записи в ленте
  }, [scrollKey, items.length]);

  return (
    // Прокручиваемая область берёт фокус с клавиатуры, иначе без мыши ленту не пролистать.
    <div
      className="feed-scroll"
      tabIndex={0}
      role="region"
      aria-label={t("feed.label")}
      ref={scroller}
      onPointerDown={onPointerDownFeed}
    >
      {/* В `column-reverse` первый в разметке стоит внизу: нижний край (новое) идёт перед списком, верхний (старое) — после. */}
      {hasNewer ? <div className="feed__edge" ref={newerEdge} aria-hidden="true" /> : null}
      {loading ? null : items.length === 0 ? (
        <p className="feed__empty">{emptyText ?? t("feed.empty")}</p>
      ) : (
        <ol className="feed">
          {items.map((item) =>
            item.kind === "day" ? (
              <li className="feed__item feed__item--day" key={item.key}>
                <h2 className="day-label">{formatDayLabel(item.journalDate, today, language, labels)}</h2>
              </li>
            ) : item.kind === "card" ? (
              <li className="feed__item feed__item--card" key={item.key}>
                {card?.node}
              </li>
            ) : (
              <li
                className={`feed__item${item.inSeries ? " feed__item--series" : ""}`}
                key={item.key}
                data-message-id={item.message.id}
              >
                <Message
                  markdown={item.message.bodyMd}
                  sentAt={new Date(item.message.createdAt)}
                  timeZone={item.message.createdTz}
                  status={item.message.status}
                  highlighted={item.message.highlighted || ui?.markedId === item.message.id}
                  markTerms={ui?.markedId === item.message.id ? ui?.markTerms : undefined}
                  onRetry={onRetry ? () => onRetry(item.message.id) : undefined}
                  onOpenLink={onOpenLink}
                  isTask={item.message.isTask}
                  taskDone={item.message.taskDoneAt != null}
                  favorite={item.message.favoriteAt != null}
                  byAgent={item.message.origin === "agent"}
                  readOnly={item.message.readOnly}
                  quote={item.message.replyTo ? <ReplyQuote quote={item.message.replyTo} onOpen={onOpenReply} /> : undefined}
                  tags={item.message.tags}
                  mode={
                    ui?.editingId === item.message.id ? "editing" : ui?.deletedIds?.has(item.message.id) ? "deleted" : "view"
                  }
                  editError={ui?.editingId === item.message.id ? ui?.editError : undefined}
                  moveTargets={moveTargets}
                  onDragStart={onDragStart?.(item.message.id)}
                  {...(actions ? messageHandlers(actions, item.message.id) : {})}
                />
              </li>
            ),
          )}
        </ol>
      )}
      {hasOlder ? <div className="feed__edge" ref={olderEdge} aria-hidden="true" /> : null}
    </div>
  );
}

/** Обработчики плашки по id записи. */
function messageHandlers(actions: FeedActions, id: string) {
  return {
    onEdit: () => actions.onEdit(id),
    onSaveEdit: (text: string) => actions.onSaveEdit(id, text),
    onCancelEdit: () => actions.onCancelEdit(id),
    onToggleTask: (on: boolean) => actions.onToggleTask(id, on),
    onToggleDone: (done: boolean) => actions.onToggleDone(id, done),
    onToggleFavorite: (on: boolean) => actions.onToggleFavorite(id, on),
    onChangeDate: () => actions.onChangeDate(id),
    onDelete: () => actions.onDelete(id),
    onRestore: () => actions.onRestore(id),
    onToggleTaskItem: (index: number, checked: boolean) => actions.onToggleTaskItem(id, index, checked),
    onTagClick: actions.onTagClick,
    onMoveTo: (chatId: string) => actions.onMoveTo(id, chatId),
  };
}
