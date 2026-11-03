import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { INBOX_ID } from "../../api/KinigeApi";
import { MOCK_SETTINGS_FILE } from "../../api/MockApiSlice4b";
import { MOCK_AGENT_SOCKET } from "../../api/MockApiSlice4a";
import { mockMessage } from "../../api/mockData";
import type { AgentAccess } from "../../api/generated/AgentAccess";
import type { AgentChatChoice } from "../../api/generated/AgentChatChoice";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import type { Settings } from "../../api/generated/Settings";
import type { SettingsFileState } from "../../api/generated/SettingsFileState";
import { DEFAULT_SETTINGS } from "../../api/mockData";
import type { AgentLogView } from "../agents/AgentLog";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { DEFAULT_BACKUP } from "../settings/demoBackup";
import { SettingsWindowView, type SettingsWindowViewProps } from "./SettingsWindowView";
import type { Route } from "./routes";
import "./prefs-stories.css";

// The "Settings" window (slice 4b) in all pages and states. "Now" is fixed: Wednesday, 28 October 2026, 14:10 (UTC).
const NOW = new Date(Date.UTC(2026, 9, 28, 14, 10));
const at = (hour: number, minute: number) => Date.UTC(2026, 9, 28, hour, minute);
const noop = () => undefined;

const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "~/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
  backup: DEFAULT_BACKUP,
};

const chat = (id: string, title: string, icon: string | null, count: number, patch: Partial<AgentChatChoice> = {}): AgentChatChoice => ({
  id,
  title,
  icon,
  isInbox: false,
  count,
  aiExcluded: false,
  visible: false,
  ...patch,
});

const CHATS: AgentChatChoice[] = [
  chat(INBOX_ID, "", "📥", 1724, { isInbox: true, visible: true }),
  chat("work", "Работа", "💼", 987, { visible: true }),
  chat("books", "Книги", "📚", 1234),
  chat("diary", "Дневник", "📓", 412, { aiExcluded: true }),
];

const access = (patch: Partial<AgentAccess> = {}): AgentAccess => ({ enabled: true, mode: "read", chats: CHATS, socket: MOCK_AGENT_SOCKET, ...patch });
const OFF = access({ enabled: false, chats: CHATS.map((c) => ({ ...c, visible: false })) });

const row = (id: number, patch: Partial<AgentLogRow> & Pick<AgentLogRow, "tool">): AgentLogRow => ({
  id,
  at: at(14, 2),
  client: "Claude Desktop",
  query: null,
  outcome: "ok",
  count: 0,
  ...patch,
});

const ROWS: AgentLogRow[] = [
  row(6, { tool: "settings_file", client: "settings.json", at: at(14, 8) }),
  row(5, { tool: "add_entry", at: at(14, 6), outcome: "read_only" }),
  row(4, { tool: "add_entry", at: at(14, 5), count: 1 }),
  row(3, { tool: "search", at: at(14, 2), query: "Raft", count: 3 }),
  row(2, { tool: "list_chats", at: at(14, 1) }),
  row(1, { tool: "connect", at: at(14, 0) }),
];

const entry = (messageId: string, chatTitle: string, icon: string, text: string, chatIsInbox = false): AgentLogEntry => ({
  messageId,
  chatTitle,
  chatIcon: icon,
  chatIsInbox,
  message: mockMessage({ id: messageId, bodyMd: text, journalDate: "2026-03-14", origin: "user" }),
});

const EXPANDED: Record<number, AgentLogEntry[]> = {
  3: [
    entry("m1", "Работа", "💼", "Итоги встречи: договорились разобрать Raft и сравнить с Paxos до конца месяца"),
    entry("m2", "Входящие", "📥", "Мысль про Raft: лидер выбирается по таймауту", true),
    { messageId: "gone", message: null, chatTitle: "", chatIcon: null, chatIsInbox: false },
  ],
};

const LOG: AgentLogView = { rows: ROWS, hasMore: false, expandedId: null, entries: {}, gone: new Set() };
const EMPTY_LOG: AgentLogView = { rows: [], hasMore: false, expandedId: null, entries: {}, gone: new Set() };

const file = (patch: Partial<SettingsFileState> = {}): SettingsFileState => ({ ...MOCK_SETTINGS_FILE, ...patch });

const BASE: SettingsWindowViewProps = {
  route: "appearance",
  onNavigate: noop,
  settings: SETTINGS,
  access: access(),
  file: file(),
  log: LOG,
  language: "ru",
  now: NOW,
  // The tree in stories does not read localStorage: only the group of the current page is expanded (if it is inside a group).
  treeExpanded: [],
  warning: false,
  onTextSizeChange: noop,
  onChooseFolder: noop,
  onShortcutChange: noop,
  onShortcutReset: noop,
  onShortcutRecording: noop,
  onEnabledChange: noop,
  onConfirmEnable: noop,
  onCancelEnable: noop,
  onModeChange: noop,
  onChatToggle: noop,
  onToggleRow: noop,
  onOpenEntry: noop,
  onMoreLog: noop,
};

