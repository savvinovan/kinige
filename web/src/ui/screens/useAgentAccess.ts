import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { AgentAccess } from "../../api/generated/AgentAccess";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import type { AgentMode } from "../../api/generated/AgentMode";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import { useLanguage } from "../../i18n/I18n";
import type { AgentAccessDialogProps } from "../agents/AgentAccessDialog";
import type { FocusTarget } from "./useFeed";

/** Как часто открытое окно перечитывает флажок, состояние сокета и первую страницу журнала. */
const REFRESH_MS = 2000;

interface UseAgentAccessOptions {
  api: KinigeApi;
  /** Открыть чат (черновик прежнего уходит в Rust до переключения). */
  select: (chatId: string | null) => Promise<void>;
  /** Лента открывает чат на записи (`useFeed`). */
  onFocus: (target: FocusTarget) => void;
  /** Окно «Настройки» открыто: «Доступ ИИ-агентов: выключен» в нём читается при открытии. */
  settingsOpen: boolean;
  /** Запись открыта в ленте: окна «Настройки» и «Доступ ИИ-агентов» закрываются. */
  onNavigated: () => void;
}

/**
 * Окно «Доступ ИИ-агентов» (слайс 4a): состояние флажка, режима, чатов и журнала, предупреждение при включении, раскрытие строк
 * журнала и открытие записи в ленте. Окно перечитывает данные само, пока открыто: обращения агента идут, пока оно на экране.
 */
export function useAgentAccess({ api, select, onFocus, settingsOpen, onNavigated }: UseAgentAccessOptions) {
  const { t } = useTranslation();
  const language = useLanguage();
  const [open, setOpen] = useState(false);
  const [access, setAccess] = useState<AgentAccess | null>(null);
  const [first, setFirst] = useState<{ rows: AgentLogRow[]; older: number | null }>({ rows: [], older: null });
  const [extra, setExtra] = useState<{ rows: AgentLogRow[]; older: number | null } | null>(null);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [entries, setEntries] = useState<Record<number, readonly AgentLogEntry[] | undefined>>({});
  const [gone, setGone] = useState<ReadonlySet<string>>(new Set());
  const [warning, setWarning] = useState(false);
  const [error, setError] = useState<ErrorCode | null>(null);
  const counter = useRef(0);

  const refresh = useCallback(async () => {
    try {
      const [fresh, page] = await Promise.all([api.getAgentAccess(), api.listAgentLog()]);
      setAccess(fresh);
      setFirst({ rows: page.rows, older: page.older });
    } catch {
      // Окно остаётся с прежними данными; следующее обновление повторит попытку.
    }
  }, [api]);

  // «Доступ ИИ-агентов: выключен» в настройках читается при их открытии.
  useEffect(() => {
    if (!settingsOpen) return;
    let live = true;
    api
      .getAgentAccess()
      .then((fresh) => {
        if (live) setAccess(fresh);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [api, settingsOpen]);

  useEffect(() => {
    if (!open) return;
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  }, [open, refresh]);

  const act = (action: Promise<void>) =>
    action
      .then(() => setError(null))
      .catch((reason: unknown) => setError(errorCode(reason)))
      .finally(() => void refresh());

  const openDialog = () => {
    setError(null);
    setExpandedId(null);
    setExtra(null);
    void refresh().then(() => setOpen(true));
  };

  const closeDialog = () => {
    setOpen(false);
    setWarning(false);
  };

  const toggleRow = (id: number) => {
    if (expandedId === id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(id);
    if (entries[id] !== undefined) return;
    api
      .listAgentLogEntries(id)
      .then((list) => setEntries((old) => ({ ...old, [id]: list })))
      .catch(() => setEntries((old) => ({ ...old, [id]: [] })));
  };

  const openEntry = (_logId: number, entry: AgentLogEntry) => {
    const message = entry.message;
    if (!message) return;
    // Запись могли удалить навсегда после того, как журнал был прочитан: тогда она остаётся в окне тихой строкой «Запись удалена».
    api
      .listMessages(message.chatId, { around: message.id, limit: 1 })
      .then(() => {
        closeDialog();
        onNavigated();
        void select(message.chatId).then(() => {
          counter.current += 1;
          onFocus({ chatId: message.chatId, messageId: message.id, terms: [], nonce: counter.current });
        });
      })
      .catch((reason: unknown) => {
        if (errorCode(reason) === "message_not_found") setGone((old) => new Set(old).add(message.id));
      });
  };

  const loadMore = () => {
    const cursor = extra ? extra.older : first.older;
    if (cursor === null) return;
    api
      .listAgentLog(cursor)
      .then((page) => setExtra((old) => ({ rows: [...(old?.rows ?? []), ...page.rows], older: page.older })))
      .catch(() => undefined);
  };

  const rows = useMemo(() => [...first.rows, ...(extra?.rows ?? [])], [first.rows, extra]);
  const hasMore = (extra ? extra.older : first.older) !== null;

  const visibleIds = (access?.chats ?? []).filter((chat) => chat.visible).map((chat) => chat.id);
  const dialog: AgentAccessDialogProps | null =
    open && access
      ? {
          access,
          log: { rows, hasMore, expandedId, entries, gone },
          language,
          warning,
          error: error ? t(`error.${error}`) : undefined,
          onEnabledChange: (on) => (on ? setWarning(true) : void act(api.setAgentEnabled(false))),
          onConfirmEnable: () => {
            setWarning(false);
            void act(api.setAgentEnabled(true));
          },
          onCancelEnable: () => setWarning(false),
          onModeChange: (mode: AgentMode) => void act(api.setAgentMode(mode)),
          onChatToggle: (id, on) =>
            void act(api.setAgentChats(on ? [...visibleIds, id] : visibleIds.filter((visible) => visible !== id))),
          onToggleRow: toggleRow,
          onOpenEntry: openEntry,
          onMoreLog: loadMore,
          onClose: closeDialog,
        }
      : null;

  return { dialog, enabled: access?.enabled ?? null, openDialog };
}
