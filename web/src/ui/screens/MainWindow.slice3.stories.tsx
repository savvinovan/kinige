import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import type { TrashItem } from "../../api/generated/TrashItem";
import { withTints } from "../sidebar/storyChats";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// Slice 3 in the main window: chats by section, a collapsed panel, editing and deleting an entry, a tag filter, the trash.
// "Today" is fixed in the stories (Tuesday, 20 October 2026): day labels and snapshots do not change from run to run.
const TODAY = "2026-10-20";

const noop = () => undefined;
const sidebar = {
  collapsed: false,
  onToggleCollapsed: noop,
  onCreateChat: noop,
  onChatAction: noop,
  trashCount: 3,
  onOpenTrash: noop,
};

const CHATS = withTints([
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 42 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "ideas", name: "Идеи", emoji: "💡", count: 12 },
  { id: "long", name: "Очень длинное название чата, которое не помещается в строку сайдбара", emoji: "🧭", count: 7 },
  { id: "old", name: "Старые заметки", emoji: "🗄️", count: 310, archived: true },
]);

const MESSAGES = [
  demoEntry("m1", "2026-10-19", "21:05", "Идея для заметок про поездку. #идеи #поездки"),
  demoEntry("m2", TODAY, "09:12", "Купить хлеб и молоко #еда"),
  demoEntry("m3", TODAY, "09:14", "- [x] Позвонить маме\n- [ ] Написать Ире"),
  demoEntry("m4", TODAY, "13:40", "Прочитать главу про консенсус"),
];

const meta = {
  title: "Экраны/Главное окно: чаты и записи",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: {
    chats: CHATS,
    selectedChatId: "books",
    messages: MESSAGES,
    today: TODAY,
    interactive: true,
    showSearch: false,
    composerButtons: { emoji: true },
    sidebar,
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A pinned chat, regular ones, the "Archive" section (collapsed) and the trash with a counter. */
export const Чаты: Story = {};

/** The same in English and with large text. */
export const ЧатыEnКрупный: Story = { globals: { language: "en", textScale: "large" } };

/** The panel is collapsed to a narrow strip: chat icons and the trash. */
export const ПанельСвёрнута: Story = { args: { sidebar: { ...sidebar, collapsed: true } } };

/** An entry being edited: a field in place of the plate, the width is fixed. */
export const Правка: Story = { args: { feedUi: { editingId: "m2" } } };

/** A deleted entry stays in place, dimmed: "Deleted · Restore". */
export const УдалённаяЗапись: Story = { args: { feedUi: { deletedIds: new Set(["m3"]) } } };

/** A tag filter: the "#food x" strip above the feed. */
export const ФильтрПоТегу: Story = {
  args: { messages: [MESSAGES[1] as (typeof MESSAGES)[number]], tagFilter: { tag: "еда", onClear: noop } },
};

/** A task, a favorite and a read-only entry. */
export const Флажки: Story = {
  args: {
    messages: [
      { ...MESSAGES[1], isTask: true, tags: ["еда"] },
      { ...MESSAGES[2], isTask: true, taskDoneAt: Date.UTC(2026, 9, 20, 10, 0), favoriteAt: Date.UTC(2026, 9, 20, 10, 5) },
      { ...MESSAGES[3], readOnly: true },
    ] as (typeof MESSAGES)[number][],
  },
};

const trashEntry = (id: string, body: string, daysLeft: number, chat: [string, string, boolean]): TrashItem => ({
  message: {
    id,
    chatId: "c",
    bodyMd: body,
    journalDate: "2026-10-18",
    createdAt: Date.UTC(2026, 9, 18, 9, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: [],
    readOnly: false,
    origin: "user",
    replyTo: null,
    attachments: [],
  },
  chatTitle: chat[0],
  chatIcon: chat[1],
  chatIsInbox: chat[2],
  chatColor: chat[2] ? "grey" : "blue",
  deletedAt: Date.UTC(2026, 9, 19, 18, 0),
  daysLeft,
});

/** The trash in place of the feed: the deadline, "Restore" and "Delete forever". */
export const Корзина: Story = {
  args: {
    selectedChatId: null,
    trash: {
      items: [
        trashEntry("t1", "Купить хлеб", 29, ["Входящие", "📥", true]),
        trashEntry("t2", "Черновик письма", 12, ["Работа", "💼", false]),
        trashEntry("t3", "Старая идея", 1, ["Идеи", "💡", false]),
      ],
      today: TODAY,
      onRestore: noop,
      onPurge: noop,
    },
  },
};

/** An empty trash. */
export const КорзинаПуста: Story = {
  args: { selectedChatId: null, sidebar: { ...sidebar, trashCount: 0 }, trash: { items: [], today: TODAY, onRestore: noop, onPurge: noop } },
};