interface DemoProps extends Partial<SettingsWindowViewProps> {
  /** The smallest window (640 × 440). */
  min?: boolean;
  /** Immediately starts recording a new shortcut in the first row ("Изменить" is pressed in the story: there is no mouse). */
  record?: boolean;
}

/** A window with real navigation through the tree; the rest makes no calls. */
function Demo({ min = false, record = false, route: initial = "appearance", warning: initialWarning = false, ...rest }: DemoProps) {
  const [route, setRoute] = useState<Route>(initial);
  const [warning, setWarning] = useState(initialWarning);
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (record) frame.current?.querySelector<HTMLButtonElement>(".shortcut-row__actions button")?.click();
  }, [record]);
  return (
    <div className={`story-prefs-frame${min ? " story-prefs-frame--min" : ""}`} ref={frame}>
      <SettingsWindowView
        {...BASE}
        {...rest}
        route={route}
        onNavigate={(next) => {
          setRoute(next);
          setWarning(false);
        }}
        warning={warning}
        onEnabledChange={(on) => setWarning(on)}
        onConfirmEnable={() => setWarning(false)}
        onCancelEnable={() => setWarning(false)}
      />
    </div>
  );
}

const meta = {
  title: "Экраны/Окно настроек",
  component: Demo,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Appearance: the text size. */
export const Оформление: Story = {};

/** Keyboard shortcuts: both are assigned. */
export const Сочетания: Story = { args: { route: "shortcuts" } };

/** Recording a new shortcut: the recording field in place of the keys, the buttons hidden, the row layout does not change. */
export const СочетанияЗапись: Story = { args: { route: "shortcuts", record: true } };

/** Tree: the "Доступ ИИ-агентов" group is collapsed (the default), chevron ›. */
export const ДеревоСвёрнуто: Story = {};

/** Tree: the group is expanded, chevron ⌄ and nested items. */
export const ДеревоРазвёрнуто: Story = { args: { treeExpanded: ["access"] } };

/** A narrow window: the keys and buttons move below the name. */
export const НаименьшийРазмерСочетания: Story = { args: { min: true, route: "shortcuts", record: true } };

/** A shortcut is taken by another program ("не назначено"), the second one has a save error. */
export const СочетаниеЗанято: Story = {
  args: {
    route: "shortcuts",
    settings: {
      ...SETTINGS,
      shortcuts: [
        { kind: "capture", accelerator: "Ctrl+Alt+KeyN", isDefault: true, state: "taken" },
        { kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: true, state: "assigned" },
      ],
    },
    shortcutErrors: { clipboard: "Другое действие уже использует это сочетание." },
  },
};

/** Copies: the Markdown copy is up to date, the database copy is taken. */
export const Копии: Story = { args: { route: "mirror", settings: { ...SETTINGS, backup: DEFAULT_BACKUP } } };

/** Copies: the folder is unavailable, days are waiting to be written, a choice error. */
export const КопииОшибка: Story = {
  args: {
    route: "mirror",
    settings: { ...SETTINGS, mirror: { ...SETTINGS.mirror, folder: "/Volumes/Backup/Журнал", isDefault: false, error: "folder_unavailable", pendingDays: 3 } },
    pickError: "Не удалось выбрать папку.",
  },
};

/** Access is off: the mode is unavailable, chats and the log open through the "Настроить ›" rows. */
export const ДоступВыключен: Story = { args: { route: "access", access: OFF } };

/** Windows until named pipes (slice 10a): there is no MCP server, so one quiet line stands in for the switch. */
export const ДоступНедоступен: Story = {
  args: {
    route: "access",
    access: access({ socket: { listening: false, error: "unsupported", bridgePath: null, socketPath: null, desktopConfig: null, codeCommand: null } }),
  },
};

/** The model provider warning is a single Modal over the window; the page under it stays in place. */
export const ПредупреждениеПриВключении: Story = { args: { route: "access", access: OFF, warning: true } };

/** Enabled, "чтение и запись"; the section was changed through the file: a quiet line next to it. */
export const ДоступВключён: Story = {
  args: {
    route: "access",
    access: access({ mode: "read_write" }),
    file: file({ changed: { appearance: null, shortcuts: null, mirror: null, access: at(14, 8) } }),
  },
};

/** The "Видимые чаты" subscreen: the whole list, "Дневник" is excluded from AI. */
export const ВидимыеЧаты: Story = { args: { route: "access/chats" } };

/** The "Журнал обращений" subscreen: empty. */
export const ЖурналПуст: Story = { args: { route: "access/log", log: EMPTY_LOG } };

/** The "Журнал обращений" subscreen: a row is expanded, "Показать ещё" at the bottom of the page; the "settings.json · доступ изменён" row. */
export const ЖурналДлинный: Story = {
  args: {
    route: "access/log",
    log: {
      rows: [...ROWS, ...Array.from({ length: 8 }, (_, i) => row(100 + i, { tool: "get_entries", at: at(13, 50 - i), count: 5 + i }))],
      hasMore: true,
      expandedId: 3,
      entries: EXPANDED,
      gone: new Set(),
    },
  },
};

/** A settings file without errors: the path, "Копировать путь". */
export const ФайлБезОшибок: Story = { args: { route: "file" } };

/** The file did not parse: line and column; the tree item has an error mark. */
export const ФайлНеРазобран: Story = {
  args: { route: "file", file: file({ problem: { kind: "parse", line: 1, column: 15 } }) },
};

/** Errors of several keys and an unknown key: the rest is applied. */
export const ОшибкиКлючей: Story = {
  args: {
    route: "file",
    file: file({
      issues: [
        { key: "text_size", kind: "value" },
        { key: "shortcuts.capture", kind: "shortcut_taken" },
        { key: "mirror.folder", kind: "folder_missing" },
        { key: "agent_access.mode", kind: "type" },
        { key: "agent_access.chats[1]", kind: "chat_ambiguous" },
        { key: "agent_access.chats[2]", kind: "chat_excluded" },
      ],
      unknownKeys: ["surprise", "shortcuts.extra"],
    }),
  },
};

/** Unsafe file permissions: the access keys and folders from it are not applied. */
export const ФайлНебезопасен: Story = {
  args: {
    route: "file",
    file: file({ unsafeReason: "permissions", issues: [{ key: "agent_access", kind: "unsafe_ignored" }] }),
  },
};

/** Long paths and names wrap or are truncated with an ellipsis. */
export const ДлинныеПутиИНазвания: Story = {
  args: {
    route: "file",
    file: file({
      path: "/Users/someone-with-a-very-long-account-name/Library/Application Support/me.savvinov.kinige/some/deeply/nested/folder/with/long/names/settings.json",
      issues: [{ key: "agent_access.chats[12345678901234567890]", kind: "chat_not_found" }],
      unknownKeys: ["очень_длинное_неизвестное_имя_ключа_которое_не_помещается_в_строку_и_должно_переноситься_по_словам_или_в_любом_месте"],
    }),
  },
};

/** The smallest window (640 × 440): everything is reachable, the tree does not scroll. */
export const НаименьшийРазмер: Story = { args: { min: true, route: "access", access: OFF } };

/** The smallest window, a subscreen. */
export const НаименьшийРазмерПодэкран: Story = { args: { min: true, route: "access/chats" } };

/** The English version of a story: the interface language and labels switch, the data is the same. */
const en = (story: Story, args: Partial<DemoProps> = {}): Story => ({
  ...story,
  args: { ...story.args, language: "en", ...args },
  globals: { language: "en" },
});

/** English: appearance. */
export const English: Story = en(Оформление);
export const EnglishShortcuts: Story = en(Сочетания);
export const EnglishShortcutTaken: Story = en(СочетаниеЗанято, { shortcutErrors: { clipboard: "The other action already uses this shortcut." } });
export const EnglishMirror: Story = en(Копии);
export const EnglishMirrorError: Story = en(КопииОшибка, { pickError: "Could not choose the folder." });
/** English: access off, the warning, enabled. */
export const EnglishAccessOff: Story = en(ДоступВыключен);
export const EnglishWarning: Story = en(ПредупреждениеПриВключении);
export const EnglishAccessUnavailable: Story = en(ДоступНедоступен);
export const EnglishAccess: Story = en(ДоступВключён);
export const EnglishVisibleChats: Story = en(ВидимыеЧаты);
export const EnglishLogEmpty: Story = en(ЖурналПуст);
export const EnglishLogLong: Story = en(ЖурналДлинный);
export const EnglishFileOk: Story = en(ФайлБезОшибок);
export const EnglishFile: Story = en(ФайлНеРазобран, { file: file({ problem: { kind: "parse", line: 3, column: 9 }, issues: [{ key: "text_size", kind: "value" }] }) });
export const EnglishKeyErrors: Story = en(ОшибкиКлючей);
export const EnglishUnsafe: Story = en(ФайлНебезопасен);
export const EnglishLongNames: Story = en(ДлинныеПутиИНазвания);
export const EnglishMinSize: Story = en(НаименьшийРазмер);
export const EnglishMinSizeSubscreen: Story = en(НаименьшийРазмерПодэкран);
export const EnglishShortcutsRecording: Story = en(СочетанияЗапись);
export const EnglishTreeCollapsed: Story = en(ДеревоСвёрнуто);
export const EnglishTreeExpanded: Story = en(ДеревоРазвёрнуто);
export const EnglishMinSizeShortcuts: Story = en(НаименьшийРазмерСочетания);
