import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { MOCK_AGENT_SOCKET } from "../../api/MockApiSlice4a";
import { INBOX_ID } from "../../api/KinigeApi";
import { mockMessage } from "../../api/mockData";
import type { AgentAccess } from "../../api/generated/AgentAccess";
import type { AgentChatChoice } from "../../api/generated/AgentChatChoice";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import { AgentAccessDialog, type AgentAccessDialogProps } from "./AgentAccessDialog";
import "../settings/settings-stories.css";

// Окно «Доступ ИИ-агентов» (слайс 4a) поверх «главного окна». «Сейчас» фиксировано: вторник, 28 октября 2026, 14:10 (UTC).
const NOW = new Date(Date.UTC(2026, 9, 28, 14, 10));
const at = (hour: number, minute: number) => Date.UTC(2026, 9, 28, hour, minute);
const noop = () => undefined;

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
  row(5, { tool: "add_entry", at: at(14, 6), outcome: "read_only" }),
  row(4, { tool: "add_entry", at: at(14, 5), count: 1 }),
  row(3, { tool: "search", at: at(14, 2), query: "Raft", count: 3 }),
  row(2, { tool: "list_chats", at: at(14, 1) }),
  row(1, { tool: "connect", at: at(14, 0) }),
];

const LONG_QUERY = "что я писал про распределённые транзакции, двухфазную фиксацию и консенсус Raft в марте и апреле прошлого года";

const entry = (messageId: string, chatTitle: string, icon: string, text: string, chatIsInbox = false): AgentLogEntry => ({
  messageId,
  chatTitle,
  chatIcon: icon,
  chatIsInbox,
  message: mockMessage({ id: messageId, bodyMd: text, journalDate: "2026-03-14", origin: "user" }),
});

const EXPANDED_ENTRIES: Record<number, AgentLogEntry[]> = {
  3: [
    entry("m1", "Работа", "💼", "Итоги встречи: договорились разобрать Raft и сравнить с Paxos до конца месяца"),
    entry("m2", "Входящие", "📥", "Мысль про Raft: лидер выбирается по таймауту", true),
    { messageId: "gone", message: null, chatTitle: "", chatIcon: null, chatIsInbox: false },
  ],
};

function Demo(props: Partial<AgentAccessDialogProps> & { access?: AgentAccess }) {
  const [warning, setWarning] = useState(props.warning ?? false);
  return (
    <div className="story-settings-window">
      <div className="story-settings-window__sidebar" />
      <div className="story-settings-window__feed">
        <div className="story-settings-window__plate" />
        <div className="story-settings-window__plate story-settings-window__plate--tall" />
      </div>
      <AgentAccessDialog
        access={access()}
        log={{ rows: ROWS, hasMore: false, expandedId: null, entries: {}, gone: new Set() }}
        language="ru"
        now={NOW}
        error={undefined}
        onEnabledChange={noop}
        onConfirmEnable={() => setWarning(false)}
        onCancelEnable={() => setWarning(false)}
        onModeChange={noop}
        onChatToggle={noop}
        onToggleRow={noop}
        onOpenEntry={noop}
        onMoreLog={noop}
        onClose={noop}
        {...props}
        warning={warning}
      />
    </div>
  );
}

const meta = {
  title: "Компоненты/Доступ ИИ-агентов",
  component: Demo,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Доступ выключен: режим и чаты недоступны, журнал и «Как подключить» видны. */
export const Выключен: Story = { args: { access: access({ enabled: false, chats: CHATS.map((c) => ({ ...c, visible: false })) }) } };

/** Предупреждение при включении: второе окно поверх, фокус на «Отмена». */
export const ПредупреждениеПриВключении: Story = {
  args: { access: access({ enabled: false, chats: CHATS.map((c) => ({ ...c, visible: false })) }), warning: true },
};

/** Включён, «только чтение»: отмечены Входящие и Работа, «Дневник» исключён из AI и неактивен; фрагмент и команда с «Копировать». */
export const ВключёнТолькоЧтение: Story = {};

/** Включён, «чтение и запись». */
export const ВключёнЧтениеИЗапись: Story = { args: { access: access({ mode: "read_write" }) } };

/** Журнал: строка раскрыта, записи с чатом, датой и началом текста; удалённая навсегда — тихая строка. */
export const ЖурналРаскрыт: Story = {
  args: { log: { rows: ROWS, hasMore: true, expandedId: 3, entries: EXPANDED_ENTRIES, gone: new Set() } },
};

/** Журнал пуст. */
export const ЖурналПуст: Story = { args: { log: { rows: [], hasMore: false, expandedId: null, entries: {}, gone: new Set() } } };

/** Запись из раскрытой строки оказалась удалённой навсегда: на её месте «Запись удалена». */
export const ЗаписьУдалена: Story = {
  args: { log: { rows: ROWS, hasMore: false, expandedId: 3, entries: EXPANDED_ENTRIES, gone: new Set(["m1"]) } },
};

/** Ошибка: папка сокета чужая; вместо «Как подключить» одна строка ошибки. */
export const ОшибкаПапкаСокета: Story = {
  args: { access: access({ socket: { ...MOCK_AGENT_SOCKET, listening: false, error: "dir_unsafe", desktopConfig: null, codeCommand: null } }) },
};

/** Ошибка: мост не найден рядом с приложением (сборка dev). */
export const ОшибкаМостНеНайден: Story = {
  args: { access: access({ socket: { ...MOCK_AGENT_SOCKET, listening: false, error: "bridge_missing", desktopConfig: null, codeCommand: null } }) },
};

/** Ошибка последнего действия под флажком. */
export const ОшибкаДействия: Story = { args: { error: "Чат не найден." } };

/** Длинные имена чатов и запросов, имя клиента с угловыми скобками: ничего не ломает вёрстку. */
export const ДлинныеИмена: Story = {
  args: {
    access: access({
      chats: [
        ...CHATS.slice(0, 2),
        chat("long", "Очень длинное название чата про распределённые системы, консенсус и транзакции за все годы", "🧪", 12345, { visible: true }),
        chat("emoji", "Чат", null, 0),
      ],
    }),
    log: {
      rows: [
        row(8, { tool: "search", at: at(14, 9), client: "Некий клиент с очень длинным названием, которое придумал не Anthropic", query: LONG_QUERY, count: 12 }),
        row(7, { tool: "add_entry", at: Date.UTC(2026, 9, 21, 9, 0), client: "<b>Claude</b>", outcome: "no_agent_entry" }),
        row(6, { tool: "get_entries", at: Date.UTC(2025, 11, 31, 23, 59), count: 200 }),
      ],
      hasMore: true,
      expandedId: 8,
      entries: { 8: [entry("m1", "Очень длинное название чата про распределённые системы", "🧪", "Строка текста записи, которая заведомо длиннее ширины окна и должна обрезаться многоточием, а не растягивать вёрстку")] },
      gone: new Set(),
    },
  },
};

/** English. */
export const English: Story = { args: { language: "en" }, globals: { language: "en" } };
