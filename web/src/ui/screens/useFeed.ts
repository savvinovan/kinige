import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessageFlag } from "../../api/generated/MessageFlag";
import type { MessageView } from "../../api/generated/MessageView";
import type { FeedMessage } from "../feed/buildFeed";
import type { FeedActions, FeedUiState } from "../feed/Feed";
import { mergeLatestPage } from "../feed/mergeFeed";

/** Сколько записей грузит первая страница ленты; более старые (дозагрузка) — слайс 8. */
export const PAGE_SIZE = 200;

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
}

/**
 * Лента открытого чата и действия над её записями (слайс 3): правка в плашке, удаление, флаги, день, пункты `- [ ]`.
 * Без оптимистичного показа: запись меняется после ответа Rust. Удалённая запись стоит на месте приглушённой («Удалено ·
 * Вернуть»), пока человек не уйдёт из чата (решение пользователя 1).
 */
export function useFeed({ api, chatId, tag, refreshChats, onError, onTag }: UseFeedOptions) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<FeedMessage[]>([]);
  const [today, setToday] = useState("");
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
  const feedKey = `${chatId ?? ""}\u0000${tag ?? ""}`;
  const [shownKey, setShownKey] = useState(feedKey);
  if (shownKey !== feedKey) {
    setShownKey(feedKey);
    setEditingId(null);
    setEditError(undefined);
    setDeletedIds(new Set());
    setDateFor(null);
    setMessages([]);
  }

  // Смена чата или фильтра: лента перечитывается.
  useEffect(() => {
    if (chatId === null) return;
    let live = true;
    api
      .listMessages(chatId, { limit: PAGE_SIZE, tag: tag ?? undefined })
      .then((page) => {
        if (!live) return;
        setMessages(page.messages);
        setToday(page.today);
        setLoadError(null);
      })
      .catch((reason: unknown) => live && setLoadError(errorCode(reason)));
    return () => {
      live = false;
    };
  }, [api, chatId, tag]);

  // Свежая последняя страница: запись пришла из окна захвата или из буфера (событие Rust «записи изменились»). Подгруженная
  // история не сбрасывается; удалённые записи, которые ещё стоят на месте, не исчезают вместе со страницей.
  const reload = useCallback(async () => {
    const { chatId: id, tag: filter } = current.current;
    if (id === null) return;
    try {
      const page = await api.listMessages(id, { limit: PAGE_SIZE, tag: filter ?? undefined });
      if (current.current.chatId !== id || current.current.tag !== filter) return;
      setMessages((loaded) => {
        const merged = mergeLatestPage(loaded, page.messages, page.older !== null);
        const kept = loaded.filter((m) => deleted.current.has(m.id) && !merged.some((x) => x.id === m.id));
        return kept.length === 0 ? merged : [...merged, ...kept];
      });
      setToday(page.today);
    } catch {
      // Лента остаётся прежней: следующее событие перечитает.
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

  const ui: FeedUiState = { editingId, deletedIds, editError };
  const dateTarget = dateFor === null ? undefined : messages.find((m) => m.id === dateFor);
  return {
    messages,
    today,
    loadError,
    ui,
    actions,
    reload,
    appendSent,
    dateTarget,
    closeDate: () => setDateFor(null),
    pickDate,
  };
}
