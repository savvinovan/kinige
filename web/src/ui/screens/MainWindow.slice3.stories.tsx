import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import type { TrashItem } from "../../api/generated/TrashItem";
import type { SidebarChat } from "../sidebar/Sidebar";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// Слайс 3 в главном окне: чаты по разделам, свёрнутая панель, правка и удаление записи, фильтр по тегу, корзина.
// «Сегодня» в историях фиксировано (вторник, 20 октября 2026): метки дней и снимки не меняются от запуска к запуску.
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

const CHATS: SidebarChat[] = [
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 42 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "ideas", name: "Идеи", emoji: "💡", count: 12 },
  { id: "long", name: "Очень длинное название чата, которое не помещается в строку сайдбара", emoji: "🧭", count: 7 },
  { id: "old", name: "Старые заметки", emoji: "🗄️", count: 310, archived: true },
];

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

/** Закреплённый чат, обычные, раздел «Архив» (свёрнут) и корзина со счётчиком. */
export const Чаты: Story = {};

/** То же по-английски и с крупным текстом. */
export const ЧатыEnКрупный: Story = { globals: { language: "en", textScale: "large" } };

/** Панель свёрнута до узкой полосы: значки чатов и корзина. */
export const ПанельСвёрнута: Story = { args: { sidebar: { ...sidebar, collapsed: true } } };

/** Запись в правке: на месте плашки поле, ширина закреплена. */
export const Правка: Story = { args: { feedUi: { editingId: "m2" } } };

/** Удалённая запись остаётся на месте приглушённой: «Удалено · Вернуть». */
export const УдалённаяЗапись: Story = { args: { feedUi: { deletedIds: new Set(["m3"]) } } };

/** Фильтр по тегу: строка «#еда ×» над лентой. */
export const ФильтрПоТегу: Story = {
  args: { messages: [MESSAGES[1] as (typeof MESSAGES)[number]], tagFilter: { tag: "еда", onClear: noop } },
};

/** Задача, избранное и запись только для чтения. */
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
  },
  chatTitle: chat[0],
  chatIcon: chat[1],
  chatIsInbox: chat[2],
  deletedAt: Date.UTC(2026, 9, 19, 18, 0),
  daysLeft,
});

/** Корзина на месте ленты: срок, «Восстановить» и «Удалить навсегда». */
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

/** Пустая корзина. */
export const КорзинаПуста: Story = {
  args: { selectedChatId: null, sidebar: { ...sidebar, trashCount: 0 }, trash: { items: [], today: TODAY, onRestore: noop, onPurge: noop } },
};
