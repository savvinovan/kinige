import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import type { SearchHit } from "../../api/generated/SearchHit";
import { NO_FILTERS, SearchResults } from "../search/SearchResults";
import type { SidebarChat } from "../sidebar/Sidebar";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// Слайс 4 в главном окне: сайдбар с импортированными чатами, поиск с результатами на месте ленты, запись, открытая из
// поиска, архив прошлых лет в ленте. «Сегодня» в историях фиксировано (вторник, 20 октября 2026).
const TODAY = "2026-10-20";
const noop = () => undefined;

const CHATS: SidebarChat[] = [
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 1724 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "tg1", name: "Пётр Тестов", emoji: "💬", count: 240 },
  { id: "tg2", name: "Анна", emoji: "💬", count: 12 },
];

const OLD = [
  demoEntry("o1", "2019-03-08", "12:00", "Первая запись из архива Telegram, март 2019"),
  demoEntry("o2", "2019-03-08", "12:03", "**Пётр Тестов**\nПривет, как книга?"),
  demoEntry("o3", "2019-03-08", "12:04", "_Переслано от Канал_\nЗапись про #книги из канала"),
  demoEntry("o4", "2023-03-14", "09:30", "_[Фото не импортировано]_\nПодпись к фото"),
  demoEntry("o5", TODAY, "09:12", "Вчерашние планы: дочитать книгу"),
];

const hit = (id: string, snippet: string, terms: string[], chatTitle: string, chatIcon: string, inbox: boolean, day: string): SearchHit => ({
  message: {
    id,
    chatId: "c",
    bodyMd: snippet,
    journalDate: day,
    createdAt: Date.UTC(2026, 9, 20, 9, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: [],
    readOnly: false,
    origin: "user",
  },
  chatTitle,
  chatIcon,
  chatIsInbox: inbox,
  snippet,
  terms,
});

const HITS = [
  hit("h1", "Вчерашние планы: дочитать книгу", ["книгу"], "", "📥", true, TODAY),
  hit("h2", "Привет, как книга?", ["книга"], "Пётр Тестов", "💬", false, "2019-03-08"),
  hit("h3", "Запись про #книги из канала", ["книги"], "", "📥", true, "2019-03-08"),
];

const searchView = (language: "ru" | "en") => (
  <SearchResults
    listId="results"
    query="книги"
    chats={[{ id: "books", title: "Книги", icon: "📚", isInbox: false, count: 1234, pinnedAt: null, archivedAt: null }]}
    hits={HITS}
    hasMore={false}
    loading={false}
    searched
    today={TODAY}
    language={language}
    filters={NO_FILTERS}
    onFiltersChange={noop}
    canScopeChat
    onPickDate={noop}
    activeIndex={1}
    onActiveChange={noop}
    onOpenChat={noop}
    onOpenHit={noop}
    onLoadMore={noop}
  />
);

const meta = {
  title: "Экраны/Главное окно: импорт и поиск",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: {
    chats: CHATS,
    selectedChatId: INBOX_ID,
    messages: OLD,
    today: TODAY,
    interactive: true,
    showSearch: false,
    composerButtons: { emoji: true },
    sidebar: { collapsed: false, onToggleCollapsed: noop, onCreateChat: noop, onChatAction: noop, trashCount: 0, onOpenTrash: noop },
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Лента прошлых лет: метки «8 марта 2019», пересланное, фото-заглушка, чужие сообщения с именем автора жирным. */
export const АрхивПрошлыхЛет: Story = {};

/** Поле «Поиск» с запросом и результаты на месте ленты (поле ввода скрыто). */
export const ПоискРезультаты: Story = {
  args: {
    search: { value: "книги", onChange: noop, resultsId: "results", activeDescendant: "results-row-1" },
    searchResults: searchView("ru"),
  },
};

/** То же по-английски и с крупным текстом. */
export const ПоискEnКрупный: Story = {
  args: {
    search: { value: "книги", onChange: noop, resultsId: "results", activeDescendant: "results-row-1" },
    searchResults: searchView("en"),
  },
  globals: { language: "en", textScale: "large" },
};

/** Запись, открытая из поиска: подсвечена заливкой, найденное слово выделено. */
export const ЗаписьИзПоиска: Story = {
  args: {
    feedUi: { markedId: "o5", markTerms: ["книгу"] },
  },
};

/** Свёрнутый сайдбар: значок поиска под кнопкой «развернуть». */
export const СвёрнутыйСПоиском: Story = {
  args: {
    search: { value: "", onChange: noop, onRailClick: noop },
    sidebar: { collapsed: true, onToggleCollapsed: noop, onCreateChat: noop, onChatAction: noop, trashCount: 0, onOpenTrash: noop },
  },
};
