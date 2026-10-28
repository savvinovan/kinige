import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { SearchHit } from "../../api/generated/SearchHit";
import { Frame } from "../stories/Frame";
import { NO_FILTERS, SearchResults } from "./SearchResults";
import "./search-stories.css";

// Результаты поиска на месте ленты. «Сегодня» в историях фиксировано (вторник, 27 октября 2026).
const TODAY = "2026-10-27";
const noop = () => undefined;

const hit = (id: string, snippet: string, terms: string[], patch: Partial<SearchHit> = {}, journalDate = TODAY): SearchHit => ({
  message: {
    id,
    chatId: "c1",
    bodyMd: snippet,
    journalDate,
    createdAt: Date.UTC(2026, 9, 27, 9, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: [],
    readOnly: false,
  },
  chatTitle: "",
  chatIcon: "📥",
  chatIsInbox: true,
  snippet,
  terms,
  ...patch,
});

const HITS: SearchHit[] = [
  hit("a", "Читаю книгу про Raft и распределённые транзакции, пока в дороге", ["книгу"]),
  hit("b", "Книга о Rust: глава про владение и заимствование", ["Книга"], { chatTitle: "Книги", chatIcon: "📚", chatIsInbox: false }, "2026-10-26"),
  hit(
    "c",
    "…разобрать, что в книге сказано про консенсус, и записать вопросы; книгами из списка занимаюсь по вечерам, а пока откладываю…",
    ["книге"],
    { chatTitle: "Учёба", chatIcon: "🎓", chatIsInbox: false },
    "2025-03-14",
  ),
  hit("d", "Очень-очень длинная строка без пробелов для проверки переноса: КнигаКнигаКнигаКнигаКнигаКнигаКнигаКнигаКнигаКнига", ["Книга"], {
    chatTitle: "Очень длинное название чата, которое не помещается в строку and Some Long English Words",
    chatIcon: "🗂️",
    chatIsInbox: false,
  }),
];

const CHAT: ChatSummary = { id: "books", title: "Книги", icon: "📚", isInbox: false, count: 1234, pinnedAt: null, archivedAt: null };

const meta = {
  title: "Экраны/Поиск",
  component: SearchResults,
  parameters: { layout: "fullscreen" },
  args: {
    listId: "results",
    query: "книги",
    chats: [],
    hits: HITS,
    hasMore: false,
    loading: false,
    searched: true,
    error: null,
    today: TODAY,
    language: "ru",
    filters: NO_FILTERS,
    onFiltersChange: noop,
    canScopeChat: true,
    onPickDate: noop,
    activeIndex: 0,
    onActiveChange: noop,
    onOpenChat: noop,
    onOpenHit: noop,
    onLoadMore: noop,
  },
  render: (args) => (
    <div className="story-search">
      <Frame bottom>
        <SearchResults {...args} />
      </Frame>
    </div>
  ),
} satisfies Meta<typeof SearchResults>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Записи с подсвеченными словами: Входящие, чат с эмодзи, другой год, длинное имя чата и слово без пробелов. */
export const Результаты: Story = {};

/** Чат, чьё название совпало, — отдельной строкой «Чаты» над записями. */
export const СЧатами: Story = { args: { chats: [CHAT], activeIndex: 1 } };

/** Выбранная стрелками строка заливкой и тонкой линией акцента. */
export const ВыбраннаяСтрока: Story = { args: { activeIndex: 2 } };

/** Фильтры: этот чат, избранное, задачи, обе даты. */
export const Фильтры: Story = {
  args: { filters: { scope: "chat", favorite: true, tasks: true, from: "2026-01-01", to: "2026-10-26" } },
};

/** Есть следующая страница: «Показать ещё». */
export const ЕщёСтраница: Story = { args: { hasMore: true } };

/** Ищем: идёт запрос. */
export const Ищем: Story = { args: { hits: [], searched: false, loading: true } };

/** Пустой результат: одна тихая строка. */
export const Пусто: Story = { args: { hits: [], query: "ничего" } };

/** Пустой запрос: подсказка. */
export const ПустойЗапрос: Story = { args: { hits: [], query: "", searched: false } };

/** Ошибка чтения: строка ошибки D1. */
export const Ошибка: Story = { args: { hits: [], error: "store_read_failed" } };

/** Английский. */
export const Английский: Story = { args: { chats: [CHAT], language: "en" }, globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { args: { chats: [CHAT] }, globals: { textScale: "large" } };
