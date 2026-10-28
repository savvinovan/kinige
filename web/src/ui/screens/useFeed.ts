import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessageFlag } from "../../api/generated/MessageFlag";
import type { MessageView } from "../../api/generated/MessageView";
import type { FeedCursor } from "../../api/generated/FeedCursor";
import type { FeedMessage } from "../feed/buildFeed";
import type { FeedActions, FeedPaging, FeedUiState, ScrollRequest } from "../feed/Feed";
import { mergeLatestPage, mergeOlder, withDeleted } from "../feed/mergeFeed";

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
}

/**
 * Лента открытого чата и действия над её записями (слайс 3): правка в плашке, удаление, флаги, день, пункты `- [ ]`.
 * Без оптимистичного показа: запись меняется после ответа Rust. Удалённая запись стоит на месте приглушённой («Удалено ·
 * Вернуть»), пока человек не уйдёт из чата (решение пользователя 1).
 */
export function useFeed({ api, chatId, tag, refreshChats, onError, onTag, focus = null }: UseFeedOptions) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<FeedMessage[]>([]);
  const [today, setToday] = useState("");
  // Края загруженной ленты: курсор следующей старой страницы и следующей новой; `newer === null` — лента дошла до конца.
  const [older, setOlder] = useState<FeedCursor | null>(null);
  const [newer, setNewer] = useState<FeedCursor | null>(null);
  const [mark, setMark] = useState<{ id: string; terms: readonly string[] } | null>(null);
  const [scrollTo, setScrollTo] = useState<ScrollRequest | null>(null);
  // Переход уже выполнен: тот же `nonce` второй раз (возврат в чат) ленту к записи не уводит.
  const usedFocus = useRef(0);
  const paging = useRef({ older, newer });
  useEffect(() => {
    paging.current = { older, newer };
  });
  const loadingEdge = useRef(false);
  const [loadError, setLoadError] = useState<ErrorCode | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editError, setEditError] = useState<string | undefined>(undefined);
  const [deletedIds, setDeletedIds] = useState<ReadonlySet<string>>(new Set());
  const [dateFor, setDateFor] = useState<string | null>(null);
  const deleted = useRef(deletedIds);
  const current = useRef({ chatId, tag });
  useEffect(() => {
    deleted.current = deletedIds;
    current.current = { chatId, tag };
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
      setDeletedIds(new Set());
      setMessages([]);
      setOlder(null);
      setNewer(null);
      setMark(null);
    } else {
      // Тот же чат, другой фильтр: на месте остаются только удалённые записи, подходящие под новый фильтр.
      setMessages((shown) => withDeleted(shown, [], deletedIds, tag));
    }
  }

  // Смена чата, фильтра или переход к записи: лента перечитывается. Переход — страница вокруг записи; найденные слова
  // выделяются в ней, запись подсвечена до следующего щелчка (`clearMark`).
  const focusNonce = focus?.nonce ?? 0;
  useEffect(() => {
    if (chatId === null) return;
    let live = true;
    const target = focus && focus.chatId === chatId && focus.nonce > usedFocus.current ? focus : null;
    if (target) usedFocus.current = target.nonce;
    api
      .listMessages(chatId, { limit: PAGE_SIZE, tag: tag ?? undefined, around: target?.messageId })
      .then((page) => {
        if (!live) return;
        setMessages((shown) => withDeleted(shown, page.messages, deleted.current, tag));
        setOlder(page.older);
        setNewer(page.newer);
        setToday(page.today);
        setLoadError(null);
        if (target) {
          setMark({ id: target.messageId, terms: target.terms });
          setScrollTo({ id: target.messageId, nonce: target.nonce });
        }
      })
      .catch((reason: unknown) => live && setLoadError(errorCode(reason)));
    return () => {
      live = false;
    };
    // `focus` читается один раз на `nonce`: сам объект в зависимости не нужен.
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
      setOlder(page.older);
    } catch (reason) {
      onError(errorCode(reason));
    } finally {
      loadingEdge.current = false;
    }
  }, [api, onError]);

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
      setNewer(page.newer);
    } catch (reason) {
      onError(errorCode(reason));
    } finally {
      loadingEdge.current = false;
    }
  }, [api, onError]);

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
      if (page.older === null) setOlder(null);
      setToday(page.today);
    } catch {
      // Лента остаётся прежней: следующее событие перечитает.
    }
  }, [api]);

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
      setOlder(page.older);
      setNewer(page.newer);
      setToday(page.today);
      setMark(null);
    } catch {
      // Лента остаётся прежней: следующее событие или «Готово» перечитает.
    }
  }, [api]);

  const replace = useCallback((view: MessageView) => {
    setMessages((list) => list.map((m) => (m.id === view.id ? view : m)));
  }, []);

  const appendSent = useCallback(
    (sent: MessageView) => {
      // Под фильтром по тегу новая запись видна, только если в ней есть этот тег.
      if (tag !== null && !sent.tags.includes(tag)) return;
      setMessages((all) => [...all, sent]);
    },
    [tag],
  );

  const fail = useCallback((reason: unknown) => onError(errorCode(reason)), [onError]);

  const setFlag = useCallback(
    (id: string, flag: MessageFlag, on: boolean) => {
      api.setMessageFlag(id, flag, on).then(replace).catch(fail);
    },
    [api, replace, fail],
  );

  const actions: FeedActions = useMemo(
    () => ({
      onEdit: (id) => {
        setEditError(undefined);
        setEditingId(id);
      },
      onSaveEdit: (id, text) => {
        api
          .editMessage(id, text)
          .then((view) => {
            replace(view);
            setEditingId(null);
            setEditError(undefined);
          })
          .catch((reason: unknown) => setEditError(t(`error.${errorCode(reason)}`)));
      },
      onCancelEdit: () => {
        setEditingId(null);
        setEditError(undefined);
      },
      onToggleTask: (id, on) => setFlag(id, "task", on),
      onToggleDone: (id, done) => setFlag(id, "task_done", done),
      onToggleFavorite: (id, on) => setFlag(id, "favorite", on),
      onChangeDate: (id) => setDateFor(id),
      onDelete: (id) => {
        api
          .deleteMessage(id)
          .then(() => {
            setDeletedIds((ids) => new Set(ids).add(id));
            setEditingId((editing) => (editing === id ? null : editing));
            return refreshChats();
          })
          .catch(fail);
      },
      onRestore: (id) => {
        api
          .restoreMessage(id)
          .then(() => {
            setDeletedIds((ids) => {
              const next = new Set(ids);
              next.delete(id);
              return next;
            });
            return refreshChats();
          })
          .catch(fail);
      },
      onToggleTaskItem: (id, index, checked) => {
        api
          .toggleTaskItem(id, index, checked)
          .then(replace)
          .catch((reason: unknown) => {
            fail(reason);
            // Текст записи успел измениться: показываем актуальный.
            if (errorCode(reason) === "task_item_stale") void reload();
          });
      },
      onTagClick: onTag,
      onMoveTo: (id, targetChatId) => {
        api
          .moveMessage(id, targetChatId)
          .then(() => {
            // Запись просто исчезает из текущего чата (без тоста, лента не прыгает); счётчики берутся заново.
            setMessages((list) => list.filter((m) => m.id !== id));
            setEditingId((editing) => (editing === id ? null : editing));
            return refreshChats();
          })
          .catch(fail);
      },
    }),
    [api, t, replace, setFlag, refreshChats, fail, reload, onTag],
  );

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
    loadError,
    ui,
    actions,
    paging: feedPaging,
    scrollTo,
    clearMark: useCallback(() => setMark(null), []),
    reload,
    restart,
    appendSent,
    dateTarget,
    closeDate: () => setDateFor(null),
    pickDate,
  };
}
