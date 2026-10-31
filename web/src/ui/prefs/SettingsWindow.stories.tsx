import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
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

// Окно «Настройки» (слайс 4b) во всех страницах и состояниях. «Сейчас» фиксировано: среда, 28 октября 2026, 14:10 (UTC).
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
  /** Наименьшее окно (640 × 440). */
  min?: boolean;
}

/** Окно с настоящей навигацией по дереву; остальное — без вызовов. */
function Demo({ min = false, route: initial = "appearance", warning: initialWarning = false, ...rest }: DemoProps) {
  const [route, setRoute] = useState<Route>(initial);
  const [warning, setWarning] = useState(initialWarning);
  return (
    <div className={`story-prefs-frame${min ? " story-prefs-frame--min" : ""}`}>
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

/** Оформление: размер текста. */
export const Оформление: Story = {};

/** Сочетания клавиш: оба назначены. */
export const Сочетания: Story = { args: { route: "shortcuts" } };

/** Сочетание занято другой программой («не назначено»), у второго ошибка сохранения. */
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

/** Копии: Markdown-копия актуальна, копия базы снята. */
export const Копии: Story = { args: { route: "mirror", settings: { ...SETTINGS, backup: DEFAULT_BACKUP } } };

/** Копии: папка недоступна, дни ждут записи, ошибка выбора. */
export const КопииОшибка: Story = {
  args: {
    route: "mirror",
    settings: { ...SETTINGS, mirror: { ...SETTINGS.mirror, folder: "/Volumes/Backup/Журнал", isDefault: false, error: "folder_unavailable", pendingDays: 3 } },
    pickError: "Не удалось выбрать папку.",
  },
};

/** Доступ выключен: режим недоступен, чаты и журнал открываются строками «Настроить ›». */
export const ДоступВыключен: Story = { args: { route: "access", access: OFF } };

/** Предупреждение о провайдере модели — один Modal поверх окна; страница под ним на месте. */
export const ПредупреждениеПриВключении: Story = { args: { route: "access", access: OFF, warning: true } };

/** Включён, «чтение и запись»; раздел изменён через файл — тихая строка рядом. */
export const ДоступВключён: Story = {
  args: {
    route: "access",
    access: access({ mode: "read_write" }),
    file: file({ changed: { appearance: null, shortcuts: null, mirror: null, access: at(14, 8) } }),
  },
};

/** Подэкран «Видимые чаты»: список целиком, «Дневник» исключён из AI. */
export const ВидимыеЧаты: Story = { args: { route: "access/chats" } };

/** Подэкран «Журнал обращений»: пуст. */
export const ЖурналПуст: Story = { args: { route: "access/log", log: EMPTY_LOG } };

/** Подэкран «Журнал обращений»: строка раскрыта, «Показать ещё» внизу страницы; строка «settings.json · доступ изменён». */
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

/** Файл настроек без ошибок: путь, «Копировать путь». */
export const ФайлБезОшибок: Story = { args: { route: "file" } };

/** Файл не разобран: строка и столбец; у пункта дерева метка ошибки. */
export const ФайлНеРазобран: Story = {
  args: { route: "file", file: file({ problem: { kind: "parse", line: 1, column: 15 } }) },
};

/** Ошибки нескольких ключей и неизвестный ключ: остальное применено. */
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

/** Небезопасные права файла: ключи доступа и папки из него не применяются. */
export const ФайлНебезопасен: Story = {
  args: {
    route: "file",
    file: file({ unsafeReason: "permissions", issues: [{ key: "agent_access", kind: "unsafe_ignored" }] }),
  },
};

/** Длинные пути и названия переносятся или обрезаются многоточием. */
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

/** Наименьшее окно (640 × 440): всё доступно, дерево не прокручивается. */
export const НаименьшийРазмер: Story = { args: { min: true, route: "access", access: OFF } };

/** Наименьшее окно, подэкран. */
export const НаименьшийРазмерПодэкран: Story = { args: { min: true, route: "access/chats" } };

/** Английская версия истории: язык интерфейса и подписи переключаются, данные те же. */
const en = (story: Story, args: Partial<DemoProps> = {}): Story => ({
  ...story,
  args: { ...story.args, language: "en", ...args },
  globals: { language: "en" },
});

/** English: оформление. */
export const English: Story = en(Оформление);
export const EnglishShortcuts: Story = en(Сочетания);
export const EnglishShortcutTaken: Story = en(СочетаниеЗанято, { shortcutErrors: { clipboard: "The other action already uses this shortcut." } });
export const EnglishMirror: Story = en(Копии);
export const EnglishMirrorError: Story = en(КопииОшибка, { pickError: "Could not choose the folder." });
/** English: доступ выключен, предупреждение, включён. */
export const EnglishAccessOff: Story = en(ДоступВыключен);
export const EnglishWarning: Story = en(ПредупреждениеПриВключении);
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
