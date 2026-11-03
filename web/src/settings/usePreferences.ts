import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../api/KinigeApi";
import type { AgentAccess } from "../api/generated/AgentAccess";
import type { AgentLogEntry } from "../api/generated/AgentLogEntry";
import type { AgentLogRow } from "../api/generated/AgentLogRow";
import type { AgentMode } from "../api/generated/AgentMode";
import type { ErrorCode } from "../api/generated/ErrorCode";
import type { Settings } from "../api/generated/Settings";
import type { SettingsFileState } from "../api/generated/SettingsFileState";
import type { ShortcutKind } from "../api/generated/ShortcutKind";
import type { TextSize } from "../api/generated/TextSize";
import { useLanguage } from "../i18n/I18n";
import type { SettingsWindowViewProps } from "../ui/prefs/SettingsWindowView";
import type { Route } from "../ui/prefs/routes";
import { applyTextSize } from "../ui/screens/textSize";

/** How often the open window rereads the backups state, the socket and the first log page (agent calls arrive on their own). */
const REFRESH_MS = 2000;

/** Everything the window reads on opening: it is drawn already with data, without flashing empty. */
export interface PreferencesInitial {
  settings: Settings;
  access: AgentAccess;
  file: SettingsFileState;
  log: { rows: AgentLogRow[]; older: number | null };
}

/** Reads the initial data of the settings window (before the first frame). */
export async function loadPreferences(api: KinigeApi): Promise<PreferencesInitial> {
  const [settings, access, file, log] = await Promise.all([api.getSettings(), api.getAgentAccess(), api.getSettingsFile(), api.listAgentLog()]);
  return { settings, access, file, log: { rows: log.rows, older: log.older } };
}

/**
 * The state of the "Настройки" (Settings) window (slice 4b): settings, agent access, the file, the log; the window actions. The window gets no feed:
 * an entry from the log is opened by the `settingsOpenEntry` command (the main window will open the chat itself). The data is reread on the
 * `settings-changed` event (an edit from this window, another window or the file) and once every two seconds (backups state, agent calls).
 */
export function usePreferences(api: KinigeApi, initial: PreferencesInitial) {
  const { t } = useTranslation();
  const language = useLanguage();
  const [route, setRoute] = useState<Route>("appearance");
  const [settings, setSettings] = useState(initial.settings);
  const [access, setAccess] = useState(initial.access);
  const [file, setFile] = useState(initial.file);
  const [first, setFirst] = useState(initial.log);
  const [extra, setExtra] = useState<{ rows: AgentLogRow[]; older: number | null } | null>(null);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [entries, setEntries] = useState<Record<number, readonly AgentLogEntry[] | undefined>>({});
  const [gone, setGone] = useState<ReadonlySet<string>>(new Set());
  const [warning, setWarning] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [pickError, setPickError] = useState<ErrorCode | null>(null);
  const [accessError, setAccessError] = useState<ErrorCode | null>(null);
  const [shortcutErrors, setShortcutErrors] = useState<Partial<Record<ShortcutKind, string>>>({});

  useEffect(() => applyTextSize(settings.textSize), [settings.textSize]);

  const refresh = useCallback(async () => {
    try {
      const [fresh, freshAccess, freshFile, page] = await Promise.all([
        api.getSettings(),
        api.getAgentAccess(),
        api.getSettingsFile(),
        api.listAgentLog(),
      ]);
      setSettings(fresh);
      setAccess(freshAccess);
      setFile(freshFile);
      setFirst({ rows: page.rows, older: page.older });
    } catch {
      // The window keeps its previous data; the next refresh will retry.
    }
  }, [api]);

  useEffect(() => {
    let closed = false;
    let off: (() => void) | undefined;
    api
      .onSettingsChanged(() => void refresh())
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => {
      closed = true;
      off?.();
      clearInterval(timer);
    };
  }, [api, refresh]);

  const navigate = (next: Route) => {
    setRoute(next);
    setWarning(false);
  };

  const applyShortcut = (kind: ShortcutKind, request: Promise<Settings>) => {
    setShortcutErrors((old) => ({ ...old, [kind]: undefined }));
    request
      .then(setSettings)
      .catch((reason: unknown) => setShortcutErrors((old) => ({ ...old, [kind]: t(`error.${errorCode(reason)}`) })))
      .finally(() => void refresh());
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

  // The entry opens in the main window. It could have been deleted forever after the log was read: then it stays a quiet "Запись удалена" (Entry deleted) line.
  const openEntry = (_logId: number, entry: AgentLogEntry) => {
    const message = entry.message;
    if (!message) return;
    api.settingsOpenEntry(message.id).catch((reason: unknown) => {
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
  const visibleIds = access.chats.filter((chat) => chat.visible).map((chat) => chat.id);

  const view: SettingsWindowViewProps = {
    route,
    onNavigate: navigate,
    settings,
    access,
    file,
    log: { rows, hasMore: (extra ? extra.older : first.older) !== null, expandedId, entries, gone },
    language,
    warning,
    choosing,
    pickError: pickError ? t(`error.${pickError}`) : undefined,
    shortcutErrors,
    accessError: accessError ? t(`error.${accessError}`) : undefined,
    onTextSizeChange: (size: TextSize) => {
      // The size is applied at once; a failure brings the previous one back on reread.
      setSettings((old) => ({ ...old, textSize: size }));
      api
        .setTextSize(size)
        .then(() => setPickError(null))
        .catch((reason: unknown) => setPickError(errorCode(reason)))
        .finally(() => void refresh());
    },
    onChooseFolder: () => {
      setChoosing(true);
      setPickError(null);
      api
        .chooseMirrorFolder()
        .then(setSettings)
        .catch((reason: unknown) => setPickError(errorCode(reason)))
        .finally(() => {
          setChoosing(false);
          void refresh();
        });
    },
    onShortcutChange: (kind, accelerator) => applyShortcut(kind, api.setShortcut(kind, accelerator)),
    onShortcutReset: (kind) => applyShortcut(kind, api.resetShortcut(kind)),
    onShortcutRecording: (active) => void api.shortcutRecording(active).catch(() => undefined),
    onEnabledChange: (on) => (on ? setWarning(true) : accessAction(api.setAgentEnabled(false))),
    onConfirmEnable: () => {
      setWarning(false);
      accessAction(api.setAgentEnabled(true));
    },
    onCancelEnable: () => setWarning(false),
    onModeChange: (mode: AgentMode) => accessAction(api.setAgentMode(mode)),
    onChatToggle: (id, on) => accessAction(api.setAgentChats(on ? [...visibleIds, id] : visibleIds.filter((visible) => visible !== id))),
    onToggleRow: toggleRow,
    onOpenEntry: openEntry,
    onMoreLog: loadMore,
  };

  function accessAction(action: Promise<void>) {
    action
      .then(() => setAccessError(null))
      .catch((reason: unknown) => setAccessError(errorCode(reason)))
      .finally(() => void refresh());
  }

  return view;
}
