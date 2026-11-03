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

/** Сколько записей грузит страница ленты. Старые страницы подгружаются при прокрутке к верху (слайс 4), без виртуализации. */
export const PAGE_SIZE = 200;

/** Переход к записи (из поиска): чат, запись и слова, которые нужно выделить. Новый `nonce` — новый переход. */
export interface FocusTarget {
  chatId: string;
  messageId: string;
  terms: readonly string[];
  nonce: number;
}

interface UseFeedOptions {
  api: KinigeApi;
  /** Открытый чат; `null` — открыта корзина, ленты нет. */
  chatId: string | null;
  /** Фильтр по тегу (нижний регистр, без `#`). */
  tag: string | null;
  /** Счётчики чатов и корзины после изменения записи. */
  refreshChats: () => Promise<void>;
  /** Ошибка действия, которой нет места внутри плашки (показывается над полем ввода). */
  onError: (code: ErrorCode) => void;
  /** Щелчок по `#тегу`. */
  onTag: (tag: string) => void;
  /** Открыть чат на записи: страница вокруг неё, дальше подгрузка вверх и вниз. Используется один раз на `nonce`. */
  focus?: FocusTarget | null;
  /** The entries of the journal changed through this feed (sent, edited, deleted, restored, moved, re-dated): the forest reloads. */
  onMutated?: () => void;
}

/**
 * Лента открытого чата и действия над её записями (слайс 3): правка в плашке, удаление, флаги, день, пункты `- [ ]`.
 * Без оптимистичного показа: запись меняется после ответа Rust. Удалённая запись стоит на месте приглушённой («Удалено ·
 * Вернуть»), пока человек не уйдёт из чата (решение пользователя 1).
 */
export function useFeed({ api, chatId, tag, refreshChats, onError, onTag, focus = null, onMutated }: UseFeedOptions) {
  const mutatedRef = useRef(onMutated);
  useEffect(() => {
    mutatedRef.current = onMutated;
  });
  const mutated = useCallback(() => mutatedRef.current?.(), []);
  const [messages, setMessages] = useState<FeedMessage[]>([]);
  const [today, setToday] = useState("");
  // Первая страница открытого чата получена: до этого лента может ничего не рисовать (первый кадр с карточкой «В этот день»).
  const [loaded, setLoaded] = useState(false);
  // Края загруженной ленты: курсор следующей старой страницы и следующей новой; `newer === null` — лента дошла до конца.
  const [older, setOlder] = useState<FeedCursor | null>(null);
  const [newer, setNewer] = useState<FeedCursor | null>(null);
  const [mark, setMark] = useState<{ id: string; terms: readonly string[] } | null>(null);
  const [scrollTo, setScrollTo] = useState<ScrollRequest | null>(null);
  // Переход уже выполнен: тот же `nonce` второй раз (возврат в чат) ленту к записи не уводит.
  const usedFocus = useRef(0);
  // Курсоры для обработчиков краёв и события Rust. Новый курсор попадает сюда в тот же миг, что и в состояние (`setCursor`):
  // React сбрасывает эффекты с задержкой, и край, показавшийся сразу после отрисовки страницы, грузил бы по старому курсору,
  // а событие «записи изменились» принимало бы середину истории за конец. В смене чата курсоры сбрасывает эффект ниже.
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
  // Последние значения для обработчиков, которые живут дольше одной отрисовки (ответы Rust, события, отправка записи).
  const current = useRef({ chatId, tag, loaded, messages, onError });
  const shownToday = useRef(today);
  useEffect(() => {
    deleted.current = deletedIds;
    current.current = { chatId, tag, loaded, messages, onError };
    shownToday.current = today;
  });

  // Смена чата или фильтра: состояние плашек сбрасывается прямо при отрисовке (так React советует «подстраивать состояние»).
  // Удалённые записи («Удалено · Вернуть») живут, пока человек не уйдёт из чата; смена тега-фильтра чат не меняет.
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
      // Просьба прокрутить к записи относилась к прежнему чату: вернувшись, лента открывается внизу, а не у старой записи.
      setScrollTo(null);
    } else {
      // Тот же чат, другой фильтр: на месте остаются только удалённые записи, подходящие под новый фильтр.
      setMessages((shown) => withDeleted(shown, [], deletedIds, tag));
    }
  }

  // Другой чат: курсоры прежнего больше не годятся (состояние сброшено выше). Объявлен раньше загрузки ленты, чтобы не
  // затереть курсоры, которые она принесёт.
  useEffect(() => {
    paging.current = { older: null, newer: null };
  }, [chatKey]);

  /**
   * Переход к записи не удался (её нет на месте: в корзине, удалена): загруженные копии записей устарели, прежде всего цитаты
   * ответов (у них осталось `state: "live"`). Свежие копии встают на место загруженных; ничего не добавляется и не убирается.
   * Окно: последняя страница, а в середине истории — страница вокруг середины загруженного (прокрутка не прыгает).
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
      // Цитаты обновятся при следующем чтении ленты.
    }
  }, [api]);

  // Смена чата, фильтра или переход к записи: лента перечитывается. Переход — страница вокруг записи; найденные слова
  // выделяются в ней, запись подсвечена до следующего щелчка (`clearMark`). Не удавшийся переход — не ошибка загрузки: лента
  // остаётся, ошибка уходит в `onError`; в чате, где ничего ещё не загружено, показывается его последняя страница.
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
    // `focus` читается один раз на `nonce`: сам объект в зависимости не нужен; `setCursor` и `refreshLoaded` стабильны.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, chatId, tag, focusNonce]);

  /** Старая страница сверху: курсор `older`, записи встают перед загруженными. Прокрутка при этом не прыгает. */
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

  /** Новая страница снизу (только после перехода к записи из середины истории); дойдя до конца, лента снова «живая». */
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

  // Свежая последняя страница: запись пришла из окна захвата или из буфера (событие Rust «записи изменились»). Подгруженная
  // история не сбрасывается; удалённые записи, которые ещё стоят на месте, не исчезают вместе со страницей.
  const reload = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    // Лента открыта в середине истории (переход из поиска): новые записи снизу появятся, когда до них дойдёт подгрузка.
    if (paging.current.newer !== null) return;
    try {
      const page = await api.listMessages(id, { limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => {
        const merged = mergeLatestPage(loaded, page.messages, page.older !== null);
        return withDeleted(loaded, merged, deleted.current, filter);
      });
      // Подгруженная история остаётся; если старше страницы ничего нет, курсора больше нет.
      if (page.older === null) setCursor("older", null);
      setToday(page.today);
    } catch {
      // Лента остаётся прежней: следующее событие перечитает.
    }
  }, [api, setCursor]);

  /**
   * Наступил новый день журнала (после 04:00), пока окно было открыто: лента перечитывается, метки «Сегодня» и «Вчера» встают
   * правильно. Вызывается при фокусе окна; день спрашивается у Rust самым дешёвым запросом (одна запись), независимо от того,
   * есть ли карточка «В этот день».
   */
  const checkDay = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    try {
      const page = await api.listMessages(id, { limit: 1, tag: filter ?? undefined });
      if (shownToday.current !== "" && page.today !== shownToday.current) await reload();
    } catch {
      // Следующий фокус проверит снова.
    }
  }, [api, reload]);

  /**
   * Лента с нуля: последняя страница заменяет всё загруженное, оба курсора пересчитываются. Нужна, когда записи легли в
   * середину уже загруженной истории (импорт): курсор «старше» прежней ленты указывал бы мимо новых записей, а страницы
   * между ними остались бы незагруженными. Удалённые плашки, не ушедшие из чата, остаются на месте.
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
      // Лента остаётся прежней: следующее событие или «Готово» перечитает.
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
   * Отправленная запись встаёт в конец ленты. Вызывается из ответа на отправку, который мог прийти после смены чата или
   * фильтра, поэтому чат и тег читаются из `current`, а не из замыкания. Запись другого чата в ленту не попадает; под
   * фильтром по тегу она видна, только если в ней есть этот тег; лента в середине истории (после перехода к записи)
   * открывается заново у живого края, иначе запись встала бы сразу после старых.
   */
  const appendSent = useCallback(
    (sent: MessageView) => {
      mutated();
      const { chatId: id, tag: filter } = current.current;
      if (sent.chatId !== id) return;
      if (filter !== null && !sent.tags.includes(filter)) return;
      if (paging.current.newer !== null) {
        // Лента встаёт у живого края, отправленная запись — в центре кадра, как после перехода к записи.
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

  /** Календарь выбрал день для записи `dateFor`; ответ — код ошибки (диалог показывает её), `null` — готово. */
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
